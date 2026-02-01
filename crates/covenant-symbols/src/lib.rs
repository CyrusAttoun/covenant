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
            implements: None,
            platform: None,
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
            implements: None,
            platform: None,
            span: make_span(),
        };

        let snippet2 = Snippet {
            id: "test.foo".into(), // Duplicate
            kind: covenant_ast::SnippetKind::Function,
            notes: vec![],
            sections: vec![],
            implements: None,
            platform: None,
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
            implements: None,
            platform: None,
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
            implements: None,
            platform: None,
            span: make_span(),
        };

        let result = build_from_snippets(&[snippet]).unwrap();
        assert!(result.graph.invariants.i1_bidirectionality);
        assert!(result.graph.invariants.i4_acyclicity);
        assert!(result.graph.invariants.i5_relation_bidirectionality);
    }

    #[test]
    fn test_struct_symbol() {
        let snippet = Snippet {
            id: "types.User".into(),
            kind: covenant_ast::SnippetKind::Struct,
            notes: vec![],
            sections: vec![],
            implements: None,
            platform: None,
            span: make_span(),
        };

        let result = build_from_snippets(&[snippet]).unwrap();
        let symbol = result.graph.get_by_name("types.User").unwrap();
        assert_eq!(symbol.kind, SymbolKind::Struct);
    }

    #[test]
    fn test_enum_symbol() {
        let snippet = Snippet {
            id: "types.Result".into(),
            kind: covenant_ast::SnippetKind::Enum,
            notes: vec![],
            sections: vec![],
            implements: None,
            platform: None,
            span: make_span(),
        };

        let result = build_from_snippets(&[snippet]).unwrap();
        let symbol = result.graph.get_by_name("types.Result").unwrap();
        assert_eq!(symbol.kind, SymbolKind::Enum);
    }

    #[test]
    fn test_extern_symbol() {
        let snippet = Snippet {
            id: "io.print".into(),
            kind: covenant_ast::SnippetKind::Extern,
            notes: vec![],
            sections: vec![],
            implements: None,
            platform: None,
            span: make_span(),
        };

        let result = build_from_snippets(&[snippet]).unwrap();
        let symbol = result.graph.get_by_name("io.print").unwrap();
        assert_eq!(symbol.kind, SymbolKind::Extern);
    }

    #[test]
    fn test_multiple_different_kinds() {
        let fn_snippet = Snippet {
            id: "app.main".into(),
            kind: covenant_ast::SnippetKind::Function,
            notes: vec![],
            sections: vec![],
            implements: None,
            platform: None,
            span: make_span(),
        };

        let struct_snippet = Snippet {
            id: "types.User".into(),
            kind: covenant_ast::SnippetKind::Struct,
            notes: vec![],
            sections: vec![],
            implements: None,
            platform: None,
            span: make_span(),
        };

        let extern_snippet = Snippet {
            id: "io.print".into(),
            kind: covenant_ast::SnippetKind::Extern,
            notes: vec![],
            sections: vec![],
            implements: None,
            platform: None,
            span: make_span(),
        };

        let result = build_from_snippets(&[fn_snippet, struct_snippet, extern_snippet]).unwrap();
        assert_eq!(result.graph.len(), 3);
        assert!(result.graph.contains("app.main"));
        assert!(result.graph.contains("types.User"));
        assert!(result.graph.contains("io.print"));
    }

    // ==========================================================================
    // COMPREHENSIVE PHASE 2 TESTS - Invariants, Cycles, Relations
    // ==========================================================================

    /// Helper to parse source and build symbol graph
    fn build_graph_from_source(source: &str) -> Result<SymbolResult, Vec<SymbolError>> {
        let program = covenant_parser::parse(source).expect("parse failed");
        build_symbol_graph(&program)
    }

    // === I1 Bidirectionality Tests ===

    #[test]
    fn test_i1_bidirectionality_simple_call() {
        // If A calls B, then B.called_by contains A
        let source = r#"
snippet id="b" kind="fn"
signature
  fn name="b"
    returns type="Int"
  end
end
body
  step id="s1" kind="return"
    lit=1
    as="_"
  end
end
end

snippet id="a" kind="fn"
signature
  fn name="a"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="b"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source).expect("should build graph");

        // Verify I1 invariant holds
        assert!(result.graph.invariants.i1_bidirectionality);

        // Verify explicitly
        let a = result.graph.get_by_name("a").expect("a exists");
        let b = result.graph.get_by_name("b").expect("b exists");

        assert!(a.calls.contains("b"), "a.calls should contain b");
        assert!(b.called_by.contains(&a.id), "b.called_by should contain a's id");
    }

    #[test]
    fn test_i1_bidirectionality_chain() {
        // A -> B -> C: verify all backward refs are correct
        let source = r#"
snippet id="c" kind="fn"
signature
  fn name="c"
    returns type="Int"
  end
end
body
  step id="s1" kind="return"
    lit=1
    as="_"
  end
end
end

snippet id="b" kind="fn"
signature
  fn name="b"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="c"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end

snippet id="a" kind="fn"
signature
  fn name="a"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="b"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source).expect("should build graph");
        assert!(result.graph.invariants.i1_bidirectionality);

        let a = result.graph.get_by_name("a").unwrap();
        let b = result.graph.get_by_name("b").unwrap();
        let c = result.graph.get_by_name("c").unwrap();

        // a calls b, b calls c
        assert!(a.calls.contains("b"));
        assert!(b.calls.contains("c"));

        // b.called_by has a, c.called_by has b
        assert!(b.called_by.contains(&a.id));
        assert!(c.called_by.contains(&b.id));

        // c is not called by a directly
        assert!(!c.called_by.contains(&a.id));
    }

    #[test]
    fn test_i1_multiple_callers() {
        // Multiple functions call the same target
        let source = r#"
snippet id="target" kind="fn"
signature
  fn name="target"
    returns type="Int"
  end
end
body
  step id="s1" kind="return"
    lit=1
    as="_"
  end
end
end

snippet id="caller1" kind="fn"
signature
  fn name="caller1"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="target"
    as="r"
  end
  step id="s2" kind="return"
    from="r"
    as="_"
  end
end
end

snippet id="caller2" kind="fn"
signature
  fn name="caller2"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="target"
    as="r"
  end
  step id="s2" kind="return"
    from="r"
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source).expect("should build graph");
        assert!(result.graph.invariants.i1_bidirectionality);

        let target = result.graph.get_by_name("target").unwrap();
        let caller1 = result.graph.get_by_name("caller1").unwrap();
        let caller2 = result.graph.get_by_name("caller2").unwrap();

        // target.called_by should have both callers
        assert!(target.called_by.contains(&caller1.id));
        assert!(target.called_by.contains(&caller2.id));
        assert_eq!(target.called_by.len(), 2);
    }

    // === I4 Cycle Detection Tests ===

    #[test]
    fn test_i4_direct_cycle_detected() {
        // A -> B -> A is a cycle
        let source = r#"
snippet id="a" kind="fn"
signature
  fn name="a"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="b"
    as="r"
  end
  step id="s2" kind="return"
    from="r"
    as="_"
  end
end
end

snippet id="b" kind="fn"
signature
  fn name="b"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="a"
    as="r"
  end
  step id="s2" kind="return"
    from="r"
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source);
        assert!(result.is_err(), "Cycle should be detected as error");

        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| matches!(e, SymbolError::CircularImport { .. })),
            "Should have CircularImport error"
        );
    }

    #[test]
    fn test_i4_indirect_cycle_detected() {
        // A -> B -> C -> A is a cycle
        let source = r#"
snippet id="a" kind="fn"
signature
  fn name="a"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="b"
    as="r"
  end
  step id="s2" kind="return"
    from="r"
    as="_"
  end
end
end

snippet id="b" kind="fn"
signature
  fn name="b"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="c"
    as="r"
  end
  step id="s2" kind="return"
    from="r"
    as="_"
  end
end
end

snippet id="c" kind="fn"
signature
  fn name="c"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="a"
    as="r"
  end
  step id="s2" kind="return"
    from="r"
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source);
        assert!(result.is_err(), "Indirect cycle should be detected");
    }

    #[test]
    fn test_i4_self_recursion_allowed() {
        // A function calling itself is NOT a cycle error (recursion is allowed)
        let source = r#"
snippet id="math.factorial" kind="fn"
signature
  fn name="factorial"
    param name="n" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=less_eq
    input var="n"
    input lit=1
    as="is_base"
  end
  step id="s2" kind="if"
    condition="is_base"
    then
      step id="s2a" kind="return"
        lit=1
        as="_"
      end
    end
    else
      step id="s2b" kind="compute"
        op=sub
        input var="n"
        input lit=1
        as="n_minus_1"
      end
      step id="s2c" kind="call"
        fn="math.factorial"
        arg name="n" from="n_minus_1"
        as="sub_result"
      end
      step id="s2d" kind="compute"
        op=mul
        input var="n"
        input var="sub_result"
        as="result"
      end
      step id="s2e" kind="return"
        from="result"
        as="_"
      end
    end
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source);
        assert!(result.is_ok(), "Self-recursion should be allowed: {:?}", result.err());

        let graph = result.unwrap();
        assert!(graph.graph.invariants.i4_acyclicity);
    }

    #[test]
    fn test_i4_mutual_recursion_is_cycle() {
        // A calls B, B calls A is mutual recursion - treated as cycle
        let source = r#"
snippet id="is_even" kind="fn"
signature
  fn name="is_even"
    param name="n" type="Int"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=equals
    input var="n"
    input lit=0
    as="is_zero"
  end
  step id="s2" kind="if"
    condition="is_zero"
    then
      step id="s2a" kind="return"
        lit=true
        as="_"
      end
    end
    else
      step id="s2b" kind="compute"
        op=sub
        input var="n"
        input lit=1
        as="n_minus_1"
      end
      step id="s2c" kind="call"
        fn="is_odd"
        arg name="n" from="n_minus_1"
        as="result"
      end
      step id="s2d" kind="return"
        from="result"
        as="_"
      end
    end
    as="_"
  end
end
end

snippet id="is_odd" kind="fn"
signature
  fn name="is_odd"
    param name="n" type="Int"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=equals
    input var="n"
    input lit=0
    as="is_zero"
  end
  step id="s2" kind="if"
    condition="is_zero"
    then
      step id="s2a" kind="return"
        lit=false
        as="_"
      end
    end
    else
      step id="s2b" kind="compute"
        op=sub
        input var="n"
        input lit=1
        as="n_minus_1"
      end
      step id="s2c" kind="call"
        fn="is_even"
        arg name="n" from="n_minus_1"
        as="result"
      end
      step id="s2d" kind="return"
        from="result"
        as="_"
      end
    end
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source);
        // Mutual recursion is detected as a cycle
        assert!(result.is_err(), "Mutual recursion should be detected as cycle");
    }

    #[test]
    fn test_i4_no_cycle_in_dag() {
        // Diamond dependency: A -> B, A -> C, B -> D, C -> D (no cycle)
        let source = r#"
snippet id="d" kind="fn"
signature
  fn name="d"
    returns type="Int"
  end
end
body
  step id="s1" kind="return"
    lit=1
    as="_"
  end
end
end

snippet id="b" kind="fn"
signature
  fn name="b"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="d"
    as="r"
  end
  step id="s2" kind="return"
    from="r"
    as="_"
  end
end
end

snippet id="c" kind="fn"
signature
  fn name="c"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="d"
    as="r"
  end
  step id="s2" kind="return"
    from="r"
    as="_"
  end
end
end

snippet id="a" kind="fn"
signature
  fn name="a"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="b"
    as="r1"
  end
  step id="s2" kind="call"
    fn="c"
    as="r2"
  end
  step id="s3" kind="compute"
    op=add
    input var="r1"
    input var="r2"
    as="result"
  end
  step id="s4" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source);
        assert!(result.is_ok(), "Diamond dependency should not be a cycle: {:?}", result.err());
        assert!(result.unwrap().graph.invariants.i4_acyclicity);
    }

    // === Duplicate Detection Tests ===

    #[test]
    fn test_duplicate_snippet_id_error() {
        let source = r#"
snippet id="duplicate" kind="fn"
signature
  fn name="first"
    returns type="Int"
  end
end
body
  step id="s1" kind="return"
    lit=1
    as="_"
  end
end
end

snippet id="duplicate" kind="fn"
signature
  fn name="second"
    returns type="Int"
  end
end
body
  step id="s1" kind="return"
    lit=2
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source);
        assert!(result.is_err(), "Duplicate ID should produce error");

        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| matches!(e, SymbolError::DuplicateId { id, .. } if id == "duplicate")),
            "Should have DuplicateId error for 'duplicate'"
        );
    }

    // === Type Reference Tests ===

    #[test]
    fn test_type_references_tracked() {
        let source = r#"
snippet id="types.User" kind="struct"
signature
  struct name="User"
    field name="id" type="Int"
    field name="name" type="String"
  end
end
end

snippet id="app.get_user" kind="fn"
signature
  fn name="get_user"
    param name="id" type="Int"
    returns type="User"
  end
end
body
  step id="s1" kind="return"
    lit=none
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source).expect("should build graph");

        let get_user = result.graph.get_by_name("app.get_user").unwrap();

        // get_user should reference User type
        assert!(
            get_user.references.contains("User") || get_user.references.contains("types.User"),
            "get_user should reference User type. References: {:?}",
            get_user.references
        );
    }

    // === Unresolved Reference Tests ===

    #[test]
    fn test_unresolved_call_produces_deferred_error() {
        let source = r#"
snippet id="app.main" kind="fn"
signature
  fn name="main"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="nonexistent.function"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source);

        // Should succeed but with deferred errors
        assert!(result.is_ok(), "Should succeed with deferred errors: {:?}", result.err());

        let graph_result = result.unwrap();
        let symbol = graph_result.graph.get_by_name("app.main").unwrap();

        // The unresolved call should be tracked
        assert!(
            symbol.unresolved_calls.contains("nonexistent.function"),
            "Should track unresolved call. Unresolved: {:?}",
            symbol.unresolved_calls
        );
    }

    // === Effect Extraction Tests ===

    #[test]
    fn test_effects_extracted_from_snippet() {
        let source = r#"
snippet id="io.write" kind="fn"
effects
  effect filesystem
  effect console
end
signature
  fn name="write"
    param name="path" type="String"
    param name="content" type="String"
    returns type="Bool"
  end
end
body
  step id="s1" kind="return"
    lit=true
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source).expect("should build graph");
        let symbol = result.graph.get_by_name("io.write").unwrap();

        assert!(symbol.declared_effects.iter().any(|e| e.name == "filesystem"));
        assert!(symbol.declared_effects.iter().any(|e| e.name == "console"));
        assert_eq!(symbol.declared_effects.len(), 2);
    }

    // === Module/Namespace Tests ===

    #[test]
    fn test_module_namespace_extraction() {
        let source = r#"
snippet id="myapp.services.user.get_by_id" kind="fn"
signature
  fn name="get_by_id"
    param name="id" type="Int"
    returns type="User"
  end
end
body
  step id="s1" kind="return"
    lit=none
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source).expect("should build graph");
        let symbol = result.graph.get_by_name("myapp.services.user.get_by_id").unwrap();

        assert_eq!(symbol.name, "myapp.services.user.get_by_id");
        // Module is the part before the last dot
        // This depends on implementation but we can verify the full name works
    }

    // === Cross-Module References ===

    #[test]
    fn test_cross_module_call_reference() {
        let source = r#"
snippet id="math.add" kind="fn"
signature
  fn name="add"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=add
    input var="a"
    input var="b"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end

snippet id="app.compute" kind="fn"
signature
  fn name="compute"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="math.add"
    arg name="a" lit=2
    arg name="b" lit=3
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
        let result = build_graph_from_source(source).expect("should build graph");

        let app_compute = result.graph.get_by_name("app.compute").unwrap();
        let math_add = result.graph.get_by_name("math.add").unwrap();

        // Cross-module call is tracked
        assert!(app_compute.calls.contains("math.add"));
        assert!(math_add.called_by.contains(&app_compute.id));
    }

    // === Large Graph Tests ===

    #[test]
    fn test_many_symbols_performance() {
        // Generate a moderately large number of symbols to verify performance
        let mut source = String::new();

        for i in 0..50 {
            source.push_str(&format!(
                r#"
snippet id="fn_{i}" kind="fn"
signature
  fn name="fn_{i}"
    returns type="Int"
  end
end
body
  step id="s1" kind="return"
    lit={i}
    as="_"
  end
end
end
"#,
                i = i
            ));
        }

        let result = build_graph_from_source(&source);
        assert!(result.is_ok(), "Should handle 50 symbols: {:?}", result.err());
        assert_eq!(result.unwrap().graph.len(), 50);
    }
}
