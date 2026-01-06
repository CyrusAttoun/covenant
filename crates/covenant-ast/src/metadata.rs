//! AST metadata for bidirectional references
//!
//! This metadata is computed by the checker and graph passes,
//! not during parsing.

use serde::{Deserialize, Serialize};

/// Unique identifier for a symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId(pub u32);

/// Unique identifier for an effect
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EffectId(pub u32);

/// Metadata computed for each symbol in the AST
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AstMetadata {
    /// The symbol's unique ID
    pub id: Option<SymbolId>,

    /// Functions that this function calls
    pub calls: Vec<SymbolId>,

    /// Functions that call this function
    pub called_by: Vec<SymbolId>,

    /// Types/symbols this references
    pub references: Vec<SymbolId>,

    /// What references this type/symbol
    pub referenced_by: Vec<SymbolId>,

    /// Computed effect set (transitive)
    pub effects: Vec<EffectId>,

    /// Is this function pure (no effects)?
    pub is_pure: bool,

    /// Is this symbol exported from its module?
    pub is_exported: bool,
}

impl AstMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_id(mut self, id: SymbolId) -> Self {
        self.id = Some(id);
        self
    }
}
