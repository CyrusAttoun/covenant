//! Cycle detection for the call graph
//!
//! Validates I4 (acyclicity) - no circular calls between different functions.
//! Self-recursion (a function calling itself) IS allowed.
//! Mutual recursion (A calls B, B calls A) is NOT allowed.

use crate::{SymbolError, SymbolGraph, SymbolId};
use std::collections::HashMap;

/// Visit state for DFS cycle detection
#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    NotVisited,
    InProgress,
    Completed,
}

/// Detects cycles in the call graph (validates I4)
///
/// Note: Self-recursion is allowed. Only cycles involving multiple
/// different functions are reported as errors.
pub struct CycleDetector;

impl CycleDetector {
    /// Check for circular calls in the symbol graph
    /// Returns errors for each cycle detected (excluding self-recursion)
    pub fn detect_cycles(graph: &SymbolGraph) -> Vec<SymbolError> {
        let mut errors = Vec::new();
        let mut state: HashMap<SymbolId, VisitState> = HashMap::new();
        let mut path: Vec<SymbolId> = Vec::new();

        // Initialize all symbols as not visited
        for symbol in graph.iter() {
            state.insert(symbol.id, VisitState::NotVisited);
        }

        // DFS from each unvisited symbol
        for symbol in graph.iter() {
            if state[&symbol.id] == VisitState::NotVisited {
                if let Some(cycle) = Self::visit(graph, symbol.id, &mut state, &mut path) {
                    errors.push(cycle);
                }
            }
        }

        errors
    }

    /// DFS visit, returns Some(error) if cycle detected
    fn visit(
        graph: &SymbolGraph,
        node: SymbolId,
        state: &mut HashMap<SymbolId, VisitState>,
        path: &mut Vec<SymbolId>,
    ) -> Option<SymbolError> {
        state.insert(node, VisitState::InProgress);
        path.push(node);

        let symbol = graph.get(node)?;
        let node_span = symbol.span;

        // Check all callees (only resolved ones - unresolved are handled separately)
        for callee_name in &symbol.calls {
            if let Some(callee_id) = graph.id_of(callee_name) {
                // Skip self-recursion - it's allowed
                if callee_id == node {
                    continue;
                }

                match state.get(&callee_id) {
                    Some(VisitState::InProgress) => {
                        // Cycle detected! Build the cycle path
                        let cycle_start = path.iter().position(|&id| id == callee_id)?;
                        let cycle_path: Vec<String> = path[cycle_start..]
                            .iter()
                            .filter_map(|id| graph.get(*id).map(|s| s.name.clone()))
                            .collect();

                        let cycle_str = format!(
                            "{} -> {}",
                            cycle_path.join(" -> "),
                            graph.get(callee_id).map(|s| s.name.as_str()).unwrap_or("?")
                        );

                        return Some(SymbolError::CircularImport {
                            cycle: cycle_str,
                            span: node_span,
                        });
                    }
                    Some(VisitState::NotVisited) => {
                        if let Some(err) = Self::visit(graph, callee_id, state, path) {
                            return Some(err);
                        }
                    }
                    _ => {} // Already completed, no cycle through this path
                }
            }
            // Unresolved references are skipped - they'll be caught as E-SYMBOL-001
        }

        path.pop();
        state.insert(node, VisitState::Completed);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SymbolInfo, SymbolKind};
    use covenant_ast::Span;

    fn make_span() -> Span {
        Span::dummy()
    }

    #[test]
    fn test_no_cycle() {
        let mut graph = SymbolGraph::new();

        let mut a = SymbolInfo::new("a".into(), SymbolKind::Function, make_span());
        a.calls.insert("b".into());

        let b = SymbolInfo::new("b".into(), SymbolKind::Function, make_span());

        graph.insert(a).unwrap();
        graph.insert(b).unwrap();

        let errors = CycleDetector::detect_cycles(&graph);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_self_recursion_allowed() {
        // Self-recursion (a function calling itself) should be allowed
        let mut graph = SymbolGraph::new();

        let mut a = SymbolInfo::new("a".into(), SymbolKind::Function, make_span());
        a.calls.insert("a".into()); // Self-reference

        graph.insert(a).unwrap();

        let errors = CycleDetector::detect_cycles(&graph);
        assert!(errors.is_empty(), "Self-recursion should be allowed");
    }

    #[test]
    fn test_two_node_cycle_rejected() {
        // Mutual recursion (A calls B, B calls A) should NOT be allowed
        let mut graph = SymbolGraph::new();

        let mut a = SymbolInfo::new("a".into(), SymbolKind::Function, make_span());
        a.calls.insert("b".into());

        let mut b = SymbolInfo::new("b".into(), SymbolKind::Function, make_span());
        b.calls.insert("a".into());

        graph.insert(a).unwrap();
        graph.insert(b).unwrap();

        let errors = CycleDetector::detect_cycles(&graph);
        assert_eq!(errors.len(), 1, "Mutual recursion should be rejected");
    }

    #[test]
    fn test_three_node_cycle_rejected() {
        // Longer cycles (A -> B -> C -> A) should NOT be allowed
        let mut graph = SymbolGraph::new();

        let mut a = SymbolInfo::new("a".into(), SymbolKind::Function, make_span());
        a.calls.insert("b".into());

        let mut b = SymbolInfo::new("b".into(), SymbolKind::Function, make_span());
        b.calls.insert("c".into());

        let mut c = SymbolInfo::new("c".into(), SymbolKind::Function, make_span());
        c.calls.insert("a".into());

        graph.insert(a).unwrap();
        graph.insert(b).unwrap();
        graph.insert(c).unwrap();

        let errors = CycleDetector::detect_cycles(&graph);
        assert_eq!(errors.len(), 1, "Three-node cycle should be rejected");
        let cycle = match &errors[0] {
            SymbolError::CircularImport { cycle, .. } => cycle,
            _ => panic!("Expected CircularImport error"),
        };
        assert!(cycle.contains("a") && cycle.contains("b") && cycle.contains("c"));
    }

    #[test]
    fn test_self_recursion_with_other_calls() {
        // A function can call itself AND call other functions
        let mut graph = SymbolGraph::new();

        let mut a = SymbolInfo::new("a".into(), SymbolKind::Function, make_span());
        a.calls.insert("a".into()); // Self-recursion
        a.calls.insert("b".into()); // Also calls b

        let b = SymbolInfo::new("b".into(), SymbolKind::Function, make_span());

        graph.insert(a).unwrap();
        graph.insert(b).unwrap();

        let errors = CycleDetector::detect_cycles(&graph);
        assert!(errors.is_empty(), "Self-recursion with other calls should be allowed");
    }
}
