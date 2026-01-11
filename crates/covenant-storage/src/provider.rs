//! Storage provider trait and transaction interface

use crate::{Node, Result, SnippetKind};

/// Pluggable storage provider interface
///
/// Implementations must support three layers:
/// - Layer 1: Core KV operations (get, put, delete, list)
/// - Layer 2: Indexed queries (by kind, effect, relation)
/// - Layer 3: Transactions with ACID guarantees
pub trait StorageProvider: Send + Sync {
    // ========== Layer 1: Core KV Operations ==========

    /// Get a node by ID
    fn get(&self, id: &str) -> Result<Option<Node>>;

    /// Store a node (insert or update)
    fn put(&mut self, id: &str, node: &Node) -> Result<()>;

    /// Delete a node by ID
    fn delete(&mut self, id: &str) -> Result<()>;

    /// List all node IDs with a given prefix
    fn list(&self, prefix: &str) -> Result<Vec<String>>;

    // ========== Layer 2: Indexed Queries ==========

    /// Query nodes by snippet kind
    fn query_by_kind(&self, kind: SnippetKind) -> Result<Vec<Node>>;

    /// Query nodes that declare a specific effect
    fn query_by_effect(&self, effect: &str) -> Result<Vec<Node>>;

    /// Query nodes that have a relation to the target
    ///
    /// Returns all nodes with `rel_type` relation pointing to `target_id`
    fn query_by_relation(&self, target_id: &str, rel_type: &str) -> Result<Vec<Node>>;

    // ========== Layer 3: Transactions ==========

    /// Begin a transaction for atomic multi-operation updates
    fn begin_transaction(&mut self) -> Result<Box<dyn Transaction + '_>>;

    // ========== Maintenance Operations ==========

    /// Rebuild all secondary indexes from primary storage
    fn rebuild_indexes(&mut self) -> Result<()>;

    /// Verify symbol graph invariants (I1-I5)
    fn verify_invariants(&self) -> Result<Vec<InvariantViolation>>;

    /// Compact storage (remove deleted entries, optimize layout)
    fn compact(&mut self) -> Result<()>;

    /// Get storage statistics
    fn stats(&self) -> Result<StorageStats> {
        Ok(StorageStats {
            total_nodes: self.list("")?.len(),
            functions: self.query_by_kind(SnippetKind::Function)?.len(),
            structs: self.query_by_kind(SnippetKind::Struct)?.len(),
            data_nodes: self.query_by_kind(SnippetKind::Data)?.len(),
        })
    }
}

/// Transaction interface for atomic multi-operation updates
pub trait Transaction {
    /// Store a node within the transaction
    fn put(&mut self, id: &str, node: &Node) -> Result<()>;

    /// Delete a node within the transaction
    fn delete(&mut self, id: &str) -> Result<()>;

    /// Commit the transaction atomically
    fn commit(self: Box<Self>) -> Result<()>;

    /// Rollback the transaction (discard all changes)
    fn rollback(self: Box<Self>) -> Result<()>;
}

/// Represents a violation of symbol graph invariants
#[derive(Debug, Clone)]
pub struct InvariantViolation {
    pub invariant: String,
    pub node_id: String,
    pub description: String,
}

impl std::fmt::Display for InvariantViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {}: {}",
            self.invariant, self.node_id, self.description
        )
    }
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_nodes: usize,
    pub functions: usize,
    pub structs: usize,
    pub data_nodes: usize,
}

impl std::fmt::Display for StorageStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Storage Statistics:")?;
        writeln!(f, "  Total snippets: {}", self.total_nodes)?;
        writeln!(f, "  Functions: {}", self.functions)?;
        writeln!(f, "  Structs: {}", self.structs)?;
        writeln!(f, "  Data nodes: {}", self.data_nodes)?;
        Ok(())
    }
}
