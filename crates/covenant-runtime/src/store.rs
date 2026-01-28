//! Symbol Store - In-memory storage for the symbol graph
//!
//! This module implements the `symbols` interface from WIT.
//! It provides CRUD operations for symbols with version tracking.

use crate::error::RuntimeError;
use crate::types::{RuntimeSymbol, SymbolFilter};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// In-memory symbol store with versioning
///
/// This is the runtime implementation of the symbol graph.
/// It supports:
/// - CRUD operations for symbols
/// - Filtering and querying
/// - Version tracking for cache invalidation
pub struct SymbolStore {
    /// All symbols indexed by ID
    symbols: HashMap<String, RuntimeSymbol>,

    /// Current version (incremented on every mutation)
    version: AtomicU64,
}

impl SymbolStore {
    /// Create a new empty symbol store
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            version: AtomicU64::new(0),
        }
    }

    /// Get the current version of the symbol store
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    /// Increment the version and return the new value
    fn bump_version(&self) -> u64 {
        self.version.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Get a symbol by ID
    pub fn get(&self, id: &str) -> Option<&RuntimeSymbol> {
        self.symbols.get(id)
    }

    /// List all symbols matching a filter
    pub fn list(&self, filter: &SymbolFilter) -> Vec<&RuntimeSymbol> {
        self.symbols
            .values()
            .filter(|sym| sym.matches(filter))
            .collect()
    }

    /// List all symbols (no filter)
    pub fn list_all(&self) -> Vec<&RuntimeSymbol> {
        self.symbols.values().collect()
    }

    /// Insert or update a symbol
    ///
    /// Returns the new version number
    pub fn upsert(&mut self, symbol: RuntimeSymbol) -> u64 {
        self.symbols.insert(symbol.id.clone(), symbol);
        self.bump_version()
    }

    /// Delete a symbol by ID
    ///
    /// Returns true if the symbol existed and was deleted
    pub fn delete(&mut self, id: &str) -> bool {
        let existed = self.symbols.remove(id).is_some();
        if existed {
            self.bump_version();
        }
        existed
    }

    /// Check if a symbol exists
    pub fn contains(&self, id: &str) -> bool {
        self.symbols.contains_key(id)
    }

    /// Get the number of symbols
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Clear all symbols
    pub fn clear(&mut self) {
        self.symbols.clear();
        self.bump_version();
    }

    /// Load symbols from a covenant-symbols SymbolGraph
    pub fn load_from_graph(&mut self, graph: &covenant_symbols::SymbolGraph) {
        self.symbols.clear();

        // First pass: create all symbols
        for info in graph.iter() {
            let symbol = RuntimeSymbol::from(info);
            self.symbols.insert(symbol.id.clone(), symbol);
        }

        // Second pass: populate called_by references
        // (The SymbolInfo has called_by as SymbolId, we need to convert to names)
        for info in graph.iter() {
            let caller_name = info.name.clone();
            for callee_name in &info.calls {
                if let Some(callee) = self.symbols.get_mut(callee_name) {
                    if !callee.called_by.contains(&caller_name) {
                        callee.called_by.push(caller_name.clone());
                    }
                }
            }
        }

        self.bump_version();
    }

    /// Update backward references after symbol mutations
    ///
    /// This should be called after adding/updating symbols to ensure
    /// called_by and referenced_by are accurate.
    pub fn recompute_backward_refs(&mut self) {
        // Collect all forward references first
        let forward_refs: Vec<(String, Vec<String>)> = self
            .symbols
            .values()
            .map(|s| (s.id.clone(), s.calls.clone()))
            .collect();

        // Clear all backward references
        for symbol in self.symbols.values_mut() {
            symbol.called_by.clear();
            symbol.referenced_by.clear();
        }

        // Recompute from forward references
        for (caller_id, callees) in forward_refs {
            for callee_id in callees {
                if let Some(callee) = self.symbols.get_mut(&callee_id) {
                    if !callee.called_by.contains(&caller_id) {
                        callee.called_by.push(caller_id.clone());
                    }
                }
            }
        }
    }

    /// Load symbols from embedded WASM metadata (JSON format)
    ///
    /// This method parses JSON-serialized symbol metadata extracted from
    /// a WASM module's data section via the `_cov_get_symbol_metadata` export.
    ///
    /// The JSON format is an array of symbol objects matching the RuntimeSymbol
    /// structure (or the EmbeddableSymbol format from covenant-codegen).
    pub fn load_from_json(&mut self, json_bytes: &[u8]) -> Result<(), RuntimeError> {
        let symbols: Vec<RuntimeSymbol> = serde_json::from_slice(json_bytes)
            .map_err(|e| RuntimeError::DeserializationFailed(e.to_string()))?;

        self.symbols.clear();
        for symbol in symbols {
            self.symbols.insert(symbol.id.clone(), symbol);
        }

        self.bump_version();
        Ok(())
    }
}

impl Default for SymbolStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_store_is_empty() {
        let store = SymbolStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.version(), 0);
    }

    #[test]
    fn test_upsert_and_get() {
        let mut store = SymbolStore::new();
        let symbol = RuntimeSymbol::new("test.foo", "fn");

        let version = store.upsert(symbol);
        assert_eq!(version, 1);
        assert_eq!(store.len(), 1);

        let retrieved = store.get("test.foo").unwrap();
        assert_eq!(retrieved.id, "test.foo");
        assert_eq!(retrieved.kind, "fn");
    }

    #[test]
    fn test_delete() {
        let mut store = SymbolStore::new();
        store.upsert(RuntimeSymbol::new("test.foo", "fn"));

        assert!(store.delete("test.foo"));
        assert!(!store.contains("test.foo"));
        assert_eq!(store.version(), 2); // upsert + delete
    }

    #[test]
    fn test_filter_by_kind() {
        let mut store = SymbolStore::new();
        store.upsert(RuntimeSymbol::new("test.foo", "fn"));
        store.upsert(RuntimeSymbol::new("test.Bar", "struct"));
        store.upsert(RuntimeSymbol::new("test.baz", "fn"));

        let filter = SymbolFilter::by_kind("fn");
        let results = store.list(&filter);

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|s| s.kind == "fn"));
    }

    #[test]
    fn test_filter_by_effect() {
        let mut store = SymbolStore::new();

        let mut sym1 = RuntimeSymbol::new("test.db_fn", "fn");
        sym1.effect_closure = vec!["database".into()];
        store.upsert(sym1);

        let mut sym2 = RuntimeSymbol::new("test.pure_fn", "fn");
        sym2.effect_closure = vec![];
        store.upsert(sym2);

        let filter = SymbolFilter::with_effect("database");
        let results = store.list(&filter);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "test.db_fn");
    }

    #[test]
    fn test_version_increments() {
        let mut store = SymbolStore::new();
        assert_eq!(store.version(), 0);

        store.upsert(RuntimeSymbol::new("a", "fn"));
        assert_eq!(store.version(), 1);

        store.upsert(RuntimeSymbol::new("b", "fn"));
        assert_eq!(store.version(), 2);

        store.delete("a");
        assert_eq!(store.version(), 3);

        // Deleting non-existent doesn't bump version
        store.delete("nonexistent");
        assert_eq!(store.version(), 3);
    }
}
