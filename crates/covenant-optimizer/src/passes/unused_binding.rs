//! Unused binding detection pass
//!
//! Detects bindings that are assigned but never read. Unlike dead_code.rs,
//! this pass focuses specifically on providing helpful warnings for unused
//! values, including those in effectful steps.

use covenant_ast::Step;

use crate::analysis::analyze_usage;
use crate::passes::dead_code::step_has_effects;
use crate::passes::{OptContext, OptWarning, OptimizationPass, PassResult};

/// Unused binding detection pass
///
/// Emits W-DEAD-001 warnings for bindings that are assigned but never read.
/// Distinguishes between effectful and pure steps in the warning message
/// to help developers understand why the step can't be removed.
pub struct UnusedBindingDetection;

impl OptimizationPass for UnusedBindingDetection {
    fn name(&self) -> &'static str {
        "unused-binding-detection"
    }

    fn run(&self, steps: &mut Vec<Step>, _ctx: &OptContext) -> PassResult {
        let usage = analyze_usage(steps);
        let mut warnings = vec![];

        for step in steps.iter() {
            // Skip discard bindings
            if step.output_binding == "_" {
                continue;
            }

            // Check if binding is ever used
            if !usage.used_by.contains_key(&step.output_binding) {
                let is_effectful = step_has_effects(&step.kind);

                let message = if is_effectful {
                    format!(
                        "Binding '{}' in step '{}' is assigned but never used. \
                         Consider using '_' if the result is intentionally discarded (effectful step preserved).",
                        step.output_binding, step.id
                    )
                } else {
                    format!(
                        "Binding '{}' in step '{}' is assigned but never used",
                        step.output_binding, step.id
                    )
                };

                warnings.push(OptWarning {
                    code: "W-DEAD-001",
                    message,
                    step_id: Some(step.id.clone()),
                });
            }
        }

        PassResult {
            modified: false, // This pass only warns, never modifies
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use covenant_ast::{BindSource, BindStep, CallStep, ReturnStep, ReturnValue, StepKind};
    use covenant_ast::{Literal, Span};

    fn make_span() -> Span {
        Span::dummy()
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

    fn make_call_step(id: &str, binding: &str) -> Step {
        Step {
            id: id.into(),
            kind: StepKind::Call(CallStep {
                fn_name: "some.function".into(),
                args: vec![],
                handle: None,
                span: make_span(),
            }),
            output_binding: binding.into(),
            span: make_span(),
        }
    }

    fn make_return_step(id: &str, var: &str) -> Step {
        Step {
            id: id.into(),
            kind: StepKind::Return(ReturnStep {
                value: ReturnValue::Var(var.into()),
                span: make_span(),
            }),
            output_binding: "_".into(),
            span: make_span(),
        }
    }

    fn make_ctx() -> OptContext {
        OptContext {
            settings: crate::passes::OptSettings {
                level: crate::passes::OptLevel::O1,
                emit_warnings: true,
            },
        }
    }

    #[test]
    fn test_unused_pure_binding() {
        let mut steps = vec![
            make_bind_step("s1", "unused"),
            make_return_step("s2", "something_else"),
        ];

        // Add something_else binding so we don't get a warning about missing binding
        steps.insert(0, make_bind_step("s0", "something_else"));

        let pass = UnusedBindingDetection;
        let result = pass.run(&mut steps, &make_ctx());

        assert!(result.warnings.iter().any(|w|
            w.code == "W-DEAD-001" &&
            w.message.contains("unused") &&
            !w.message.contains("effectful")
        ));
    }

    #[test]
    fn test_unused_effectful_binding() {
        let mut steps = vec![
            make_call_step("s1", "result"),
            make_return_step("s2", "something_else"),
        ];
        steps.insert(0, make_bind_step("s0", "something_else"));

        let pass = UnusedBindingDetection;
        let result = pass.run(&mut steps, &make_ctx());

        // Should warn but mention it's effectful
        assert!(result.warnings.iter().any(|w|
            w.code == "W-DEAD-001" &&
            w.message.contains("effectful")
        ));
    }

    #[test]
    fn test_used_binding_no_warning() {
        let mut steps = vec![
            make_bind_step("s1", "value"),
            make_return_step("s2", "value"),
        ];

        let pass = UnusedBindingDetection;
        let result = pass.run(&mut steps, &make_ctx());

        // No warnings - "value" is used
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_discard_binding_no_warning() {
        let mut steps = vec![
            make_call_step("s1", "_"),
            make_return_step("s2", "something"),
        ];
        steps.insert(0, make_bind_step("s0", "something"));

        let pass = UnusedBindingDetection;
        let result = pass.run(&mut steps, &make_ctx());

        // No warnings for "_" bindings
        assert!(!result.warnings.iter().any(|w| w.step_id == Some("s1".into())));
    }

    #[test]
    fn test_does_not_modify() {
        let mut steps = vec![make_bind_step("s1", "unused")];
        let original_len = steps.len();

        let pass = UnusedBindingDetection;
        let result = pass.run(&mut steps, &make_ctx());

        assert!(!result.modified);
        assert_eq!(steps.len(), original_len);
    }
}
