//! Dead code elimination pass
//!
//! Detects and optionally removes unreachable code (steps after unconditional returns)
//! and warns about unused bindings.

use covenant_ast::{Step, StepKind};

use crate::analysis::{compute_reachable, analyze_usage};
use crate::passes::{OptContext, OptLevel, OptWarning, OptimizationPass, PassResult};

/// Dead code elimination pass
///
/// Detects:
/// - Unreachable code after unconditional returns (W-DEAD-002)
/// - Unused bindings (W-DEAD-001) - only for non-effectful steps
///
/// At O2+, actually removes unreachable steps from the IR.
pub struct DeadCodeElimination;

impl OptimizationPass for DeadCodeElimination {
    fn name(&self) -> &'static str {
        "dead-code-elimination"
    }

    fn run(&self, steps: &mut Vec<Step>, ctx: &OptContext) -> PassResult {
        let reachable = compute_reachable(steps);
        let usage = analyze_usage(steps);
        let mut warnings = vec![];

        // 1. Flag unreachable steps (after unconditional return)
        for step in steps.iter() {
            if !reachable.contains(&step.id) {
                warnings.push(OptWarning {
                    code: "W-DEAD-002",
                    message: format!("Unreachable code: step '{}' is after a return", step.id),
                    step_id: Some(step.id.clone()),
                });
            }
        }

        // 2. Flag unused bindings (binding never read) - only for non-effectful pure steps
        for step in steps.iter() {
            // Skip discard bindings and unreachable steps
            if step.output_binding == "_" || !reachable.contains(&step.id) {
                continue;
            }

            // Check if binding is ever used
            if !usage.used_by.contains_key(&step.output_binding) && !step_has_effects(&step.kind) {
                warnings.push(OptWarning {
                    code: "W-DEAD-001",
                    message: format!(
                        "Unused binding '{}' in step '{}'",
                        step.output_binding, step.id
                    ),
                    step_id: Some(step.id.clone()),
                });
            }
        }

        // 3. Optionally remove unreachable steps at O2+
        let modified = if ctx.settings.level >= OptLevel::O2 {
            let original_len = steps.len();
            steps.retain(|s| reachable.contains(&s.id));
            steps.len() != original_len
        } else {
            false
        };

        PassResult { modified, warnings }
    }
}

/// Check if a step kind potentially has side effects
///
/// We're conservative here - calls might be effectful, queries/inserts/etc definitely are.
/// Pure compute, bind, and return steps have no effects.
pub fn step_has_effects(kind: &StepKind) -> bool {
    matches!(
        kind,
        StepKind::Call(_)
            | StepKind::Query(_)
            | StepKind::Insert(_)
            | StepKind::Update(_)
            | StepKind::Delete(_)
            | StepKind::Transaction(_)
            | StepKind::Traverse(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use covenant_ast::{BindSource, BindStep, ComputeStep, Operation, Input, InputSource, ReturnStep, ReturnValue};
    use covenant_ast::{Literal, Span};

    fn make_span() -> Span {
        Span::dummy()
    }

    fn make_return_step(id: &str) -> Step {
        Step {
            id: id.into(),
            kind: StepKind::Return(ReturnStep {
                value: ReturnValue::Lit(Literal::Int(0)),
                span: make_span(),
            }),
            output_binding: "_".into(),
            span: make_span(),
        }
    }

    fn make_bind_step(id: &str, binding: &str) -> Step {
        Step {
            id: id.into(),
            kind: StepKind::Bind(BindStep {
                source: BindSource::Lit(Literal::Int(42)),
                span: make_span(),
            }),
            output_binding: binding.into(),
            span: make_span(),
        }
    }

    fn make_compute_step(id: &str, binding: &str, input_var: &str) -> Step {
        Step {
            id: id.into(),
            kind: StepKind::Compute(ComputeStep {
                op: Operation::Add,
                inputs: vec![
                    Input {
                        source: InputSource::Var(input_var.into()),
                        span: make_span(),
                    },
                    Input {
                        source: InputSource::Lit(Literal::Int(1)),
                        span: make_span(),
                    },
                ],
                span: make_span(),
            }),
            output_binding: binding.into(),
            span: make_span(),
        }
    }

    fn make_ctx(level: OptLevel) -> OptContext {
        OptContext {
            settings: crate::passes::OptSettings {
                level,
                emit_warnings: true,
            },
        }
    }

    #[test]
    fn test_detects_unreachable_code() {
        let mut steps = vec![
            make_bind_step("s1", "a"),
            make_return_step("s2"),
            make_bind_step("s3", "b"),
        ];

        let pass = DeadCodeElimination;
        let result = pass.run(&mut steps, &make_ctx(OptLevel::O1));

        assert!(result.warnings.iter().any(|w| w.code == "W-DEAD-002" && w.step_id == Some("s3".into())));
        assert!(!result.modified); // O1 doesn't remove code
    }

    #[test]
    fn test_removes_unreachable_at_o2() {
        let mut steps = vec![
            make_bind_step("s1", "_"),
            make_return_step("s2"),
            make_bind_step("s3", "_"),
        ];

        let pass = DeadCodeElimination;
        let result = pass.run(&mut steps, &make_ctx(OptLevel::O2));

        assert!(result.modified);
        assert_eq!(steps.len(), 2);
        assert!(steps.iter().all(|s| s.id != "s3"));
    }

    #[test]
    fn test_detects_unused_binding() {
        let mut steps = vec![
            make_bind_step("s1", "unused"),
            make_return_step("s2"),
        ];

        let pass = DeadCodeElimination;
        let result = pass.run(&mut steps, &make_ctx(OptLevel::O1));

        assert!(result.warnings.iter().any(|w|
            w.code == "W-DEAD-001" &&
            w.message.contains("unused")
        ));
    }

    #[test]
    fn test_used_binding_no_warning() {
        let mut steps = vec![
            make_bind_step("s1", "value"),
            make_compute_step("s2", "result", "value"),
            Step {
                id: "s3".into(),
                kind: StepKind::Return(ReturnStep {
                    value: ReturnValue::Var("result".into()),
                    span: make_span(),
                }),
                output_binding: "_".into(),
                span: make_span(),
            },
        ];

        let pass = DeadCodeElimination;
        let result = pass.run(&mut steps, &make_ctx(OptLevel::O1));

        // No unused binding warnings - "value" is used by s2, "result" is used by s3
        assert!(!result.warnings.iter().any(|w| w.code == "W-DEAD-001"));
    }

    #[test]
    fn test_discard_binding_no_warning() {
        let mut steps = vec![
            make_bind_step("s1", "_"), // discard binding
            make_return_step("s2"),
        ];

        let pass = DeadCodeElimination;
        let result = pass.run(&mut steps, &make_ctx(OptLevel::O1));

        // No warnings for "_" bindings
        assert!(!result.warnings.iter().any(|w| w.code == "W-DEAD-001"));
    }
}
