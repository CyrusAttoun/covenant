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

    #[error("incompatible union: {value_type} is not a member of {union_type}")]
    IncompatibleUnion {
        value_type: String,
        union_type: String,
    },

    #[error("non-exhaustive match on {matched_type}: missing variants {missing:?}")]
    NonExhaustiveMatch {
        missing: Vec<String>,
        matched_type: String,
    },

    #[error("unknown query target: {target}")]
    UnknownQueryTarget { target: String },

    #[error("unknown field '{field}' in type '{type_name}'")]
    UnknownField { field: String, type_name: String },

    #[error("extern-impl '{impl_id}' references unknown extern-abstract '{abstract_id}'")]
    UnknownExternAbstract {
        impl_id: String,
        abstract_id: String,
    },

    #[error("no binding for '{extern_id}' on target '{target}'")]
    NoBindingForTarget {
        extern_id: String,
        target: String,
    },
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
