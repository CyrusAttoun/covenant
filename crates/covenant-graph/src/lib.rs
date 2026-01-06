//! Covenant Graph - Bidirectional reference tracking and queries
//!
//! This crate builds the bidirectional reference graph and provides
//! a query interface for navigating the codebase.

mod graph;
mod query;

pub use graph::*;
pub use query::*;

use std::collections::{HashMap, HashSet};
use covenant_ast::{SymbolId, EffectId, AstMetadata};
use covenant_checker::{SymbolTable, EffectTable};
use serde::{Deserialize, Serialize};

/// The complete reference graph for a program
#[derive(Debug, Default)]
pub struct ReferenceGraph {
    /// Forward edges: who does this symbol call?
    pub calls: HashMap<SymbolId, HashSet<SymbolId>>,
    /// Backward edges: who calls this symbol?
    pub called_by: HashMap<SymbolId, HashSet<SymbolId>>,
    /// Forward type refs: what types does this symbol reference?
    pub references: HashMap<SymbolId, HashSet<SymbolId>>,
    /// Backward type refs: what references this type?
    pub referenced_by: HashMap<SymbolId, HashSet<SymbolId>>,
    /// Computed effects for each symbol
    pub effects: HashMap<SymbolId, HashSet<EffectId>>,
}

impl ReferenceGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a call edge
    pub fn add_call(&mut self, caller: SymbolId, callee: SymbolId) {
        self.calls.entry(caller).or_default().insert(callee);
        self.called_by.entry(callee).or_default().insert(caller);
    }

    /// Add a type reference edge
    pub fn add_reference(&mut self, referrer: SymbolId, referenced: SymbolId) {
        self.references.entry(referrer).or_default().insert(referenced);
        self.referenced_by.entry(referenced).or_default().insert(referrer);
    }

    /// Get all symbols that call the given symbol
    pub fn callers_of(&self, symbol: SymbolId) -> HashSet<SymbolId> {
        self.called_by.get(&symbol).cloned().unwrap_or_default()
    }

    /// Get all symbols that the given symbol calls
    pub fn callees_of(&self, symbol: SymbolId) -> HashSet<SymbolId> {
        self.calls.get(&symbol).cloned().unwrap_or_default()
    }

    /// Check if a symbol is dead code (not called by anything and not exported)
    pub fn is_dead_code(&self, symbol: SymbolId, is_exported: bool, is_entry: bool) -> bool {
        if is_exported || is_entry {
            return false;
        }
        self.callers_of(symbol).is_empty()
    }

    /// Get metadata for a symbol
    pub fn metadata_for(&self, symbol: SymbolId, is_exported: bool) -> AstMetadata {
        let calls: Vec<SymbolId> = self.callees_of(symbol).into_iter().collect();
        let called_by: Vec<SymbolId> = self.callers_of(symbol).into_iter().collect();
        let references: Vec<SymbolId> = self.references.get(&symbol).cloned().unwrap_or_default().into_iter().collect();
        let referenced_by: Vec<SymbolId> = self.referenced_by.get(&symbol).cloned().unwrap_or_default().into_iter().collect();
        let effects: Vec<EffectId> = self.effects.get(&symbol).cloned().unwrap_or_default().into_iter().collect();
        let is_pure = effects.is_empty();

        AstMetadata {
            id: Some(symbol),
            calls,
            called_by,
            references,
            referenced_by,
            effects,
            is_pure,
            is_exported,
        }
    }
}

/// Build a reference graph from the symbol and effect tables
pub fn build_graph(symbols: &SymbolTable, effects: &EffectTable) -> ReferenceGraph {
    let mut graph = ReferenceGraph::new();

    // Copy effect information
    for symbol in symbols.iter() {
        let effect_set = effects.effects_of(symbol.id);
        if !effect_set.is_empty() {
            graph.effects.insert(symbol.id, effect_set);
        }
    }

    // TODO: Build call graph from AST traversal
    // For now, the checker would need to record calls during type checking

    graph
}
