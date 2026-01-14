//! Covenant Type Checker
//!
//! Performs type checking, effect tracking, and symbol resolution.

mod types;
mod symbols;
mod checker;
mod effects;
mod snippet_checker;

pub use types::*;
pub use symbols::*;
pub use checker::*;
pub use effects::*;
pub use snippet_checker::SnippetChecker;

use covenant_ast::Program;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CheckError {
    #[error("undefined symbol: {name}")]
    UndefinedSymbol { name: String },

    #[error("type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: String, found: String },

    #[error("effect not allowed: {effect} in pure function")]
    EffectNotAllowed { effect: String },

    #[error("duplicate definition: {name}")]
    DuplicateDefinition { name: String },
}

/// Check a program and return the typed/annotated version
pub fn check(program: &Program) -> Result<CheckResult, Vec<CheckError>> {
    match program {
        Program::Legacy { declarations, .. } => {
            let mut checker = Checker::new();
            checker.check_declarations(declarations)
        }
        Program::Snippets { snippets, .. } => {
            let checker = SnippetChecker::new();
            checker.check_snippets(snippets)
        }
    }
}

/// Result of type checking
#[derive(Debug, Default)]
pub struct CheckResult {
    /// All symbols discovered
    pub symbols: SymbolTable,
    /// Effect information
    pub effects: EffectTable,
}
