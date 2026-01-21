//! Binding usage analysis
//!
//! Tracks which bindings are defined and used by which steps, enabling
//! dead code elimination and unused binding detection.

use std::collections::{HashMap, HashSet};

use covenant_ast::{
    BindSource, CallStep, ComputeStep, ConditionKind, InputSource,
    QueryContent, ReturnStep, ReturnValue, Step, StepKind,
};

/// Result of usage analysis on a function body
#[derive(Debug, Clone, Default)]
pub struct UsageAnalysis {
    /// Map from binding name to step IDs that use it
    pub used_by: HashMap<String, HashSet<String>>,
    /// Map from step ID to the binding it produces
    pub produces: HashMap<String, String>,
    /// Map from step ID to bindings it consumes
    pub consumes: HashMap<String, HashSet<String>>,
}

/// Analyze binding usage in a list of steps
pub fn analyze_usage(steps: &[Step]) -> UsageAnalysis {
    let mut analysis = UsageAnalysis::default();

    for step in steps {
        // Record what this step produces
        if !step.output_binding.is_empty() && step.output_binding != "_" {
            analysis
                .produces
                .insert(step.id.clone(), step.output_binding.clone());
        }

        // Find all bindings this step consumes
        let mut consumed = HashSet::new();
        collect_consumed_bindings(&step.kind, &mut consumed);

        // Record consumption
        analysis.consumes.insert(step.id.clone(), consumed.clone());

        // Build reverse map
        for binding in consumed {
            analysis
                .used_by
                .entry(binding)
                .or_default()
                .insert(step.id.clone());
        }

        // Recursively analyze nested steps (if, match, for, transaction)
        analyze_nested_steps(&step.kind, &mut analysis);
    }

    analysis
}

/// Collect all variable bindings consumed by a step kind
fn collect_consumed_bindings(kind: &StepKind, consumed: &mut HashSet<String>) {
    match kind {
        StepKind::Compute(compute) => {
            collect_from_compute(compute, consumed);
        }
        StepKind::Call(call) => {
            collect_from_call(call, consumed);
        }
        StepKind::Query(query) => {
            // Collect from query parameters and conditions
            match &query.content {
                QueryContent::Covenant(cov) => {
                    if let Some(cond) = &cov.where_clause {
                        collect_from_condition(&cond.kind, consumed);
                    }
                }
                QueryContent::Dialect(dialect) => {
                    for param in &dialect.params {
                        consumed.insert(param.from.clone());
                    }
                }
            }
        }
        StepKind::Bind(bind) => {
            collect_from_bind_source(&bind.source, consumed);
        }
        StepKind::Return(ret) => {
            collect_from_return(ret, consumed);
        }
        StepKind::If(if_step) => {
            // Condition is a binding reference
            consumed.insert(if_step.condition.clone());
        }
        StepKind::Match(match_step) => {
            // The value being matched on
            consumed.insert(match_step.on.clone());
        }
        StepKind::For(for_step) => {
            // The collection being iterated
            consumed.insert(for_step.collection.clone());
        }
        StepKind::Insert(insert) => {
            for assignment in &insert.assignments {
                collect_from_input_source(&assignment.value, consumed);
            }
        }
        StepKind::Update(update) => {
            for assignment in &update.assignments {
                collect_from_input_source(&assignment.value, consumed);
            }
            if let Some(cond) = &update.where_clause {
                collect_from_condition(&cond.kind, consumed);
            }
        }
        StepKind::Delete(delete) => {
            if let Some(cond) = &delete.where_clause {
                collect_from_condition(&cond.kind, consumed);
            }
        }
        StepKind::Transaction(_) => {
            // Transaction itself doesn't consume bindings directly
            // (nested steps are handled separately)
        }
        StepKind::Traverse(traverse) => {
            consumed.insert(traverse.from.clone());
        }
        StepKind::Construct(construct) => {
            for field in &construct.fields {
                collect_from_input_source(&field.value, consumed);
            }
        }
    }
}

fn collect_from_compute(compute: &ComputeStep, consumed: &mut HashSet<String>) {
    for input in &compute.inputs {
        collect_from_input_source(&input.source, consumed);
    }
}

fn collect_from_call(call: &CallStep, consumed: &mut HashSet<String>) {
    for arg in &call.args {
        collect_from_input_source(&arg.source, consumed);
    }
}

fn collect_from_return(ret: &ReturnStep, consumed: &mut HashSet<String>) {
    match &ret.value {
        ReturnValue::Var(name) => {
            consumed.insert(name.clone());
        }
        ReturnValue::Lit(_) => {}
        ReturnValue::Struct(s) => {
            for field in &s.fields {
                collect_from_input_source(&field.value, consumed);
            }
        }
        ReturnValue::Variant(v) => {
            for field in &v.fields {
                collect_from_input_source(&field.value, consumed);
            }
        }
    }
}

fn collect_from_input_source(source: &InputSource, consumed: &mut HashSet<String>) {
    match source {
        InputSource::Var(name) => {
            consumed.insert(name.clone());
        }
        InputSource::Lit(_) => {}
        InputSource::Field { of, .. } => {
            consumed.insert(of.clone());
        }
    }
}

fn collect_from_bind_source(source: &BindSource, consumed: &mut HashSet<String>) {
    match source {
        BindSource::Var(name) => {
            consumed.insert(name.clone());
        }
        BindSource::Lit(_) => {}
        BindSource::Field { of, .. } => {
            consumed.insert(of.clone());
        }
    }
}

