//! Runtime types that mirror the WIT interface definitions

use serde::{Deserialize, Serialize};

/// A symbol in the runtime symbol store
///
/// This mirrors the `symbol` record in `runtime/wit/covenant-runtime.wit`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSymbol {
    /// Unique identifier (e.g., "auth.login", "db.query")
    pub id: String,

    /// Kind of snippet: "fn", "struct", "enum", "module", "database", "extern"
    pub kind: String,

    /// Source file path
    #[serde(default)]
    pub file: String,

    /// Line number in source
    #[serde(default)]
    pub line: u32,

    /// Forward references: functions this symbol calls
    pub calls: Vec<String>,

    /// Forward references: types this symbol references
    pub references: Vec<String>,

    /// Backward references: functions that call this symbol
    pub called_by: Vec<String>,

    /// Backward references: symbols that reference this type
    pub referenced_by: Vec<String>,

    /// Declared effects
    pub effects: Vec<String>,

    /// Transitive effect closure
    pub effect_closure: Vec<String>,

    /// Linked requirements
    pub requirements: Vec<String>,

    /// Linked tests
    pub tests: Vec<String>,

    /// For test snippets: requirements this test covers
    #[serde(default)]
    pub covers: Vec<String>,
}

impl RuntimeSymbol {
    /// Create a new empty symbol with the given ID and kind
    pub fn new(id: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            file: String::new(),
            line: 0,
            calls: Vec::new(),
            references: Vec::new(),
            called_by: Vec::new(),
            referenced_by: Vec::new(),
            effects: Vec::new(),
            effect_closure: Vec::new(),
            requirements: Vec::new(),
            tests: Vec::new(),
            covers: Vec::new(),
        }
    }

    /// Check if this symbol matches a filter
    pub fn matches(&self, filter: &SymbolFilter) -> bool {
        // Filter by kind
        if let Some(ref kind) = filter.kind {
            if &self.kind != kind {
                return false;
            }
        }

        // Filter by effect
        if let Some(ref effect) = filter.has_effect {
            if !self.effect_closure.contains(effect) {
                return false;
            }
        }

        // Filter by calls
        if let Some(ref fn_name) = filter.calls_fn {
            if !self.calls.contains(fn_name) {
                return false;
            }
        }

        // Filter by called_by
        if let Some(ref fn_name) = filter.called_by_fn {
            if !self.called_by.contains(fn_name) {
                return false;
            }
        }

        true
    }
}

/// Convert from covenant-symbols SymbolInfo to RuntimeSymbol
impl From<&covenant_symbols::SymbolInfo> for RuntimeSymbol {
    fn from(info: &covenant_symbols::SymbolInfo) -> Self {
        Self {
            id: info.name.clone(),
            kind: format!("{:?}", info.kind).to_lowercase(),
            file: String::new(), // Span doesn't track file path in current implementation
            line: info.span.start as u32,
            calls: info.calls.iter().cloned().collect(),
            references: info.references.iter().cloned().collect(),
            called_by: Vec::new(), // Will be populated separately
            referenced_by: Vec::new(),
            effects: info.declared_effects.clone(),
            effect_closure: Vec::new(), // Computed by effect checker
            requirements: info.requirements.clone(),
            tests: info.tests.clone(),
            covers: info.covers.clone(),
        }
    }
}

/// Filter criteria for querying symbols
///
/// This mirrors the `symbol-filter` record in `runtime/wit/covenant-runtime.wit`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SymbolFilter {
    /// Filter by kind (optional)
    pub kind: Option<String>,

    /// Filter by having a specific effect in effect_closure
    pub has_effect: Option<String>,

    /// Filter by calling a specific function
    pub calls_fn: Option<String>,

    /// Filter by being called by a specific function
    pub called_by_fn: Option<String>,
}

impl SymbolFilter {
    /// Create an empty filter that matches all symbols
    pub fn all() -> Self {
        Self::default()
    }

    /// Create a filter for a specific kind
    pub fn by_kind(kind: impl Into<String>) -> Self {
        Self {
            kind: Some(kind.into()),
            ..Default::default()
        }
    }

    /// Create a filter for symbols with a specific effect
    pub fn with_effect(effect: impl Into<String>) -> Self {
        Self {
            has_effect: Some(effect.into()),
            ..Default::default()
        }
    }
}
