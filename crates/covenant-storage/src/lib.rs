//! Covenant Storage - Pluggable storage backend for the symbol graph
//!
//! This crate provides a storage abstraction for persisting and querying
//! the Covenant project's symbol graph, supporting multiple backends:
//! - In-memory (for testing)
//! - redb (production-ready embedded database)
//!
//! ## Architecture
//!
//! The storage layer follows a three-layer design:
//! - Layer 1: Core KV operations (get, put, delete, list)
//! - Layer 2: Indexed queries (by kind, effect, relation)
//! - Layer 3: Transactions with ACID guarantees

mod error;
mod node;
mod provider;
mod memory;
mod redb_storage;
mod sync;

pub use error::{StorageError, Result};
pub use node::{Node, SnippetKind, Relation};
pub use provider::{StorageProvider, Transaction, InvariantViolation};
pub use memory::InMemoryStorage;
pub use redb_storage::RedbStorage;
pub use sync::StorageSync;
