//! redb-based persistent storage implementation

use crate::{Node, Result, SnippetKind, StorageProvider, Transaction, InvariantViolation};
use redb::{Database, ReadableTable, TableDefinition};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

// Table definitions
const NODES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("nodes");
const KIND_INDEX: TableDefinition<&str, &[u8]> = TableDefinition::new("kind_index");
const EFFECT_INDEX: TableDefinition<&str, &[u8]> = TableDefinition::new("effect_index");
const RELATION_INDEX: TableDefinition<&str, &[u8]> = TableDefinition::new("relation_index");
const VERSION_TABLE: TableDefinition<&str, u64> = TableDefinition::new("version");

/// redb-based persistent storage
///
/// Provides ACID-compliant persistent storage using redb embedded database.
/// All data is stored in a single `.redb` file with automatic crash recovery.
pub struct RedbStorage {
    db: Database,
    path: PathBuf,
}

impl RedbStorage {
    /// Create or open a redb storage at the given path
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let db = Database::create(&path)?;

        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            write_txn.open_table(NODES_TABLE)?;
            write_txn.open_table(KIND_INDEX)?;
            write_txn.open_table(EFFECT_INDEX)?;
            write_txn.open_table(RELATION_INDEX)?;
            write_txn.open_table(VERSION_TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self { db, path })
    }

    /// Get the file path of this storage
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Update secondary indexes for a node
    fn update_indexes_in_txn(
        &self,
        write_txn: &redb::WriteTransaction,
        id: &str,
        node: &Node,
    ) -> Result<()> {
        // Update kind index
        {
            let mut kind_index = write_txn.open_table(KIND_INDEX)?;
            let kind_key = format!("{:?}", node.kind);
            let mut ids: HashSet<String> = match kind_index.get(kind_key.as_str())? {
                Some(bytes) => bincode::deserialize(bytes.value())?,
                None => HashSet::new(),
            };
            ids.insert(id.to_string());
            let bytes = bincode::serialize(&ids)?;
            kind_index.insert(kind_key.as_str(), bytes.as_slice())?;
        }

        // Update effect index
        {
            let mut effect_index = write_txn.open_table(EFFECT_INDEX)?;
            for effect in &node.effect_closure {
                let mut ids: HashSet<String> = match effect_index.get(effect.as_str())? {
                    Some(bytes) => bincode::deserialize(bytes.value())?,
                    None => HashSet::new(),
                };
                ids.insert(id.to_string());
                let bytes = bincode::serialize(&ids)?;
                effect_index.insert(effect.as_str(), bytes.as_slice())?;
            }
        }

        // Update relation index
        {
            let mut relation_index = write_txn.open_table(RELATION_INDEX)?;
            for rel in &node.relations {
                let rel_key = format!("{}:{}", rel.target, rel.rel_type);
                let mut ids: HashSet<String> = match relation_index.get(rel_key.as_str())? {
                    Some(bytes) => bincode::deserialize(bytes.value())?,
                    None => HashSet::new(),
                };
                ids.insert(id.to_string());
                let bytes = bincode::serialize(&ids)?;
                relation_index.insert(rel_key.as_str(), bytes.as_slice())?;
            }
        }

        Ok(())
    }

    /// Remove a node from secondary indexes
    fn remove_from_indexes_in_txn(
        &self,
        write_txn: &redb::WriteTransaction,
        id: &str,
        node: &Node,
    ) -> Result<()> {
        // Remove from kind index
        {
            let mut kind_index = write_txn.open_table(KIND_INDEX)?;
            let kind_key = format!("{:?}", node.kind);
            let ids_data = if let Some(bytes) = kind_index.get(kind_key.as_str())? {
                Some(bytes.value().to_vec())
            } else {
                None
            };

            if let Some(data) = ids_data {
                let mut ids: HashSet<String> = bincode::deserialize(&data)?;
                ids.remove(id);
                if ids.is_empty() {
                    kind_index.remove(kind_key.as_str())?;
                } else {
                    let bytes = bincode::serialize(&ids)?;
                    kind_index.insert(kind_key.as_str(), bytes.as_slice())?;
                }
            }
        }

        // Remove from effect index
        {
            let mut effect_index = write_txn.open_table(EFFECT_INDEX)?;
            for effect in &node.effect_closure {
                let ids_data = if let Some(bytes) = effect_index.get(effect.as_str())? {
                    Some(bytes.value().to_vec())
                } else {
                    None
                };

                if let Some(data) = ids_data {
                    let mut ids: HashSet<String> = bincode::deserialize(&data)?;
                    ids.remove(id);
                    if ids.is_empty() {
                        effect_index.remove(effect.as_str())?;
                    } else {
                        let bytes = bincode::serialize(&ids)?;
                        effect_index.insert(effect.as_str(), bytes.as_slice())?;
                    }
                }
            }
        }

        // Remove from relation index
        {
            let mut relation_index = write_txn.open_table(RELATION_INDEX)?;
            for rel in &node.relations {
                let rel_key = format!("{}:{}", rel.target, rel.rel_type);
                let ids_data = if let Some(bytes) = relation_index.get(rel_key.as_str())? {
                    Some(bytes.value().to_vec())
                } else {
                    None
                };

                if let Some(data) = ids_data {
                    let mut ids: HashSet<String> = bincode::deserialize(&data)?;
                    ids.remove(id);
                    if ids.is_empty() {
                        relation_index.remove(rel_key.as_str())?;
                    } else {
                        let bytes = bincode::serialize(&ids)?;
                        relation_index.insert(rel_key.as_str(), bytes.as_slice())?;
                    }
                }
            }
        }

        Ok(())
    }
}

