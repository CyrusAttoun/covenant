//! Error types for symbol graph building (Phase 2)

use covenant_ast::Span;
use thiserror::Error;

/// Errors from symbol graph building (Phase 2)
#[derive(Debug, Clone, Error)]
pub enum SymbolError {
    /// E-SYMBOL-001: Undefined reference (soft error in Phase 2)
    #[error("undefined reference: {name}")]
    UndefinedReference {
        name: String,
        span: Span,
        /// The symbol containing the unresolved reference
        referrer: String,
    },

    /// E-SYMBOL-002: Duplicate ID (hard error)
    #[error("duplicate symbol ID: {id}")]
    DuplicateId { id: String, span: Span },

    /// E-SYMBOL-003: Circular import (hard error)
    #[error("circular import detected: {cycle}")]
    CircularImport {
        /// Full cycle path, e.g., "a -> b -> c -> a"
        cycle: String,
        span: Span,
    },

    /// E-REL-001: Relation target not found (hard error)
    #[error("relation target not found: {target}")]
    RelationTargetNotFound {
        target: String,
        span: Span,
        /// The symbol containing the relation
        from_symbol: String,
    },
}

impl SymbolError {
    /// Get the source span of this error
    pub fn span(&self) -> Span {
        match self {
            SymbolError::UndefinedReference { span, .. } => *span,
            SymbolError::DuplicateId { span, .. } => *span,
            SymbolError::CircularImport { span, .. } => *span,
            SymbolError::RelationTargetNotFound { span, .. } => *span,
        }
    }

    /// Whether this is a hard error that blocks compilation
    pub fn is_hard_error(&self) -> bool {
        match self {
            SymbolError::UndefinedReference { .. } => false, // Soft in Phase 2
            SymbolError::DuplicateId { .. } => true,
            SymbolError::CircularImport { .. } => true,
            SymbolError::RelationTargetNotFound { .. } => true,
        }
    }

    /// Error code for machine-readable output
    pub fn code(&self) -> &'static str {
        match self {
            SymbolError::UndefinedReference { .. } => "E-SYMBOL-001",
            SymbolError::DuplicateId { .. } => "E-SYMBOL-002",
            SymbolError::CircularImport { .. } => "E-SYMBOL-003",
            SymbolError::RelationTargetNotFound { .. } => "E-REL-001",
        }
    }
}
