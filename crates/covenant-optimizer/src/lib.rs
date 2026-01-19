//! IR Optimizer - Optimization passes for Covenant IR
//!
//! This crate provides optimization passes that transform the IR to improve
//! performance and detect potential issues like dead code.
//!
//! # Optimization Passes
//!
//! - **Dead Code Elimination**: Removes unreachable steps and warns about unused bindings
//! - **Constant Folding**: Evaluates constant expressions at compile time
//! - **Unused Binding Detection**: Warns about assigned-but-never-read bindings
//!
//! # Usage
//!
//! ```ignore
//! use covenant_optimizer::{optimize, OptSettings, OptLevel};
//! use covenant_ast::snippet::BodySection;
//!
//! let settings = OptSettings {
//!     level: OptLevel::O2,
//!     emit_warnings: true,
//! };
//! let result = optimize(&mut body.steps, &settings);
//! for warning in result.warnings {
//!     eprintln!("{}: {}", warning.code, warning.message);
//! }
//! ```

pub mod analysis;
pub mod passes;

pub use passes::{
    ConstantFolding, DeadCodeElimination, OptContext, OptLevel, OptSettings, OptWarning,
    OptimizationPass, PassResult, UnusedBindingDetection,
};

use covenant_ast::Step;

/// Result of running all optimization passes
#[derive(Debug, Clone, Default)]
pub struct OptResult {
    /// Whether any pass modified the IR
    pub modified: bool,
    /// All warnings from all passes
    pub warnings: Vec<OptWarning>,
}

