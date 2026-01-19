//! Constant folding pass
//!
//! Evaluates operations on literal values at compile time, replacing compute
//! steps with bind steps containing the result.

use covenant_ast::{BindSource, BindStep, ComputeStep, InputSource, Operation, Step, StepKind};
use covenant_ast::Literal;

use crate::passes::{OptContext, OptimizationPass, PassResult};

/// Constant folding optimization pass
///
/// Folds constant expressions like `add(lit=2, lit=3)` into `lit=5`.
/// Only folds when ALL inputs are literals.
pub struct ConstantFolding;

impl OptimizationPass for ConstantFolding {
    fn name(&self) -> &'static str {
        "constant-folding"
    }

    fn run(&self, steps: &mut Vec<Step>, _ctx: &OptContext) -> PassResult {
        let mut modified = false;

        for step in steps.iter_mut() {
            if let StepKind::Compute(compute) = &step.kind {
                if let Some(result) = try_fold(compute) {
                    let span = compute.span;
                    step.kind = StepKind::Bind(BindStep {
                        source: BindSource::Lit(result),
                        span,
                    });
                    modified = true;
                }
            }
        }

        PassResult {
            modified,
            warnings: vec![],
        }
    }
}

/// Try to fold a compute step into a literal
fn try_fold(compute: &ComputeStep) -> Option<Literal> {
    // Collect all inputs - only fold if ALL are literals
    let literals: Vec<&Literal> = compute
        .inputs
        .iter()
        .filter_map(|input| match &input.source {
            InputSource::Lit(lit) => Some(lit),
            _ => None,
        })
        .collect();

    // If not all inputs are literals, can't fold
    if literals.len() != compute.inputs.len() {
        return None;
    }

    fold_operation(compute.op, &literals)
}

/// Fold an operation with literal inputs
fn fold_operation(op: Operation, inputs: &[&Literal]) -> Option<Literal> {
    match op {
        // Arithmetic (binary)
        Operation::Add => fold_binary_arithmetic(inputs, |a, b| a + b, |a, b| a + b),
        Operation::Sub => fold_binary_arithmetic(inputs, |a, b| a - b, |a, b| a - b),
        Operation::Mul => fold_binary_arithmetic(inputs, |a, b| a * b, |a, b| a * b),
        Operation::Div => fold_binary_arithmetic_checked(inputs,
            |a, b| if b != 0 { Some(a / b) } else { None },
            |a, b| if b != 0.0 { Some(a / b) } else { None }),
        Operation::Mod => fold_binary_arithmetic_checked(inputs,
            |a, b| if b != 0 { Some(a % b) } else { None },
            |_, _| None), // Mod for floats not supported

        // Comparison
        Operation::Equals => fold_equals(inputs),
        Operation::NotEquals => fold_not_equals(inputs),
        Operation::Less => fold_ord_comparison(inputs, |a, b| a < b),
        Operation::Greater => fold_ord_comparison(inputs, |a, b| a > b),
        Operation::LessEq => fold_ord_comparison(inputs, |a, b| a <= b),
        Operation::GreaterEq => fold_ord_comparison(inputs, |a, b| a >= b),

        // Logical
        Operation::And => fold_logical_binary(inputs, |a, b| a && b),
        Operation::Or => fold_logical_binary(inputs, |a, b| a || b),
        Operation::Not => fold_logical_unary(inputs, |a| !a),
        Operation::Neg => fold_negation(inputs),

        // Numeric
        Operation::Abs => fold_abs(inputs),
        Operation::Min => fold_min_max(inputs, true),
        Operation::Max => fold_min_max(inputs, false),

        // String operations that can be folded
        Operation::Concat => fold_string_concat(inputs),
        Operation::StrLen => fold_string_len(inputs),
        Operation::Upper => fold_string_case(inputs, true),
        Operation::Lower => fold_string_case(inputs, false),
        Operation::Contains => fold_string_contains(inputs),
        Operation::StartsWith => fold_string_starts_ends(inputs, true),
        Operation::EndsWith => fold_string_starts_ends(inputs, false),
        Operation::IsEmpty => fold_is_empty(inputs),

        // Everything else can't be folded (or isn't worth the complexity)
        _ => None,
    }
}