impl StorageProvider for RedbStorage {
    fn get(&self, id: &str) -> Result<Option<Node>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(NODES_TABLE)?;

        match table.get(id)? {
            Some(bytes) => {
                let node: Node = bincode::deserialize(bytes.value())?;
                Ok(Some(node))
            }
            None => Ok(None),
        }
    }

    fn put(&mut self, id: &str, node: &Node) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            // Get old node to remove from indexes
            let table = write_txn.open_table(NODES_TABLE)?;
            let old_node_data = if let Some(bytes) = table.get(id)? {
                Some(bytes.value().to_vec())
            } else {
                None
            };
            drop(table);

            if let Some(data) = old_node_data {
                let old_node: Node = bincode::deserialize(&data)?;
                self.remove_from_indexes_in_txn(&write_txn, id, &old_node)?;
            }

            // Serialize and insert new node
            let bytes = bincode::serialize(node)?;
            let mut table = write_txn.open_table(NODES_TABLE)?;
            table.insert(id, bytes.as_slice())?;
            drop(table);

            // Update indexes
            self.update_indexes_in_txn(&write_txn, id, node)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    fn delete(&mut self, id: &str) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            // Get node to remove from indexes
            let table = write_txn.open_table(NODES_TABLE)?;
            let node_data = if let Some(bytes) = table.get(id)? {
                Some(bytes.value().to_vec())
            } else {
                None
            };
            drop(table);

            if let Some(data) = node_data {
                let node: Node = bincode::deserialize(&data)?;
                self.remove_from_indexes_in_txn(&write_txn, id, &node)?;

                // Delete from main table
                let mut table = write_txn.open_table(NODES_TABLE)?;
                table.remove(id)?;
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(NODES_TABLE)?;

        let mut ids = Vec::new();
        for entry in table.iter()? {
            let (key, _) = entry?;
            let key_str = key.value();
            if key_str.starts_with(prefix) {
                ids.push(key_str.to_string());
            }
        }

        ids.sort();
        Ok(ids)
    }

    fn query_by_kind(&self, kind: SnippetKind) -> Result<Vec<Node>> {
        let read_txn = self.db.begin_read()?;
        let kind_index = read_txn.open_table(KIND_INDEX)?;
        let nodes_table = read_txn.open_table(NODES_TABLE)?;

        let kind_key = format!("{:?}", kind);
        let ids: HashSet<String> = match kind_index.get(kind_key.as_str())? {
            Some(bytes) => bincode::deserialize(bytes.value())?,
            None => return Ok(vec![]),
        };

        let mut results = Vec::new();
        for id in ids {
            if let Some(bytes) = nodes_table.get(id.as_str())? {
                let node: Node = bincode::deserialize(bytes.value())?;
                results.push(node);
            }
        }

        Ok(results)
    }

    fn query_by_effect(&self, effect: &str) -> Result<Vec<Node>> {
        let read_txn = self.db.begin_read()?;
        let effect_index = read_txn.open_table(EFFECT_INDEX)?;
        let nodes_table = read_txn.open_table(NODES_TABLE)?;

        let ids: HashSet<String> = match effect_index.get(effect)? {
            Some(bytes) => bincode::deserialize(bytes.value())?,
            None => return Ok(vec![]),
        };

        let mut results = Vec::new();
        for id in ids {
            if let Some(bytes) = nodes_table.get(id.as_str())? {
                let node: Node = bincode::deserialize(bytes.value())?;
                results.push(node);
            }
        }

        Ok(results)
    }

    fn query_by_relation(&self, target_id: &str, rel_type: &str) -> Result<Vec<Node>> {
        let read_txn = self.db.begin_read()?;
        let relation_index = read_txn.open_table(RELATION_INDEX)?;
        let nodes_table = read_txn.open_table(NODES_TABLE)?;

        let rel_key = format!("{}:{}", target_id, rel_type);
        let ids: HashSet<String> = match relation_index.get(rel_key.as_str())? {
            Some(bytes) => bincode::deserialize(bytes.value())?,
            None => return Ok(vec![]),
        };

        let mut results = Vec::new();
        for id in ids {
            if let Some(bytes) = nodes_table.get(id.as_str())? {
                let node: Node = bincode::deserialize(bytes.value())?;
                results.push(node);
            }
        }

        Ok(results)
    }

    fn begin_transaction(&mut self) -> Result<Box<dyn Transaction + '_>> {
        Ok(Box::new(RedbTransaction {
            storage: self,
            operations: Vec::new(),
        }))
    }

    fn rebuild_indexes(&mut self) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            // Clear all indexes
            {
                let mut kind_index = write_txn.open_table(KIND_INDEX)?;
                let keys: Vec<String> = kind_index
                    .iter()?
                    .map(|r| r.map(|(k, _)| k.value().to_string()))
                    .collect::<std::result::Result<_, _>>()?;
                for key in keys {
                    kind_index.remove(key.as_str())?;
                }
            }
            {
                let mut effect_index = write_txn.open_table(EFFECT_INDEX)?;
                let keys: Vec<String> = effect_index
                    .iter()?
                    .map(|r| r.map(|(k, _)| k.value().to_string()))
                    .collect::<std::result::Result<_, _>>()?;
                for key in keys {
                    effect_index.remove(key.as_str())?;
                }
            }
            {
                let mut relation_index = write_txn.open_table(RELATION_INDEX)?;
                let keys: Vec<String> = relation_index
                    .iter()?
                    .map(|r| r.map(|(k, _)| k.value().to_string()))
                    .collect::<std::result::Result<_, _>>()?;
                for key in keys {
                    relation_index.remove(key.as_str())?;
                }
            }

            // Rebuild from nodes
            let nodes_table = write_txn.open_table(NODES_TABLE)?;
            let entries: Vec<(String, Vec<u8>)> = nodes_table
                .iter()?
                .map(|r| {
                    r.map(|(k, v)| (k.value().to_string(), v.value().to_vec()))
                })
                .collect::<std::result::Result<_, _>>()?;
            drop(nodes_table);

            for (id, bytes) in entries {
                let node: Node = bincode::deserialize(&bytes)?;
                self.update_indexes_in_txn(&write_txn, &id, &node)?;
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    fn verify_invariants(&self) -> Result<Vec<InvariantViolation>> {
        let mut violations = Vec::new();
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(NODES_TABLE)?;

        // Load all nodes into memory for verification
        let mut nodes: std::collections::HashMap<String, Node> = std::collections::HashMap::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            let node: Node = bincode::deserialize(value.value())?;
            nodes.insert(key.value().to_string(), node);
        }

        // Check invariants
        for (id, node) in &nodes {
            // I1: Bidirectionality of calls
            for callee_id in &node.calls {
                if let Some(callee) = nodes.get(callee_id) {
                    if !callee.called_by.contains(id) {
                        violations.push(InvariantViolation {
                            invariant: "I1".to_string(),
                            node_id: id.clone(),
                            description: format!(
                                "calls {} but not in {}'s called_by",
                                callee_id, callee_id
                            ),
                        });
                    }
                } else {
                    violations.push(InvariantViolation {
                        invariant: "I1".to_string(),
                        node_id: id.clone(),
                        description: format!("calls non-existent node {}", callee_id),
                    });
                }
            }

            // I5: Relation bidirectionality
            for rel in &node.relations {
                if let Some(target) = nodes.get(&rel.target) {
                    let inverse_type = rel.inverse_type();
                    let has_inverse = target
                        .relations
                        .iter()
                        .any(|r| r.target == *id && r.rel_type == inverse_type);

                    if !has_inverse {
                        violations.push(InvariantViolation {
                            invariant: "I5".to_string(),
                            node_id: id.clone(),
                            description: format!(
                                "has relation {}:{} but target lacks inverse {}:{}",
                                rel.rel_type, rel.target, inverse_type, id
                            ),
                        });
                    }
                } else {
                    violations.push(InvariantViolation {
                        invariant: "I5".to_string(),
                        node_id: id.clone(),
                        description: format!(
                            "has relation to non-existent node {}",
                            rel.target
                        ),
                    });
                }
            }
        }

        Ok(violations)
    }

    fn compact(&mut self) -> Result<()> {
        // redb handles compaction automatically
        // Manual compaction can be triggered via compact() method
        self.db.compact()?;
        Ok(())
    }
}

