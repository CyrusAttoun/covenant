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

#[test]
fn test_all_examples_parse() {
    let examples = discover_examples();

    assert!(
        !examples.is_empty(),
        "No .cov examples found! Check examples/ directory."
    );

    let mut failures = Vec::new();

    for example_path in &examples {
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