/// Run all optimization passes based on the settings
///
/// # Arguments
/// * `steps` - The steps from a function body to optimize
/// * `settings` - Optimization settings controlling which passes run
///
/// # Returns
/// An `OptResult` containing whether the IR was modified and any warnings
pub fn optimize(steps: &mut Vec<Step>, settings: &OptSettings) -> OptResult {
    let ctx = OptContext {
        settings: settings.clone(),
    };

    let mut result = OptResult::default();

    // Select passes based on optimization level
    let passes: Vec<Box<dyn OptimizationPass>> = match settings.level {
        OptLevel::O0 => vec![],
        OptLevel::O1 => vec![
            Box::new(DeadCodeElimination),
            Box::new(UnusedBindingDetection),
        ],
        OptLevel::O2 | OptLevel::O3 => vec![
            Box::new(ConstantFolding),
            Box::new(DeadCodeElimination),
            Box::new(UnusedBindingDetection),
        ],
    };

    // Run each pass in sequence
    for pass in passes {
        let pass_result = pass.run(steps, &ctx);
        result.modified |= pass_result.modified;
        if settings.emit_warnings {
            result.warnings.extend(pass_result.warnings);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use covenant_ast::{
        BindSource, BindStep, ComputeStep, Input, InputSource, Operation, ReturnStep, ReturnValue,
        StepKind,
    };
    use covenant_ast::{Literal, Span};

    fn make_span() -> Span {
        Span::dummy()
    }

    fn make_bind_step(id: &str, binding: &str, value: i64) -> Step {
        Step {
            id: id.into(),
            kind: StepKind::Bind(BindStep {
                source: BindSource::Lit(Literal::Int(value)),
                span: make_span(),
            }),
            output_binding: binding.into(),
            span: make_span(),
        }
    }

    fn make_compute_step(id: &str, binding: &str, op: Operation, lits: Vec<i64>) -> Step {
        Step {
            id: id.into(),
            kind: StepKind::Compute(ComputeStep {
                op,
                inputs: lits
                    .into_iter()
                    .map(|v| Input {
                        source: InputSource::Lit(Literal::Int(v)),
                        span: make_span(),
                    })
                    .collect(),
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

    fn make_return_lit_step(id: &str) -> Step {
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

    #[test]
    fn test_o0_no_optimization() {
        let mut steps = vec![
            make_compute_step("s1", "x", Operation::Add, vec![1, 2]),
            make_return_lit_step("s2"),
            make_bind_step("s3", "dead", 42), // unreachable
        ];

        let settings = OptSettings {
            level: OptLevel::O0,
            emit_warnings: true,
        };

        let result = optimize(&mut steps, &settings);

        assert!(!result.modified);
        assert!(result.warnings.is_empty());
        assert_eq!(steps.len(), 3); // Nothing removed
    }

    #[test]
    fn test_o1_detects_but_does_not_remove() {
        let mut steps = vec![
            make_bind_step("s1", "_", 1),
            make_return_lit_step("s2"),
            make_bind_step("s3", "_", 42), // unreachable
        ];

        let settings = OptSettings {
            level: OptLevel::O1,
            emit_warnings: true,
        };

        let result = optimize(&mut steps, &settings);

        assert!(!result.modified); // O1 doesn't modify
        assert!(result.warnings.iter().any(|w| w.code == "W-DEAD-002"));
        assert_eq!(steps.len(), 3); // Still there
    }

    #[test]
    fn test_o2_removes_unreachable() {
        let mut steps = vec![
            make_bind_step("s1", "_", 1),
            make_return_lit_step("s2"),
            make_bind_step("s3", "_", 42), // unreachable - will be removed
        ];

        let settings = OptSettings {
            level: OptLevel::O2,
            emit_warnings: true,
        };

        let result = optimize(&mut steps, &settings);

        assert!(result.modified);
        assert_eq!(steps.len(), 2);
        assert!(steps.iter().all(|s| s.id != "s3"));
    }

    #[test]
    fn test_o2_folds_constants() {
        let mut steps = vec![
            make_compute_step("s1", "result", Operation::Add, vec![2, 3]),
            make_return_step("s2", "result"),
        ];

        let settings = OptSettings {
            level: OptLevel::O2,
            emit_warnings: true,
        };

        let result = optimize(&mut steps, &settings);

        assert!(result.modified);
        // s1 should now be a Bind with Lit(5)
        match &steps[0].kind {
            StepKind::Bind(bind) => match &bind.source {
                BindSource::Lit(Literal::Int(5)) => {}
                other => panic!("Expected Int(5), got {:?}", other),
            },
            other => panic!("Expected Bind, got {:?}", other),
        }
    }

    #[test]
    fn test_unused_binding_warning() {
        let mut steps = vec![
            make_bind_step("s1", "unused_value", 42),
            make_return_lit_step("s2"),
        ];

        let settings = OptSettings {
            level: OptLevel::O1,
            emit_warnings: true,
        };

        let result = optimize(&mut steps, &settings);

        assert!(result.warnings.iter().any(|w| w.code == "W-DEAD-001"
            && w.message.contains("unused_value")));
    }

    #[test]
    fn test_used_binding_no_warning() {
        let mut steps = vec![
            make_bind_step("s1", "value", 42),
            make_return_step("s2", "value"),
        ];

        let settings = OptSettings {
            level: OptLevel::O1,
            emit_warnings: true,
        };

        let result = optimize(&mut steps, &settings);

        // No unused binding warnings
        assert!(!result.warnings.iter().any(|w| w.code == "W-DEAD-001"));
    }

    #[test]
    fn test_warnings_disabled() {
        let mut steps = vec![
            make_bind_step("s1", "unused", 42),
            make_return_lit_step("s2"),
            make_bind_step("s3", "_", 99), // unreachable
        ];

        let settings = OptSettings {
            level: OptLevel::O1,
            emit_warnings: false,
        };

        let result = optimize(&mut steps, &settings);

        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_combined_optimizations() {
        // This test verifies that multiple passes work together:
        // 1. Constant folding happens first
        // 2. Dead code elimination detects unused bindings and unreachable code
        let mut steps = vec![
            make_compute_step("s1", "constant", Operation::Add, vec![10, 20]), // Will be folded to 30
            make_bind_step("s2", "unused", 5), // Will trigger unused binding warning
            make_return_step("s3", "constant"),
            make_bind_step("s4", "_", 99), // Unreachable, will be removed at O2
        ];

        let settings = OptSettings {
            level: OptLevel::O2,
            emit_warnings: true,
        };

        let result = optimize(&mut steps, &settings);

        // Should have modified (constant folding + dead code removal)
        assert!(result.modified);

        // Should have warnings for unused binding and unreachable code
        assert!(result.warnings.iter().any(|w| w.code == "W-DEAD-001")); // unused
        assert!(result.warnings.iter().any(|w| w.code == "W-DEAD-002")); // unreachable

        // s4 should be removed
        assert_eq!(steps.len(), 3);

        // s1 should be folded to a literal
        match &steps[0].kind {
            StepKind::Bind(bind) => match &bind.source {
                BindSource::Lit(Literal::Int(30)) => {}
                other => panic!("Expected Int(30), got {:?}", other),
            },
            other => panic!("Expected Bind after folding, got {:?}", other),
        }
    }
}