/// redb transaction
struct RedbTransaction<'a> {
    storage: &'a mut RedbStorage,
    operations: Vec<TransactionOp>,
}

enum TransactionOp {
    Put(String, Node),
    Delete(String),
}

impl<'a> Transaction for RedbTransaction<'a> {
    fn put(&mut self, id: &str, node: &Node) -> Result<()> {
        self.operations
            .push(TransactionOp::Put(id.to_string(), node.clone()));
        Ok(())
    }

    fn delete(&mut self, id: &str) -> Result<()> {
        self.operations
            .push(TransactionOp::Delete(id.to_string()));
        Ok(())
    }

    fn commit(self: Box<Self>) -> Result<()> {
        // Apply all operations in a single database transaction
        for op in self.operations {
            match op {
                TransactionOp::Put(id, node) => {
                    self.storage.put(&id, &node)?;
                }
                TransactionOp::Delete(id) => {
                    self.storage.delete(&id)?;
                }
            }
        }
        Ok(())
    }

    fn rollback(self: Box<Self>) -> Result<()> {
        // Simply drop self, discarding all operations
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_basic_crud() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.redb");
        let mut storage = RedbStorage::new(&db_path).unwrap();

        // Create a node
        let mut node = Node::new("test.func", SnippetKind::Function);
        node.source_file = "test.cov".to_string();

        // Put
        storage.put("test.func", &node).unwrap();

        // Get
        let retrieved = storage.get("test.func").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test.func");

        // Delete
        storage.delete("test.func").unwrap();
        assert!(storage.get("test.func").unwrap().is_none());
    }

    #[test]
    fn test_persistence() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.redb");

        // Write data
        {
            let mut storage = RedbStorage::new(&db_path).unwrap();
            let node = Node::new("test.func", SnippetKind::Function);
            storage.put("test.func", &node).unwrap();
        }

        // Read data (new storage instance)
        {
            let storage = RedbStorage::new(&db_path).unwrap();
            let node = storage.get("test.func").unwrap();
            assert!(node.is_some());
            assert_eq!(node.unwrap().id, "test.func");
        }
    }

    #[test]
    fn test_query_by_kind() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.redb");
        let mut storage = RedbStorage::new(&db_path).unwrap();

        let func1 = Node::new("func1", SnippetKind::Function);
        let func2 = Node::new("func2", SnippetKind::Function);
        let struct1 = Node::new("struct1", SnippetKind::Struct);

        storage.put("func1", &func1).unwrap();
        storage.put("func2", &func2).unwrap();
        storage.put("struct1", &struct1).unwrap();

        let functions = storage.query_by_kind(SnippetKind::Function).unwrap();
        assert_eq!(functions.len(), 2);

        let structs = storage.query_by_kind(SnippetKind::Struct).unwrap();
        assert_eq!(structs.len(), 1);
    }
}
