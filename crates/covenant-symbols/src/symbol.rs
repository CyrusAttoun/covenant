//! Symbol information and types

use covenant_ast::{Span, SnippetKind};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Unique identifier for a symbol in the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId(pub u32);

/// Kind of symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Module,
    Database,
    Extern,
    /// Platform-abstract extern declaration
    ExternAbstract,
    /// Platform-specific extern implementation
    ExternImpl,
    Test,
    Data,
}

impl From<SnippetKind> for SymbolKind {
    fn from(kind: SnippetKind) -> Self {
        match kind {
            SnippetKind::Function => SymbolKind::Function,
            SnippetKind::Struct => SymbolKind::Struct,
            SnippetKind::Enum => SymbolKind::Enum,
            SnippetKind::Module => SymbolKind::Module,
            SnippetKind::Database => SymbolKind::Database,
            SnippetKind::Extern => SymbolKind::Extern,
            SnippetKind::ExternAbstract => SymbolKind::ExternAbstract,
            SnippetKind::ExternImpl => SymbolKind::ExternImpl,
            SnippetKind::Test => SymbolKind::Test,
            SnippetKind::Data => SymbolKind::Data,
        }
    }
}

/// A relation reference to another symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationRef {
    /// Target snippet ID
    pub target: String,
    /// Relation type (e.g., "contains", "describes")
    pub relation_type: String,
}

/// Information about a symbol extracted from the AST
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    /// Unique ID assigned during extraction
    pub id: SymbolId,

    /// The snippet's string ID (e.g., "math.add", "config.Config")
    pub name: String,

    /// Kind of symbol (Function, Struct, Enum, etc.)
    pub kind: SymbolKind,

    /// Source location
    pub span: Span,

    // === Forward References (extracted in Pass 1) ===
    /// Function/method calls this symbol makes
    pub calls: HashSet<String>,

    /// Type references (parameter types, return types, field types)
    pub references: HashSet<String>,

    /// Declared effects (from effects section)
    pub declared_effects: Vec<String>,

    /// Relations declared (from relations section)
    pub relations_to: Vec<RelationRef>,

    // === Backward References (computed in Pass 2) ===
    /// Symbols that call this symbol
    pub called_by: HashSet<SymbolId>,

    /// Symbols that reference this type
    pub referenced_by: HashSet<SymbolId>,

    /// Inverse relations (computed from relations_to)
    pub relations_from: Vec<RelationRef>,

    // === Resolution State ===
    /// Unresolved call references (for deferred error handling)
    pub unresolved_calls: HashSet<String>,

    /// Unresolved type references (for deferred error handling)
    pub unresolved_references: HashSet<String>,

    // === Platform Abstraction (for extern-abstract and extern-impl) ===
    /// For extern-impl: the abstract snippet ID this implements
    pub implements: Option<String>,

    /// For extern-impl: the target platform
    pub target_platform: Option<String>,
}

impl SymbolInfo {
    /// Create a new symbol with the given name, kind, and span
    pub fn new(name: String, kind: SymbolKind, span: Span) -> Self {
        Self {
            id: SymbolId(0), // Will be assigned during graph insertion
            name,
            kind,
            span,
            calls: HashSet::new(),
            references: HashSet::new(),
            declared_effects: Vec::new(),
            relations_to: Vec::new(),
            called_by: HashSet::new(),
            referenced_by: HashSet::new(),
            relations_from: Vec::new(),
            unresolved_calls: HashSet::new(),
            unresolved_references: HashSet::new(),
            implements: None,
            target_platform: None,
        }
    }

    /// Check if this symbol is a callable (function or extern)
    pub fn is_callable(&self) -> bool {
        matches!(
            self.kind,
            SymbolKind::Function | SymbolKind::Extern | SymbolKind::ExternAbstract
        )
    }

    /// Check if this symbol is a type definition
    pub fn is_type(&self) -> bool {
        matches!(self.kind, SymbolKind::Struct | SymbolKind::Enum)
    }

    /// Check if this symbol has any unresolved references
    pub fn has_unresolved(&self) -> bool {
        !self.unresolved_calls.is_empty() || !self.unresolved_references.is_empty()
    }
}
