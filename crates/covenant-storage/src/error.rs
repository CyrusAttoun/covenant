//! Error types for storage operations

use std::io;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("Database error: {0}")]
    Database(#[from] redb::Error),

    #[error("Database creation error: {0}")]
    DatabaseCreation(#[from] redb::DatabaseError),

    #[error("Transaction error: {0}")]
    Transaction(#[from] redb::TransactionError),

    #[error("Commit error: {0}")]
    Commit(#[from] redb::CommitError),

    #[error("Compaction error: {0}")]
    Compaction(#[from] redb::CompactionError),

    #[error("Table error: {0}")]
    Table(#[from] redb::TableError),

    #[error("Storage error: {0}")]
    Storage(#[from] redb::StorageError),

    #[error("Node not found: {0}")]
    NotFound(String),

    #[error("Invariant violation: {0}")]
    InvariantViolation(String),

    #[error("Version conflict: expected {expected}, found {actual}")]
    VersionConflict { expected: u64, actual: u64 },

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("File watcher error: {0}")]
    FileWatcher(String),

    #[error("Invalid JSON in AST field: {0}")]
    InvalidJson(String),

    #[error("Invalid file path: {0}")]
    InvalidPath(String),
}
