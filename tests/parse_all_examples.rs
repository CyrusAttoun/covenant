use covenant_parser::parse;
use std::fs;
use std::path::{Path, PathBuf};

/// Discover all .cov files in the examples/ directory
fn discover_examples() -> Vec<PathBuf> {
    let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples");

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

/// Discover all .cov files in the runtime/std/*/tests/ directories
fn discover_stdlib_tests() -> Vec<PathBuf> {
    let stdlib_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("runtime")
        .join("std");

    if !stdlib_dir.exists() {
        return Vec::new();
    }

    let mut test_files = Vec::new();

    if let Ok(entries) = fs::read_dir(&stdlib_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let tests_dir = entry.path().join("tests");
            if tests_dir.is_dir() {
                if let Ok(test_entries) = fs::read_dir(&tests_dir) {
                    for test_entry in test_entries.filter_map(|e| e.ok()) {
                        let path = test_entry.path();
                        if path.extension().and_then(|ext| ext.to_str()) == Some("cov") {
                            test_files.push(path);
                        }
                    }
                }
            }
        }
    }

    test_files
}

#[test]
fn test_all_examples_parse() {
    let examples = discover_examples();

    assert!(
        !examples.is_empty(),
        "No .cov examples found! Check examples/ directory."
    );

    // Examples that use features not yet implemented in the parser
    let skip_files = [
        "21-structured-concurrency.cov", // Uses parallel/race step kinds (not yet parsed)
        "24-cross-platform-storage.cov", // Uses ML-style comments and dotted effect names
    ];

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

        let source = fs::read_to_string(&example_path)
            .expect(&format!("Failed to read {:?}", example_path));

        match parse(&source) {
            Ok(_program) => {
                println!("✓ Parsed: {}", example_path.display());
            }
            Err(err) => {
                eprintln!("✗ Failed to parse: {}", example_path.display());
                eprintln!("  Error: {:?}", err);
                failures.push((example_path.clone(), err));
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
                .map(|(path, err)| format!("  - {}: {:?}", path.display(), err))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

#[test]
fn test_stdlib_tests_parse() {
    let test_files = discover_stdlib_tests();

    if test_files.is_empty() {
        println!("⊘ No stdlib test files found (runtime/std/*/tests/ may not exist)");
        return;
    }

    let mut failures = Vec::new();

    for test_path in &test_files {
        let source = fs::read_to_string(&test_path)
            .expect(&format!("Failed to read {:?}", test_path));

        match parse(&source) {
            Ok(_program) => {
                println!("✓ Parsed: {}", test_path.display());
            }
            Err(err) => {
                eprintln!("✗ Failed to parse: {}", test_path.display());
                eprintln!("  Error: {:?}", err);
                failures.push((test_path.clone(), err));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "\n{} out of {} stdlib tests failed to parse:\n{}",
            failures.len(),
            test_files.len(),
            failures
                .iter()
                .map(|(path, err)| format!("  - {}: {:?}", path.display(), err))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
