//! Statement AST nodes

use serde::{Deserialize, Serialize};
use crate::{Span, Type, Expr};

/// A block of statements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

/// A statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatementKind {
    /// Let binding: `let x = 5` or `let mut x: Int = 5`
    Let {
        name: String,
        mutable: bool,
        ty: Option<Type>,
        value: Expr,
    },

    /// Return statement: `return x`
    Return(Option<Expr>),

    /// Expression statement: `foo()`
    Expr(Expr),

    /// For loop: `for x in items { ... }`
    For {
        binding: String,
        iterable: Expr,
        body: Block,
    },
}

/// Pattern for match expressions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternKind {
    /// Wildcard: `_`
    Wildcard,

    /// Binding: `x`
    Binding(String),

    /// Literal: `42`, `"hello"`
    Literal(crate::Literal),

    /// Variant: `Some(x)`, `Error { code }`
    Variant {
        path: crate::TypePath,
        fields: PatternFields,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternFields {
    /// Positional: `Some(x, y)`
    Positional(Vec<Pattern>),
    /// Named: `Error { code, message }`
    Named(Vec<(String, Pattern)>),
    /// Unit: `None`
    Unit,
}

/// A match arm: `pattern => expr`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}
