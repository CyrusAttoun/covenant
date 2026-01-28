//! Runtime error types

use thiserror::Error;

/// Errors that can occur during runtime operations
#[derive(Debug, Clone, Error)]
pub enum RuntimeError {
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    #[error("Query cancelled")]
    QueryCancelled,

    #[error("Query timed out")]
    QueryTimeout,

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Compilation error: {0}")]
    CompilationError(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Deserialization failed: {0}")]
    DeserializationFailed(String),
}

impl RuntimeError {
    /// Get the error code for this error
    pub fn code(&self) -> &'static str {
        match self {
            RuntimeError::SymbolNotFound(_) => "E-RT-001",
            RuntimeError::InvalidQuery(_) => "E-RT-002",
            RuntimeError::QueryCancelled => "E-RT-003",
            RuntimeError::QueryTimeout => "E-RT-004",
            RuntimeError::ParseError(_) => "E-RT-005",
            RuntimeError::ValidationError(_) => "E-RT-006",
            RuntimeError::CompilationError(_) => "E-RT-007",
            RuntimeError::DeserializationFailed(_) => "E-RT-008",
            RuntimeError::Internal(_) => "E-RT-999",
        }
    }
}
