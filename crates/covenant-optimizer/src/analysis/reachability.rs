//! Reachability analysis
//!
//! Determines which steps are reachable based on control flow. Steps after
//! an unconditional return are marked as unreachable.

use std::collections::HashSet;

use covenant_ast::{Step, StepKind};

/// Compute the set of reachable step IDs
///
/// A step is unreachable if it comes after an unconditional return statement.
/// Control flow through if/match is analyzed to detect when all branches return.
pub fn compute_reachable(steps: &[Step]) -> HashSet<String> {
    let mut reachable = HashSet::new();

    for step in steps {
        reachable.insert(step.id.clone());

        // Check if this step definitely terminates (unconditional return)
        if definitely_returns(&step.kind) {
            // All subsequent steps are unreachable
            break;
        }
    }

    // Also mark nested steps as reachable
    for step in steps {
        if reachable.contains(&step.id) {
            mark_nested_reachable(&step.kind, &mut reachable);
        }
    }

    reachable
}

/// Check if a step kind definitely returns (terminates the function)
fn definitely_returns(kind: &StepKind) -> bool {
    match kind {
        StepKind::Return(_) => true,
        StepKind::If(if_step) => {
            // If returns definitely only if BOTH branches return
            let then_returns = branch_returns(&if_step.then_steps);
            let else_returns = if_step
                .else_steps
                .as_ref()
                .map(|steps| branch_returns(steps))
                .unwrap_or(false);
            then_returns && else_returns
        }
        StepKind::Match(match_step) => {
            // Match returns definitely only if ALL cases return
            match_step
                .cases
                .iter()
                .all(|case| branch_returns(&case.steps))
        }
        _ => false,
    }
}

/// Check if a branch (sequence of steps) definitely returns
fn branch_returns(steps: &[Step]) -> bool {
    steps.iter().any(|s| definitely_returns(&s.kind))
}

/// Mark nested steps as reachable
fn mark_nested_reachable(kind: &StepKind, reachable: &mut HashSet<String>) {
    match kind {
        StepKind::If(if_step) => {
            mark_branch_reachable(&if_step.then_steps, reachable);
            if let Some(else_steps) = &if_step.else_steps {
                mark_branch_reachable(else_steps, reachable);
            }
        }
        StepKind::Match(match_step) => {
            for case in &match_step.cases {
                mark_branch_reachable(&case.steps, reachable);
            }
        }
        StepKind::For(for_step) => {
            mark_branch_reachable(&for_step.steps, reachable);
        }
        StepKind::Transaction(txn) => {
            mark_branch_reachable(&txn.steps, reachable);
        }
        StepKind::Call(call) => {
            if let Some(handle) = &call.handle {
                for case in &handle.cases {
                    mark_branch_reachable(&case.steps, reachable);
                }
            }
        }
        _ => {}
    }
}

