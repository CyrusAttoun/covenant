//! In-memory storage implementation for testing

use crate::{Node, Result, SnippetKind, StorageProvider, Transaction, InvariantViolation};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// In-memory storage implementation
///
/// Fast, non-persistent storage primarily for testing.
/// All data is lost when the storage is dropped.
pub struct InMemoryStorage {
    nodes: Arc<RwLock<HashMap<String, Node>>>,
    kind_index: Arc<RwLock<HashMap<SnippetKind, HashSet<String>>>>,
    effect_index: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    relation_index: Arc<RwLock<HashMap<(String, String), HashSet<String>>>>,
}

impl InMemoryStorage {
    /// Create a new empty in-memory storage
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            kind_index: Arc::new(RwLock::new(HashMap::new())),
            effect_index: Arc::new(RwLock::new(HashMap::new())),
            relation_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update secondary indexes after a node is inserted/updated
    fn update_indexes(&mut self, id: &str, node: &Node) -> Result<()> {
        // Update kind index
        self.kind_index
            .write()
            .unwrap()
            .entry(node.kind)
            .or_insert_with(HashSet::new)
            .insert(id.to_string());

        // Update effect index
        let mut effect_index = self.effect_index.write().unwrap();
        for effect in &node.effect_closure {
            effect_index
                .entry(effect.clone())
                .or_insert_with(HashSet::new)
                .insert(id.to_string());
        }
        drop(effect_index);

        // Update relation index
        let mut relation_index = self.relation_index.write().unwrap();
        for rel in &node.relations {
            let key = (rel.target.clone(), rel.rel_type.clone());
            relation_index
                .entry(key)
                .or_insert_with(HashSet::new)
                .insert(id.to_string());
        }
        drop(relation_index);

        Ok(())
    }

    /// Remove a node from all secondary indexes
    fn remove_from_indexes(&mut self, id: &str, node: &Node) -> Result<()> {
        // Remove from kind index
        if let Some(ids) = self.kind_index.write().unwrap().get_mut(&node.kind) {
            ids.remove(id);
        }

        // Remove from effect index
        let mut effect_index = self.effect_index.write().unwrap();
        for effect in &node.effect_closure {
            if let Some(ids) = effect_index.get_mut(effect) {
                ids.remove(id);
            }
        }
        drop(effect_index);

        // Remove from relation index
        let mut relation_index = self.relation_index.write().unwrap();
        for rel in &node.relations {
            let key = (rel.target.clone(), rel.rel_type.clone());
            if let Some(ids) = relation_index.get_mut(&key) {
                ids.remove(id);
            }
        }
        drop(relation_index);

        Ok(())
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageProvider for InMemoryStorage {
    fn get(&self, id: &str) -> Result<Option<Node>> {
        Ok(self.nodes.read().unwrap().get(id).cloned())
    }

    fn put(&mut self, id: &str, node: &Node) -> Result<()> {
        // Remove old version from indexes if it exists
        let old_node = self.nodes.read().unwrap().get(id).cloned();
        if let Some(old) = old_node {
            self.remove_from_indexes(id, &old)?;
        }

        // Insert new version
        self.nodes
            .write()
            .unwrap()
            .insert(id.to_string(), node.clone());

        // Update indexes
        self.update_indexes(id, node)?;

        Ok(())
    }

    fn delete(&mut self, id: &str) -> Result<()> {
        let node = self.nodes.write().unwrap().remove(id);
        if let Some(n) = node {
            self.remove_from_indexes(id, &n)?;
        }
        Ok(())
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let nodes = self.nodes.read().unwrap();
        let mut ids: Vec<String> = nodes
            .keys()
            .filter(|id| id.starts_with(prefix))
            .cloned()
            .collect();
        ids.sort();
        Ok(ids)
    }

    fn query_by_kind(&self, kind: SnippetKind) -> Result<Vec<Node>> {
        let kind_index = self.kind_index.read().unwrap();
        let Some(ids) = kind_index.get(&kind) else {
            return Ok(Vec::new());
        };

        let nodes = self.nodes.read().unwrap();
        let mut results = Vec::new();
        for id in ids {
            if let Some(node) = nodes.get(id) {
                results.push(node.clone());
            }
        }

        Ok(results)
    }

    fn query_by_effect(&self, effect: &str) -> Result<Vec<Node>> {
        let effect_index = self.effect_index.read().unwrap();
        let Some(ids) = effect_index.get(effect) else {
            return Ok(Vec::new());
        };

        let nodes = self.nodes.read().unwrap();
        let mut results = Vec::new();
        for id in ids {
            if let Some(node) = nodes.get(id) {
                results.push(node.clone());
            }
        }

        Ok(results)
    }

    fn query_by_relation(&self, target_id: &str, rel_type: &str) -> Result<Vec<Node>> {
        let relation_index = self.relation_index.read().unwrap();
        let key = (target_id.to_string(), rel_type.to_string());
        let Some(ids) = relation_index.get(&key) else {
            return Ok(Vec::new());
        };

        let nodes = self.nodes.read().unwrap();
        let mut results = Vec::new();
        for id in ids {
            if let Some(node) = nodes.get(id) {
                results.push(node.clone());
            }
        }

        Ok(results)
    }

    fn begin_transaction(&mut self) -> Result<Box<dyn Transaction + '_>> {
        Ok(Box::new(InMemoryTransaction {
            storage: self,
            operations: Vec::new(),
        }))
    }

    fn rebuild_indexes(&mut self) -> Result<()> {
        // Clear indexes
        self.kind_index.write().unwrap().clear();
        self.effect_index.write().unwrap().clear();
        self.relation_index.write().unwrap().clear();

        // Rebuild from nodes (clone to avoid borrow issues)
        let nodes: Vec<(String, Node)> = self
            .nodes
            .read()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        for (id, node) in &nodes {
            self.update_indexes(id, node)?;
        }

        Ok(())
    }

    fn verify_invariants(&self) -> Result<Vec<InvariantViolation>> {
        let mut violations = Vec::new();
        let nodes = self.nodes.read().unwrap();

        for (id, node) in nodes.iter() {
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
        // No-op for in-memory storage (no fragmentation)
        Ok(())
    }
}

/// In-memory transaction
struct InMemoryTransaction<'a> {
    storage: &'a mut InMemoryStorage,
    operations: Vec<TransactionOp>,
}

enum TransactionOp {
    Put(String, Node),
    Delete(String),
}

impl<'a> Transaction for InMemoryTransaction<'a> {
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
        // Apply all operations
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

    #[test]
    fn test_basic_crud() {
        let mut storage = InMemoryStorage::new();

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
    fn test_query_by_kind() {
        let mut storage = InMemoryStorage::new();

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

    #[test]
    fn test_invariant_verification() {
        let mut storage = InMemoryStorage::new();

        // Create two nodes with bidirectional call relationship
        let mut caller = Node::new("caller", SnippetKind::Function);
        caller.calls.push("callee".to_string());

        let mut callee = Node::new("callee", SnippetKind::Function);
        callee.called_by.push("caller".to_string());

        storage.put("caller", &caller).unwrap();
        storage.put("callee", &callee).unwrap();

        let violations = storage.verify_invariants().unwrap();
        assert_eq!(violations.len(), 0);

        // Break the invariant
        caller.calls.push("nonexistent".to_string());
        storage.put("caller", &caller).unwrap();

        let violations = storage.verify_invariants().unwrap();
        assert!(violations.len() > 0);
    }
}