fn fold_binary_arithmetic<F, G>(inputs: &[&Literal], int_op: F, float_op: G) -> Option<Literal>
where
    F: Fn(i64, i64) -> i64,
    G: Fn(f64, f64) -> f64,
{
    if inputs.len() != 2 {
        return None;
    }
    match (inputs[0], inputs[1]) {
        (Literal::Int(a), Literal::Int(b)) => Some(Literal::Int(int_op(*a, *b))),
        (Literal::Float(a), Literal::Float(b)) => Some(Literal::Float(float_op(*a, *b))),
        // Mixed int/float - promote to float
        (Literal::Int(a), Literal::Float(b)) => Some(Literal::Float(float_op(*a as f64, *b))),
        (Literal::Float(a), Literal::Int(b)) => Some(Literal::Float(float_op(*a, *b as f64))),
        _ => None,
    }
}

fn fold_binary_arithmetic_checked<F, G>(inputs: &[&Literal], int_op: F, float_op: G) -> Option<Literal>
where
    F: Fn(i64, i64) -> Option<i64>,
    G: Fn(f64, f64) -> Option<f64>,
{
    if inputs.len() != 2 {
        return None;
    }
    match (inputs[0], inputs[1]) {
        (Literal::Int(a), Literal::Int(b)) => int_op(*a, *b).map(Literal::Int),
        (Literal::Float(a), Literal::Float(b)) => float_op(*a, *b).map(Literal::Float),
        _ => None,
    }
}

fn fold_equals(inputs: &[&Literal]) -> Option<Literal> {
    if inputs.len() != 2 {
        return None;
    }
    let result = match (inputs[0], inputs[1]) {
        (Literal::Int(a), Literal::Int(b)) => a == b,
        (Literal::Float(a), Literal::Float(b)) => (a - b).abs() < f64::EPSILON,
        (Literal::Bool(a), Literal::Bool(b)) => a == b,
        (Literal::String(a), Literal::String(b)) => a == b,
        _ => return None,
    };
    Some(Literal::Bool(result))
}

fn fold_not_equals(inputs: &[&Literal]) -> Option<Literal> {
    fold_equals(inputs).map(|lit| {
        match lit {
            Literal::Bool(b) => Literal::Bool(!b),
            _ => lit,
        }
    })
}

fn fold_ord_comparison<F>(inputs: &[&Literal], cmp: F) -> Option<Literal>
where
    F: Fn(f64, f64) -> bool,
{
    if inputs.len() != 2 {
        return None;
    }
    match (inputs[0], inputs[1]) {
        (Literal::Int(a), Literal::Int(b)) => Some(Literal::Bool(cmp(*a as f64, *b as f64))),
        (Literal::Float(a), Literal::Float(b)) => Some(Literal::Bool(cmp(*a, *b))),
        (Literal::Int(a), Literal::Float(b)) => Some(Literal::Bool(cmp(*a as f64, *b))),
        (Literal::Float(a), Literal::Int(b)) => Some(Literal::Bool(cmp(*a, *b as f64))),
        _ => None,
    }
}

fn fold_logical_binary<F>(inputs: &[&Literal], op: F) -> Option<Literal>
where
    F: Fn(bool, bool) -> bool,
{
    if inputs.len() != 2 {
        return None;
    }
    match (inputs[0], inputs[1]) {
        (Literal::Bool(a), Literal::Bool(b)) => Some(Literal::Bool(op(*a, *b))),
        _ => None,
    }
}

fn fold_logical_unary<F>(inputs: &[&Literal], op: F) -> Option<Literal>
where
    F: Fn(bool) -> bool,
{
    if inputs.len() != 1 {
        return None;
    }
    match inputs[0] {
        Literal::Bool(a) => Some(Literal::Bool(op(*a))),
        _ => None,
    }
}

fn fold_negation(inputs: &[&Literal]) -> Option<Literal> {
    if inputs.len() != 1 {
        return None;
    }
    match inputs[0] {
        Literal::Int(a) => Some(Literal::Int(-*a)),
        Literal::Float(a) => Some(Literal::Float(-*a)),
        _ => None,
    }
}

fn fold_abs(inputs: &[&Literal]) -> Option<Literal> {
    if inputs.len() != 1 {
        return None;
    }
    match inputs[0] {
        Literal::Int(a) => Some(Literal::Int(a.abs())),
        Literal::Float(a) => Some(Literal::Float(a.abs())),
        _ => None,
    }
}

fn fold_min_max(inputs: &[&Literal], is_min: bool) -> Option<Literal> {
    if inputs.len() != 2 {
        return None;
    }
    match (inputs[0], inputs[1]) {
        (Literal::Int(a), Literal::Int(b)) => {
            Some(Literal::Int(if is_min { (*a).min(*b) } else { (*a).max(*b) }))
        }
        (Literal::Float(a), Literal::Float(b)) => {
            Some(Literal::Float(if is_min { a.min(*b) } else { a.max(*b) }))
        }
        _ => None,
    }
}

