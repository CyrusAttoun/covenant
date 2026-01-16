//! covenant-symbols: Symbol Graph Builder (Phase 2)
//!
//! This crate implements Phase 2 of the Covenant compiler:
//! - Extract symbols and forward references from parsed snippets
//! - Compute backward references (called_by, referenced_by)
//! - Validate invariants I1, I4, I5
//! - Detect and report errors E-SYMBOL-001/002/003
//!
//! # Example
//!
//! ```ignore
//! use covenant_parser::parse;
//! use covenant_symbols::build_symbol_graph;
//!
//! let source = r#"
//! snippet id="math.add" kind="fn"
//!   signature
//!     fn name="add"
//!       param name="a" type="Int"
//!       param name="b" type="Int"
//!       returns type="Int"
//!     end
//!   end
//! end
//! "#;
//!
//! let program = parse(source).unwrap();
//! let result = build_symbol_graph(&program).unwrap();
//! assert!(result.graph.contains("math.add"));
//! ```

mod cycle;
mod error;
mod extractor;
mod graph;
mod resolver;
mod symbol;

pub use error::SymbolError;
pub use graph::{InvariantStatus, SymbolGraph, SymbolResult};
pub use symbol::{RelationRef, SymbolId, SymbolInfo, SymbolKind};

use covenant_ast::{Program, Snippet};
use cycle::CycleDetector;
use extractor::SymbolExtractor;
use resolver::BackwardResolver;

/// Build a symbol graph from a parsed program
///
/// This is the main entry point for Phase 2.
///
/// # Arguments
/// * `program` - The parsed AST from Phase 1
///
/// # Returns
/// * `Ok(SymbolResult)` - Symbol graph with deferred errors (if any)
/// * `Err(Vec<SymbolError>)` - Hard errors that block compilation
pub fn build_symbol_graph(program: &Program) -> Result<SymbolResult, Vec<SymbolError>> {
    let snippets = match program {
        Program::Snippets { snippets, .. } => snippets,
        Program::Legacy { .. } => {
            // Legacy mode not supported for symbol graph building
            return Ok(SymbolResult {
                graph: SymbolGraph::new(),
                deferred_errors: Vec::new(),
            });
        }
    };

    build_from_snippets(snippets)
}

/// Build a symbol graph directly from snippets
pub fn build_from_snippets(snippets: &[Snippet]) -> Result<SymbolResult, Vec<SymbolError>> {
    // Pass 1: Extract symbols and forward references
    let extractor = SymbolExtractor::new();
    let (mut graph, mut all_errors) = extractor.extract(snippets);

    // Check for hard errors from extraction (duplicate IDs)
    let hard_errors: Vec<_> = all_errors
        .iter()
        .filter(|e| e.is_hard_error())
        .cloned()
        .collect();
    if !hard_errors.is_empty() {
        return Err(hard_errors);
    }

    // Pass 2: Compute backward references
    let resolution_errors = BackwardResolver::resolve(&mut graph);
    all_errors.extend(resolution_errors);

    // Validate I4: Acyclicity
    let cycle_errors = CycleDetector::detect_cycles(&graph);
    if !cycle_errors.is_empty() {
        // Cycles are hard errors
        return Err(cycle_errors);
    }

    // Validate I1: Bidirectionality (should always pass if resolver works correctly)
    let i1_valid = validate_bidirectionality(&graph);

    // Validate I5: Relation bidirectionality
    let i5_valid = validate_relation_bidirectionality(&graph);

    // Update invariant status
    graph.invariants = InvariantStatus {
        i1_bidirectionality: i1_valid,
        i4_acyclicity: true, // Passed if we got here
        i5_relation_bidirectionality: i5_valid,
    };

    // Separate soft errors (deferred) from hard errors
    let (deferred, hard): (Vec<_>, Vec<_>) =
        all_errors.into_iter().partition(|e| !e.is_hard_error());

    if !hard.is_empty() {
        return Err(hard);
    }

    Ok(SymbolResult {
        graph,
        deferred_errors: deferred,
    })
}

/// Validate I1: If A calls B, B's called_by includes A
fn validate_bidirectionality(graph: &SymbolGraph) -> bool {
    for symbol in graph.iter() {
        for callee_name in &symbol.calls {
            if let Some(callee) = graph.get_by_name(callee_name) {
                if !callee.called_by.contains(&symbol.id) {
                    return false;
                }
            }
            // If callee doesn't exist, that's a separate error (undefined reference)
        }
    }
    true
}

/// Validate I5: Relations have proper inverses
fn validate_relation_bidirectionality(graph: &SymbolGraph) -> bool {
    for symbol in graph.iter() {
        for rel in &symbol.relations_to {
            if let Some(target) = graph.get_by_name(&rel.target) {
                let has_inverse = target
                    .relations_from
                    .iter()
                    .any(|r| r.target == symbol.name);
                if !has_inverse {
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use covenant_ast::Span;

    fn make_span() -> Span {
        Span::dummy()
    }

    #[test]
    fn test_empty_snippets() {
        let result = build_from_snippets(&[]).unwrap();
        assert!(result.graph.is_empty());
        assert!(result.deferred_errors.is_empty());
    }

    #[test]
    fn test_single_symbol() {
        let snippet = Snippet {
            id: "test.foo".into(),
            kind: covenant_ast::SnippetKind::Function,
            notes: vec![],
            sections: vec![],
            span: make_span(),
        };

        let result = build_from_snippets(&[snippet]).unwrap();
        assert_eq!(result.graph.len(), 1);
        assert!(result.graph.contains("test.foo"));
    }

    #[test]
    fn test_duplicate_id_error() {
        let snippet1 = Snippet {
            id: "test.foo".into(),
            kind: covenant_ast::SnippetKind::Function,
            notes: vec![],
            sections: vec![],
            span: make_span(),
        };

        let snippet2 = Snippet {
            id: "test.foo".into(), // Duplicate
            kind: covenant_ast::SnippetKind::Function,
            notes: vec![],
            sections: vec![],
            span: make_span(),
        };

        let result = build_from_snippets(&[snippet1, snippet2]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, SymbolError::DuplicateId { .. })));
    }

    #[test]
    fn test_graph_symbol_info() {
        let snippet = Snippet {
            id: "mymod.myfn".into(),
            kind: covenant_ast::SnippetKind::Function,
            notes: vec![],
            sections: vec![],
            span: make_span(),
        };

        let result = build_from_snippets(&[snippet]).unwrap();
        let symbol = result.graph.get_by_name("mymod.myfn").unwrap();

        assert_eq!(symbol.name, "mymod.myfn");
        assert_eq!(symbol.kind, SymbolKind::Function);
    }

    #[test]
    fn test_invariants_validated() {
        let snippet = Snippet {
            id: "test.fn".into(),
            kind: covenant_ast::SnippetKind::Function,
            notes: vec![],
            sections: vec![],
            span: make_span(),
        };

        let result = build_from_snippets(&[snippet]).unwrap();
        assert!(result.graph.invariants.i1_bidirectionality);
        assert!(result.graph.invariants.i4_acyclicity);
        assert!(result.graph.invariants.i5_relation_bidirectionality);
    }
}
