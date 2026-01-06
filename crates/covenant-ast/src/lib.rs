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

pub use span::*;
pub use types::*;
pub use expr::*;
pub use stmt::*;
pub use decl::*;
pub use query::*;
pub use metadata::*;

use serde::{Deserialize, Serialize};

/// A complete Covenant program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub declarations: Vec<Declaration>,
    pub span: Span,
}
