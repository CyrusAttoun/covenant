//! Covenant - A machine-first programming language
//!
//! This is the root workspace crate that provides integration tests.
//! The actual implementation is in the workspace member crates.

// Re-export main crates for convenience
pub use covenant_ast as ast;
pub use covenant_lexer as lexer;
pub use covenant_parser as parser;

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles() {
        // Ensure the workspace compiles
        assert!(true);
    }
}
