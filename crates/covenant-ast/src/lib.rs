//! Covenant AST - Core types for the abstract syntax tree
//!
//! This crate defines all AST node types, spans for source locations,
//! and metadata structures for bidirectional references.

mod span;
mod types;
mod expr;
mod stmt;
mod decl;
mod query;
mod metadata;
mod snippet;

pub use span::*;
pub use types::*;
pub use expr::*;
pub use stmt::*;
pub use decl::*;
pub use query::*;
pub use metadata::*;
pub use snippet::*;

use serde::{Deserialize, Serialize};

/// A complete Covenant program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Program {
    /// Legacy mode - traditional declarations
    Legacy {
        declarations: Vec<Declaration>,
        span: Span,
    },
    /// Snippet mode - IR-based snippets
    Snippets {
        snippets: Vec<Snippet>,
        span: Span,
    },
}
