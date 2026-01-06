//! Symbol table for name resolution

use std::collections::HashMap;
use covenant_ast::SymbolId;
use crate::ResolvedType;

/// Symbol table mapping names to their definitions
#[derive(Debug, Default)]
pub struct SymbolTable {
    symbols: Vec<Symbol>,
    by_name: HashMap<String, SymbolId>,
    scopes: Vec<Scope>,
}

/// A symbol definition
#[derive(Debug, Clone)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub ty: ResolvedType,
}

/// Kind of symbol
#[derive(Debug, Clone)]
pub enum SymbolKind {
    Function {
        params: Vec<(String, ResolvedType)>,
        effects: Vec<String>,
    },
    Type,
    Variable { mutable: bool },
    Parameter,
    Field,
}

/// A scope in the symbol table
#[derive(Debug, Default)]
struct Scope {
    symbols: HashMap<String, SymbolId>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Define a new symbol
    pub fn define(&mut self, name: String, kind: SymbolKind, ty: ResolvedType) -> SymbolId {
        let id = SymbolId(self.symbols.len() as u32);
        let symbol = Symbol {
            id,
            name: name.clone(),
            kind,
            ty,
        };
        self.symbols.push(symbol);
        self.by_name.insert(name.clone(), id);

        if let Some(scope) = self.scopes.last_mut() {
            scope.symbols.insert(name, id);
        }

        id
    }

    /// Look up a symbol by name
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        // Search from innermost scope outward
        for scope in self.scopes.iter().rev() {
            if let Some(&id) = scope.symbols.get(name) {
                return self.get(id);
            }
        }
        // Fall back to global lookup
        self.by_name.get(name).and_then(|&id| self.get(id))
    }

    /// Get a symbol by ID
    pub fn get(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.0 as usize)
    }

    /// Enter a new scope
    pub fn enter_scope(&mut self) {
        self.scopes.push(Scope::default());
    }

    /// Exit the current scope
    pub fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    /// Iterate over all symbols
    pub fn iter(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.iter()
    }

    /// Get all function symbols
    pub fn functions(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.iter().filter(|s| matches!(s.kind, SymbolKind::Function { .. }))
    }
}
