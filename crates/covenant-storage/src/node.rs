//! Node schema for storage

use serde::{Deserialize, Serialize};

/// A stored node representing a Covenant snippet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier (e.g., "auth.login", "docs.overview")
    pub id: String,

    /// The kind of snippet
    pub kind: SnippetKind,

    /// Version for optimistic locking
    pub version: u64,

    /// Source file path
    pub source_file: String,

    /// Line number where snippet starts
    pub line_start: usize,

    /// Line number where snippet ends
    pub line_end: usize,

    /// SHA-256 hash of source content
    pub content_hash: String,

    /// Full AST as JSON string (from covenant-ast types)
    pub ast: String,

    /// Functions/symbols this calls
    pub calls: Vec<String>,

    /// Functions/symbols that call this
    pub called_by: Vec<String>,

    /// Types/symbols this references
    pub references: Vec<String>,

    /// What references this
    pub referenced_by: Vec<String>,

    /// Declared effects
    pub effects: Vec<String>,

    /// Transitive effect closure
    pub effect_closure: Vec<String>,

    /// Requirements this implements
    pub requirements: Vec<String>,

    /// Tests covering this
    pub tests: Vec<String>,

    /// Custom bidirectional relations
    pub relations: Vec<Relation>,

    /// Notes/documentation
    pub notes: Vec<Note>,
}

impl Node {
    /// Create a new node with given ID and kind
    pub fn new(id: impl Into<String>, kind: SnippetKind) -> Self {
        Self {
            id: id.into(),
            kind,
            version: 0,
            source_file: String::new(),
            line_start: 0,
            line_end: 0,
            content_hash: String::new(),
            ast: "null".to_string(),
            calls: Vec::new(),
            called_by: Vec::new(),
            references: Vec::new(),
            referenced_by: Vec::new(),
            effects: Vec::new(),
            effect_closure: Vec::new(),
            requirements: Vec::new(),
            tests: Vec::new(),
            relations: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Increment version (for optimistic locking)
    pub fn increment_version(&mut self) {
        self.version += 1;
    }

    /// Check if this node has a specific effect
    pub fn has_effect(&self, effect: &str) -> bool {
        self.effect_closure.iter().any(|e| e == effect)
    }

    /// Get all relation targets of a specific type
    pub fn get_relations(&self, rel_type: &str) -> Vec<&str> {
        self.relations
            .iter()
            .filter(|r| r.rel_type == rel_type)
            .map(|r| r.target.as_str())
            .collect()
    }

    /// Get the AST as a parsed JSON value
    ///
    /// This parses the JSON string into a `serde_json::Value` for manipulation.
    /// Most queries won't need to call this - they can use the metadata fields directly.
    pub fn get_ast(&self) -> crate::error::Result<serde_json::Value> {
        serde_json::from_str(&self.ast)
            .map_err(|e| crate::error::StorageError::InvalidJson(e.to_string()))
    }

    /// Set the AST from a JSON value
    ///
    /// This serializes the value to a JSON string and stores it.
    pub fn set_ast(&mut self, value: &serde_json::Value) -> crate::error::Result<()> {
        self.ast = serde_json::to_string(value)
            .map_err(|e| crate::error::StorageError::Serialization(
                bincode::Error::from(bincode::ErrorKind::Custom(e.to_string()))
            ))?;
        Ok(())
    }

    /// Set the AST directly from a JSON string
    ///
    /// This is useful when the compiler already has a JSON string from serializing
    /// covenant-ast types. Avoids double-parsing.
    pub fn set_ast_json(&mut self, json: String) {
        self.ast = json;
    }
}

/// The kind of snippet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SnippetKind {
    /// Function declaration
    Function,
    /// Struct definition
    Struct,
    /// Enum definition
    Enum,
    /// Module
    Module,
    /// Database binding
    Database,
    /// External binding
    Extern,
    /// Data node (pure data/documentation)
    Data,
    /// Requirement specification
    Requirement,
    /// Test
    Test,
}

/// A bidirectional relation between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    /// Target node ID
    pub target: String,
    /// Relation type (e.g., "describes", "contains", "implements")
    pub rel_type: String,
}

impl Relation {
    pub fn new(target: impl Into<String>, rel_type: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            rel_type: rel_type.into(),
        }
    }

    /// Get the inverse relation type
    pub fn inverse_type(&self) -> &str {
        match self.rel_type.as_str() {
            "describes" => "described_by",
            "described_by" => "describes",
            "contains" => "contained_by",
            "contained_by" => "contains",
            "implements" => "implemented_by",
            "implemented_by" => "implements",
            "elaborates_on" => "elaborated_by",
            "elaborated_by" => "elaborates_on",
            "contrasts_with" => "contrasts_with", // symmetric
            "related_to" => "related_to",         // symmetric
            "example_of" => "has_example",
            "has_example" => "example_of",
            "depends_on" => "depended_by",
            "depended_by" => "depends_on",
            _ => "related_to", // fallback
        }
    }
}

/// A note/documentation attached to a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// Language tag (e.g., "en", "es", "pseudo")
    pub lang: Option<String>,
    /// Note content
    pub content: String,
}

impl Note {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            lang: None,
            content: content.into(),
        }
    }

    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = Some(lang.into());
        self
    }
}