/// Mark steps in a branch as reachable (until a return)
fn mark_branch_reachable(steps: &[Step], reachable: &mut HashSet<String>) {
    for step in steps {
        reachable.insert(step.id.clone());
        mark_nested_reachable(&step.kind, reachable);

        if definitely_returns(&step.kind) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use covenant_ast::{BindSource, BindStep, IfStep, MatchCase, MatchPattern, MatchStep, ReturnStep, ReturnValue};
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

    #[test]
    fn test_all_reachable_no_return() {
        let steps = vec![
            make_bind_step("s1", "a"),
            make_bind_step("s2", "b"),
            make_bind_step("s3", "c"),
        ];

        let reachable = compute_reachable(&steps);

        assert!(reachable.contains("s1"));
        assert!(reachable.contains("s2"));
        assert!(reachable.contains("s3"));
    }

    #[test]
    fn test_unreachable_after_return() {
        let steps = vec![
            make_bind_step("s1", "a"),
            make_return_step("s2"),
            make_bind_step("s3", "b"), // unreachable
            make_bind_step("s4", "c"), // unreachable
        ];

        let reachable = compute_reachable(&steps);

        assert!(reachable.contains("s1"));
        assert!(reachable.contains("s2"));
        assert!(!reachable.contains("s3"));
        assert!(!reachable.contains("s4"));
    }

    #[test]
    fn test_if_both_branches_return() {
        let steps = vec![
            Step {
                id: "s1".into(),
                kind: StepKind::If(IfStep {
                    condition: "cond".into(),
                    then_steps: vec![make_return_step("s1.1")],
                    else_steps: Some(vec![make_return_step("s1.2")]),
                    span: make_span(),
                }),
                output_binding: "_".into(),
                span: make_span(),
            },
            make_bind_step("s2", "x"), // unreachable - both branches return
        ];

        let reachable = compute_reachable(&steps);

        assert!(reachable.contains("s1"));
        assert!(reachable.contains("s1.1"));
        assert!(reachable.contains("s1.2"));
        assert!(!reachable.contains("s2")); // both branches return
    }

    #[test]
    fn test_if_one_branch_returns() {
        let steps = vec![
            Step {
                id: "s1".into(),
                kind: StepKind::If(IfStep {
                    condition: "cond".into(),
                    then_steps: vec![make_return_step("s1.1")],
                    else_steps: Some(vec![make_bind_step("s1.2", "y")]),
                    span: make_span(),
                }),
                output_binding: "_".into(),
                span: make_span(),
            },
            make_bind_step("s2", "x"), // reachable - else branch doesn't return
        ];

        let reachable = compute_reachable(&steps);

        assert!(reachable.contains("s1"));
        assert!(reachable.contains("s2")); // reachable through else
    }

    #[test]
    fn test_if_no_else() {
        let steps = vec![
            Step {
                id: "s1".into(),
                kind: StepKind::If(IfStep {
                    condition: "cond".into(),
                    then_steps: vec![make_return_step("s1.1")],
                    else_steps: None, // no else = fall through
                    span: make_span(),
                }),
                output_binding: "_".into(),
                span: make_span(),
            },
            make_bind_step("s2", "x"), // reachable - might skip then branch
        ];

        let reachable = compute_reachable(&steps);

        assert!(reachable.contains("s1"));
        assert!(reachable.contains("s2")); // reachable when condition is false
    }

    #[test]
    fn test_match_all_branches_return() {
        let steps = vec![
            Step {
                id: "s1".into(),
                kind: StepKind::Match(MatchStep {
                    on: "value".into(),
                    cases: vec![
                        MatchCase {
                            pattern: MatchPattern::Variant {
                                variant: "A".into(),
                                bindings: vec![],
                            },
                            steps: vec![make_return_step("s1.1")],
                            span: make_span(),
                        },
                        MatchCase {
                            pattern: MatchPattern::Wildcard,
                            steps: vec![make_return_step("s1.2")],
                            span: make_span(),
                        },
                    ],
                    span: make_span(),
                }),
                output_binding: "_".into(),
                span: make_span(),
            },
            make_bind_step("s2", "x"), // unreachable - all cases return
        ];

        let reachable = compute_reachable(&steps);

        assert!(reachable.contains("s1"));
        assert!(!reachable.contains("s2"));
    }

    #[test]
    fn test_match_one_branch_no_return() {
        let steps = vec![
            Step {
                id: "s1".into(),
                kind: StepKind::Match(MatchStep {
                    on: "value".into(),
                    cases: vec![
                        MatchCase {
                            pattern: MatchPattern::Variant {
                                variant: "A".into(),
                                bindings: vec![],
                            },
                            steps: vec![make_return_step("s1.1")],
                            span: make_span(),
                        },
                        MatchCase {
                            pattern: MatchPattern::Wildcard,
                            steps: vec![make_bind_step("s1.2", "y")], // no return
                            span: make_span(),
                        },
                    ],
                    span: make_span(),
                }),
                output_binding: "_".into(),
                span: make_span(),
            },
            make_bind_step("s2", "x"), // reachable
        ];

        let reachable = compute_reachable(&steps);

        assert!(reachable.contains("s1"));
        assert!(reachable.contains("s2"));
    }
}