fn fold_string_concat(inputs: &[&Literal]) -> Option<Literal> {
    if inputs.len() != 2 {
        return None;
    }
    match (inputs[0], inputs[1]) {
        (Literal::String(a), Literal::String(b)) => {
            Some(Literal::String(format!("{}{}", a, b)))
        }
        _ => None,
    }
}

fn fold_string_len(inputs: &[&Literal]) -> Option<Literal> {
    if inputs.len() != 1 {
        return None;
    }
    match inputs[0] {
        Literal::String(s) => Some(Literal::Int(s.chars().count() as i64)),
        _ => None,
    }
}

fn fold_string_case(inputs: &[&Literal], to_upper: bool) -> Option<Literal> {
    if inputs.len() != 1 {
        return None;
    }
    match inputs[0] {
        Literal::String(s) => {
            let result = if to_upper { s.to_uppercase() } else { s.to_lowercase() };
            Some(Literal::String(result))
        }
        _ => None,
    }
}

fn fold_string_contains(inputs: &[&Literal]) -> Option<Literal> {
    if inputs.len() != 2 {
        return None;
    }
    match (inputs[0], inputs[1]) {
        (Literal::String(haystack), Literal::String(needle)) => {
            Some(Literal::Bool(haystack.contains(needle.as_str())))
        }
        _ => None,
    }
}

fn fold_string_starts_ends(inputs: &[&Literal], is_starts: bool) -> Option<Literal> {
    if inputs.len() != 2 {
        return None;
    }
    match (inputs[0], inputs[1]) {
        (Literal::String(s), Literal::String(pattern)) => {
            let result = if is_starts {
                s.starts_with(pattern.as_str())
            } else {
                s.ends_with(pattern.as_str())
            };
            Some(Literal::Bool(result))
        }
        _ => None,
    }
}

