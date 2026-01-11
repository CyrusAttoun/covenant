use std::fs;
use std::path::{Path, PathBuf};

/// Load a .cov file from examples/
pub fn load_example(name: &str) -> String {
    let path = example_path(name);
    fs::read_to_string(&path)
        .expect(&format!("Failed to load example: {}", name))
}

/// Get path to an example file
pub fn example_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(format!("{}.cov", name))
}

/// Load a test fixture from tests/fixtures/
pub fn load_fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    fs::read_to_string(&path)
        .expect(&format!("Failed to load fixture: {}", name))
}
