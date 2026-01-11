use std::fs;
use std::path::{Path, PathBuf};

/// Discover all .cov files in the examples/ directory
pub fn discover_examples() -> Vec<PathBuf> {
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

/// Extract a test name from a .cov file path
/// Example: "01-hello-world.cov" -> "hello_world"
pub fn test_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.replace('-', "_"))
        .map(|s| {
            // Remove leading numbers: "01_hello_world" -> "hello_world"
            s.split('_')
                .skip_while(|part| part.chars().all(|c| c.is_numeric()))
                .collect::<Vec<_>>()
                .join("_")
        })
        .unwrap_or_else(|| "unknown".to_string())
}
