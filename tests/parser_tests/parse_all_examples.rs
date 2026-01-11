use covenant_parser::parse;
use std::fs;

mod example_discovery;
use example_discovery::*;

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