fn collect_from_condition(kind: &ConditionKind, consumed: &mut HashSet<String>) {
    match kind {
        ConditionKind::Equals { value, .. }
        | ConditionKind::Contains { value, .. }
        | ConditionKind::NotEquals { value, .. } => {
            collect_from_input_source(value, consumed);
        }
        ConditionKind::And(left, right) | ConditionKind::Or(left, right) => {
            collect_from_condition(&left.kind, consumed);
            collect_from_condition(&right.kind, consumed);
        }
        ConditionKind::RelTo { .. } | ConditionKind::RelFrom { .. } => {}
    }
}

/// Recursively analyze nested steps in control flow constructs
fn analyze_nested_steps(kind: &StepKind, analysis: &mut UsageAnalysis) {
    match kind {
        StepKind::If(if_step) => {
            let nested = analyze_usage(&if_step.then_steps);
            merge_analysis(analysis, &nested);
            if let Some(else_steps) = &if_step.else_steps {
                let nested = analyze_usage(else_steps);
                merge_analysis(analysis, &nested);
            }
        }
        StepKind::Match(match_step) => {
            for case in &match_step.cases {
                let nested = analyze_usage(&case.steps);
                merge_analysis(analysis, &nested);
            }
        }
        StepKind::For(for_step) => {
            let nested = analyze_usage(&for_step.steps);
            merge_analysis(analysis, &nested);
        }
        StepKind::Transaction(txn) => {
            let nested = analyze_usage(&txn.steps);
            merge_analysis(analysis, &nested);
        }
        StepKind::Call(call) => {
            if let Some(handle) = &call.handle {
                for case in &handle.cases {
                    let nested = analyze_usage(&case.steps);
                    merge_analysis(analysis, &nested);
                }
            }
        }
        _ => {}
    }
}

/// Merge nested analysis into parent
fn merge_analysis(parent: &mut UsageAnalysis, nested: &UsageAnalysis) {
    for (binding, steps) in &nested.used_by {
        parent
            .used_by
            .entry(binding.clone())
            .or_default()
            .extend(steps.clone());
    }
    parent.produces.extend(nested.produces.clone());
    parent.consumes.extend(nested.consumes.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use covenant_ast::{BindStep, IfStep, Input, Literal, Operation, Span};

    fn make_span() -> Span {
        Span::dummy()
    }

    #[test]
    fn test_compute_step_usage() {
        let steps = vec![Step {
            id: "s1".into(),
            kind: StepKind::Compute(ComputeStep {
                op: Operation::Add,
                inputs: vec![
                    Input {
                        source: InputSource::Var("a".into()),
                        span: make_span(),
                    },
                    Input {
                        source: InputSource::Var("b".into()),
                        span: make_span(),
                    },
                ],
                span: make_span(),
            }),
            output_binding: "result".into(),
            span: make_span(),
        }];

        let analysis = analyze_usage(&steps);

        assert_eq!(analysis.produces.get("s1"), Some(&"result".to_string()));
        assert!(analysis.consumes.get("s1").unwrap().contains("a"));
        assert!(analysis.consumes.get("s1").unwrap().contains("b"));
        assert!(analysis.used_by.get("a").unwrap().contains("s1"));
        assert!(analysis.used_by.get("b").unwrap().contains("s1"));
    }

    #[test]
    fn test_return_step_usage() {
        let steps = vec![Step {
            id: "s1".into(),
            kind: StepKind::Return(ReturnStep {
                value: ReturnValue::Var("result".into()),
                span: make_span(),
            }),
            output_binding: "_".into(),
            span: make_span(),
        }];

        let analysis = analyze_usage(&steps);

        assert!(!analysis.produces.contains_key("s1")); // "_" is not tracked
        assert!(analysis.used_by.get("result").unwrap().contains("s1"));
    }

    #[test]
    fn test_unused_binding_detection() {
        let steps = vec![
            Step {
                id: "s1".into(),
                kind: StepKind::Bind(BindStep {
                    source: BindSource::Lit(Literal::Int(42)),
                    span: make_span(),
                }),
                output_binding: "unused".into(),
                span: make_span(),
            },
            Step {
                id: "s2".into(),
                kind: StepKind::Return(ReturnStep {
                    value: ReturnValue::Lit(Literal::Int(0)),
                    span: make_span(),
                }),
                output_binding: "_".into(),
                span: make_span(),
            },
        ];

        let analysis = analyze_usage(&steps);

        // "unused" is produced but never used
        assert_eq!(analysis.produces.get("s1"), Some(&"unused".to_string()));
        assert!(!analysis.used_by.contains_key("unused"));
    }

    #[test]
    fn test_nested_if_analysis() {
        let steps = vec![Step {
            id: "s1".into(),
            kind: StepKind::If(IfStep {
                condition: "cond".into(),
                then_steps: vec![Step {
                    id: "s1.1".into(),
                    kind: StepKind::Compute(ComputeStep {
                        op: Operation::Add,
                        inputs: vec![Input {
                            source: InputSource::Var("x".into()),
                            span: make_span(),
                        }],
                        span: make_span(),
                    }),
                    output_binding: "nested_result".into(),
                    span: make_span(),
                }],
                else_steps: None,
                span: make_span(),
            }),
            output_binding: "_".into(),
            span: make_span(),
        }];

        let analysis = analyze_usage(&steps);

        // Should track usage in nested steps
        assert!(analysis.used_by.get("cond").unwrap().contains("s1"));
        assert!(analysis.used_by.get("x").unwrap().contains("s1.1"));
        assert_eq!(
            analysis.produces.get("s1.1"),
            Some(&"nested_result".to_string())
        );
    }
}
