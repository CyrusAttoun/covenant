//! Symbol graph data structure

use crate::{SymbolError, SymbolId, SymbolInfo, SymbolKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Status of invariant validations
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct InvariantStatus {
    /// I1: Bidirectionality (calls/called_by match)
    pub i1_bidirectionality: bool,
    /// I4: No circular imports
    pub i4_acyclicity: bool,
    /// I5: Relation bidirectionality
    pub i5_relation_bidirectionality: bool,
}

/// The complete symbol graph for a program
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SymbolGraph {
    /// All symbols indexed by numeric ID
    symbols: Vec<SymbolInfo>,

    /// Name to ID mapping for lookup
    by_name: HashMap<String, SymbolId>,

    /// Validated invariants
    pub invariants: InvariantStatus,
}

impl SymbolGraph {
    /// Create a new empty symbol graph
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a symbol by name
    pub fn get_by_name(&self, name: &str) -> Option<&SymbolInfo> {
        self.by_name.get(name).map(|id| &self.symbols[id.0 as usize])
    }

    /// Get a mutable symbol by name
    pub fn get_by_name_mut(&mut self, name: &str) -> Option<&mut SymbolInfo> {
        self.by_name
            .get(name)
            .map(|id| &mut self.symbols[id.0 as usize])
    }

    /// Get a symbol by ID
    pub fn get(&self, id: SymbolId) -> Option<&SymbolInfo> {
        self.symbols.get(id.0 as usize)
    }

    /// Get a mutable symbol by ID
    pub fn get_mut(&mut self, id: SymbolId) -> Option<&mut SymbolInfo> {
        self.symbols.get_mut(id.0 as usize)
    }

    /// Check if a symbol exists by name
    pub fn contains(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    /// Get the ID for a symbol name
    pub fn id_of(&self, name: &str) -> Option<SymbolId> {
        self.by_name.get(name).copied()
    }

    /// Insert a new symbol, returning error if duplicate
    pub fn insert(&mut self, mut symbol: SymbolInfo) -> Result<SymbolId, SymbolError> {
        if self.by_name.contains_key(&symbol.name) {
            return Err(SymbolError::DuplicateId {
                id: symbol.name.clone(),
                span: symbol.span,
            });
        }

        let id = SymbolId(self.symbols.len() as u32);
        symbol.id = id;
        self.by_name.insert(symbol.name.clone(), id);
        self.symbols.push(symbol);
        Ok(id)
    }

    /// Iterate over all symbols
    pub fn iter(&self) -> impl Iterator<Item = &SymbolInfo> {
        self.symbols.iter()
    }

    /// Iterate over all symbols mutably
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut SymbolInfo> {
        self.symbols.iter_mut()
    }

    /// Get the number of symbols in the graph
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Check if the graph is empty
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Get all function symbols
    pub fn functions(&self) -> impl Iterator<Item = &SymbolInfo> {
        self.symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
    }

    /// Get all type symbols (Struct, Enum)
    pub fn types(&self) -> impl Iterator<Item = &SymbolInfo> {
        self.symbols
            .iter()
            .filter(|s| matches!(s.kind, SymbolKind::Struct | SymbolKind::Enum))
    }

    /// Get all extern symbols
    pub fn externs(&self) -> impl Iterator<Item = &SymbolInfo> {
        self.symbols.iter().filter(|s| s.kind == SymbolKind::Extern)
    }

    /// Get callers of a symbol (by name)
    pub fn callers_of(&self, name: &str) -> Vec<String> {
        self.get_by_name(name)
            .map(|s| {
                s.called_by
                    .iter()
                    .filter_map(|id| self.get(*id).map(|s| s.name.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get callees of a symbol (by name)
    pub fn callees_of(&self, name: &str) -> Vec<String> {
        self.get_by_name(name)
            .map(|s| s.calls.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all symbol names
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.by_name.keys()
    }

    /// Collect all symbols with unresolved references
    pub fn unresolved_symbols(&self) -> Vec<&SymbolInfo> {
        self.symbols.iter().filter(|s| s.has_unresolved()).collect()
    }
}

/// Result of symbol graph building
#[derive(Debug)]
pub struct SymbolResult {
    /// The built symbol graph
    pub graph: SymbolGraph,
    /// Soft errors (undefined references, deferred to Phase 4)
    pub deferred_errors: Vec<SymbolError>,
}