fn fold_is_empty(inputs: &[&Literal]) -> Option<Literal> {
    if inputs.len() != 1 {
        return None;
    }
    match inputs[0] {
        Literal::String(s) => Some(Literal::Bool(s.is_empty())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use covenant_ast::{Input, Span};

    fn make_span() -> Span {
        Span::dummy()
    }

    fn make_compute_step(id: &str, binding: &str, op: Operation, inputs: Vec<Literal>) -> Step {
        Step {
            id: id.into(),
            kind: StepKind::Compute(ComputeStep {
                op,
                inputs: inputs
                    .into_iter()
                    .map(|lit| Input {
                        source: InputSource::Lit(lit),
                        span: make_span(),
                    })
                    .collect(),
                span: make_span(),
            }),
            output_binding: binding.into(),
            span: make_span(),
        }
    }

    fn make_ctx() -> OptContext {
        OptContext {
            settings: crate::passes::OptSettings {
                level: crate::passes::OptLevel::O2,
                emit_warnings: true,
            },
        }
    }

    #[test]
    fn test_fold_add_int() {
        let mut steps = vec![make_compute_step(
            "s1",
            "result",
            Operation::Add,
            vec![Literal::Int(2), Literal::Int(3)],
        )];

        let pass = ConstantFolding;
        let result = pass.run(&mut steps, &make_ctx());

        assert!(result.modified);
        match &steps[0].kind {
            StepKind::Bind(bind) => match &bind.source {
                BindSource::Lit(Literal::Int(5)) => {}
                other => panic!("Expected Int(5), got {:?}", other),
            },
            other => panic!("Expected Bind, got {:?}", other),
        }
    }

    #[test]
    fn test_fold_add_float() {
        let mut steps = vec![make_compute_step(
            "s1",
            "result",
            Operation::Add,
            vec![Literal::Float(2.5), Literal::Float(3.5)],
        )];

        let pass = ConstantFolding;
        let result = pass.run(&mut steps, &make_ctx());

        assert!(result.modified);
        match &steps[0].kind {
            StepKind::Bind(bind) => match &bind.source {
                BindSource::Lit(Literal::Float(f)) if (*f - 6.0).abs() < 0.001 => {}
                other => panic!("Expected Float(6.0), got {:?}", other),
            },
            other => panic!("Expected Bind, got {:?}", other),
        }
    }

    #[test]
    fn test_fold_sub() {
        let mut steps = vec![make_compute_step(
            "s1",
            "result",
            Operation::Sub,
            vec![Literal::Int(10), Literal::Int(3)],
        )];

        let pass = ConstantFolding;
        let result = pass.run(&mut steps, &make_ctx());

        assert!(result.modified);
        match &steps[0].kind {
            StepKind::Bind(bind) => match &bind.source {
                BindSource::Lit(Literal::Int(7)) => {}
                other => panic!("Expected Int(7), got {:?}", other),
            },
            _ => panic!("Expected Bind"),
        }
    }

    #[test]
    fn test_fold_and() {
        let mut steps = vec![make_compute_step(
            "s1",
            "result",
            Operation::And,
            vec![Literal::Bool(true), Literal::Bool(false)],
        )];

        let pass = ConstantFolding;
        let result = pass.run(&mut steps, &make_ctx());

        assert!(result.modified);
        match &steps[0].kind {
            StepKind::Bind(bind) => match &bind.source {
                BindSource::Lit(Literal::Bool(false)) => {}
                other => panic!("Expected Bool(false), got {:?}", other),
            },
            _ => panic!("Expected Bind"),
        }
    }

    #[test]
    fn test_fold_or() {
        let mut steps = vec![make_compute_step(
            "s1",
            "result",
            Operation::Or,
            vec![Literal::Bool(true), Literal::Bool(false)],
        )];

        let pass = ConstantFolding;
        let result = pass.run(&mut steps, &make_ctx());

        assert!(result.modified);
        match &steps[0].kind {
            StepKind::Bind(bind) => match &bind.source {
                BindSource::Lit(Literal::Bool(true)) => {}
                other => panic!("Expected Bool(true), got {:?}", other),
            },
            _ => panic!("Expected Bind"),
        }
    }

    #[test]
    fn test_fold_not() {
        let mut steps = vec![make_compute_step(
            "s1",
            "result",
            Operation::Not,
            vec![Literal::Bool(true)],
        )];

        let pass = ConstantFolding;
        let result = pass.run(&mut steps, &make_ctx());

        assert!(result.modified);
        match &steps[0].kind {
            StepKind::Bind(bind) => match &bind.source {
                BindSource::Lit(Literal::Bool(false)) => {}
                other => panic!("Expected Bool(false), got {:?}", other),
            },
            _ => panic!("Expected Bind"),
        }
    }

    #[test]
    fn test_fold_equals() {
        let mut steps = vec![make_compute_step(
            "s1",
            "result",
            Operation::Equals,
            vec![Literal::Int(5), Literal::Int(5)],
        )];

        let pass = ConstantFolding;
        let result = pass.run(&mut steps, &make_ctx());

        assert!(result.modified);
        match &steps[0].kind {
            StepKind::Bind(bind) => match &bind.source {
                BindSource::Lit(Literal::Bool(true)) => {}
                other => panic!("Expected Bool(true), got {:?}", other),
            },
            _ => panic!("Expected Bind"),
        }
    }

    #[test]
    fn test_fold_string_concat() {
        let mut steps = vec![make_compute_step(
            "s1",
            "result",
            Operation::Concat,
            vec![
                Literal::String("Hello, ".into()),
                Literal::String("World!".into()),
            ],
        )];

        let pass = ConstantFolding;
        let result = pass.run(&mut steps, &make_ctx());

        assert!(result.modified);
        match &steps[0].kind {
            StepKind::Bind(bind) => match &bind.source {
                BindSource::Lit(Literal::String(s)) if s == "Hello, World!" => {}
                other => panic!("Expected String(\"Hello, World!\"), got {:?}", other),
            },
            _ => panic!("Expected Bind"),
        }
    }

    #[test]
    fn test_no_fold_with_variable() {
        let mut steps = vec![Step {
            id: "s1".into(),
            kind: StepKind::Compute(ComputeStep {
                op: Operation::Add,
                inputs: vec![
                    Input {
                        source: InputSource::Var("x".into()),
                        span: make_span(),
                    },
                    Input {
                        source: InputSource::Lit(Literal::Int(1)),
                        span: make_span(),
                    },
                ],
                span: make_span(),
            }),
            output_binding: "result".into(),
            span: make_span(),
        }];

        let pass = ConstantFolding;
        let result = pass.run(&mut steps, &make_ctx());

        assert!(!result.modified);
        assert!(matches!(&steps[0].kind, StepKind::Compute(_)));
    }

    #[test]
    fn test_fold_div_by_zero_no_fold() {
        let mut steps = vec![make_compute_step(
            "s1",
            "result",
            Operation::Div,
            vec![Literal::Int(10), Literal::Int(0)],
        )];

        let pass = ConstantFolding;
        let result = pass.run(&mut steps, &make_ctx());

        assert!(!result.modified); // Division by zero is not folded
    }
}
