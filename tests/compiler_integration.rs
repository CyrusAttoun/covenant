//! End-to-end integration tests for the Covenant compiler
//!
//! These tests verify the full compilation pipeline from source to WASM.

use std::fs;
use std::path::PathBuf;

/// Discover all .cov files in the examples/ directory
fn discover_examples() -> Vec<PathBuf> {
    let examples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");

    fs::read_dir(&examples_dir)
        .expect("Failed to read examples directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                == Some("cov")
        })
        .collect()
}

/// Test that all example files parse successfully
/// Note: Some examples use features not yet implemented in the parser and are skipped
#[test]
fn e2e_all_examples_parse() {
    let examples = discover_examples();

    assert!(
        !examples.is_empty(),
        "No .cov examples found! Check examples/ directory."
    );

    // Examples that use features not yet implemented in the parser
    let skip_files: [&str; 0] = [];

    let mut failures = Vec::new();

    for example_path in &examples {
        let filename = example_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if skip_files.iter().any(|s| filename == *s) {
            println!("⊘ Skipped (uses unimplemented features): {}", example_path.display());
            continue;
        }

        let source =
            fs::read_to_string(example_path).expect(&format!("Failed to read {:?}", example_path));

        match covenant_parser::parse(&source) {
            Ok(_program) => {
                println!("✓ Parsed: {}", example_path.display());
            }
            Err(err) => {
                eprintln!("✗ Failed to parse: {}", example_path.display());
                eprintln!("  Error: {:?}", err);
                failures.push((example_path.clone(), format!("{:?}", err)));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "\n{} out of {} examples failed to parse:\n{}",
            failures.len(),
            examples.len(),
            failures
                .iter()
                .map(|(path, err)| format!("  - {}: {}", path.display(), err))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// Test that all example files build valid symbol graphs
/// Note: Some examples have intentional forward references to external symbols and are skipped
#[test]
fn e2e_all_examples_build_symbol_graph() {
    let examples = discover_examples();
    let mut failures = Vec::new();

    // Examples that have intentional forward references to symbols not defined in the file
    let skip_files = [
        "19-data-nodes.cov",     // Has references to auth/docs/kb symbols in other files
        "20-knowledge-base.cov", // Has references to external kb/effects symbols
    ];

    for example_path in &examples {
        let filename = example_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if skip_files.iter().any(|s| filename == *s) {
            println!("⊘ Skipped (has external forward references): {}", example_path.display());
            continue;
        }

        let source =
            fs::read_to_string(example_path).expect(&format!("Failed to read {:?}", example_path));

        let program = match covenant_parser::parse(&source) {
            Ok(p) => p,
            Err(_) => continue, // Skip parse failures (tested separately)
        };

        match covenant_symbols::build_symbol_graph(&program) {
            Ok(result) => {
                println!(
                    "✓ Symbol graph: {} ({} symbols)",
                    example_path.display(),
                    result.graph.len()
                );
            }
            Err(errors) => {
                // Cycle errors are expected for some examples (mutual recursion)
                let non_cycle_errors: Vec<_> = errors
                    .iter()
                    .filter(|e| !matches!(e, covenant_symbols::SymbolError::CircularImport { .. }))
                    .collect();

                if !non_cycle_errors.is_empty() {
                    eprintln!("✗ Symbol graph failed: {}", example_path.display());
                    for err in &non_cycle_errors {
                        eprintln!("  Error: {:?}", err);
                    }
                    failures.push((example_path.clone(), format!("{:?}", non_cycle_errors)));
                }
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "\n{} examples failed symbol graph building:\n{}",
            failures.len(),
            failures
                .iter()
                .map(|(path, err)| format!("  - {}: {}", path.display(), err))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// Test that pure-function examples type-check successfully
#[test]
fn e2e_pure_functions_type_check() {
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

    let program = covenant_parser::parse(source).expect("parse failed");
    let result = covenant_checker::check(&program);
    assert!(
        result.is_ok(),
        "Pure functions should type-check: {:?}",
        result.err()
    );
}

/// Test that effectful code without declarations fails
#[test]
fn e2e_missing_effect_detected() {
    let source = r#"
snippet id="io.print" kind="extern"

effects
  effect console
end

signature
  fn name="print"
    param name="msg" type="String"
    returns type="Unit"
  end
end

end

snippet id="app.greet" kind="fn"

signature
  fn name="greet"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="io.print"
    arg name="msg" lit="Hello"
    as="_"
  end
end

end
"#;

    let program = covenant_parser::parse(source).expect("parse failed");
    let symbols = covenant_symbols::build_symbol_graph(&program).expect("symbol graph failed");
    let effect_result = covenant_checker::check_effects(&symbols.graph);

    assert!(
        !effect_result.violations.is_empty(),
        "Should detect missing console effect"
    );
}

/// Test that parse errors block later phases
#[test]
fn e2e_parse_error_stops_pipeline() {
    let source = r#"
snippet id="bad" kind="fn"
signature
  fn name="bad"
    returns type="Int"
  end
end
body
  // Missing step and end keywords
"#;

    let result = covenant_parser::parse(source);
    assert!(result.is_err(), "Malformed source should fail to parse");
}

/// Test full pipeline for simple arithmetic
#[test]
fn e2e_full_pipeline_simple_arithmetic() {
    let source = r#"
snippet id="math.triple" kind="fn"

signature
  fn name="triple"
    param name="x" type="Int"
    returns type="Int"
  end
end

body
  step id="s1" kind="compute"
    op=mul
    input var="x"
    input lit=3
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    // Phase 1: Parse
    let program = covenant_parser::parse(source).expect("Phase 1 failed: parse error");

    // Phase 2: Symbol Graph
    let symbol_result =
        covenant_symbols::build_symbol_graph(&program).expect("Phase 2 failed: symbol error");
    assert!(symbol_result.graph.contains("math.triple"));

    // Phase 3: Effect Check
    let effect_result = covenant_checker::check_effects(&symbol_result.graph);
    assert!(
        effect_result.violations.is_empty(),
        "Phase 3 failed: effect violations"
    );

    // Phase 4: Type Check
    let check_result = covenant_checker::check(&program).expect("Phase 4 failed: type error");
    assert!(check_result.symbols.lookup("math.triple").is_some());

    // Phase 7: Codegen (if available)
    let wasm_result = covenant_codegen::compile(&program, &check_result.symbols);
    assert!(wasm_result.is_ok(), "Phase 7 failed: {:?}", wasm_result.err());
}

/// Test that type errors are detected across snippets
/// Note: Cross-snippet type checking is a future enhancement. This test verifies the current
/// behavior and will need updating when inter-snippet type checking is implemented.
#[test]
#[ignore = "Cross-snippet type checking not yet implemented"]
fn e2e_cross_snippet_type_error() {
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

snippet id="app.main" kind="fn"
signature
  fn name="main"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="math.add"
    arg name="a" lit="not an int"
    arg name="b" lit=2
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;

    let program = covenant_parser::parse(source).expect("parse failed");
    let result = covenant_checker::check(&program);

    assert!(
        result.is_err(),
        "Should detect type mismatch in cross-snippet call"
    );
}

/// Test invariant validation
#[test]
fn e2e_invariants_validated() {
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
  step id="s1" kind="return"
    lit=42
    as="_"
  end
end
end
"#;

    let program = covenant_parser::parse(source).expect("parse failed");
    let result = covenant_symbols::build_symbol_graph(&program).expect("symbol graph failed");

    // All invariants should be validated
    assert!(result.graph.invariants.i1_bidirectionality);
    assert!(result.graph.invariants.i4_acyclicity);
    assert!(result.graph.invariants.i5_relation_bidirectionality);

    // Verify I1 explicitly
    let a = result.graph.get_by_name("a").unwrap();
    let b = result.graph.get_by_name("b").unwrap();
    assert!(a.calls.contains("b"));
    assert!(b.called_by.contains(&a.id));
}

/// Test cycle detection blocks compilation
#[test]
fn e2e_cycle_detection_blocks_compilation() {
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

    let program = covenant_parser::parse(source).expect("parse failed");
    let result = covenant_symbols::build_symbol_graph(&program);

    assert!(result.is_err(), "Cycle should block symbol graph building");
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, covenant_symbols::SymbolError::CircularImport { .. })));
}
