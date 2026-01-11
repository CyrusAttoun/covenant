//! Expression AST nodes

use serde::{Deserialize, Serialize};
use crate::{Span, Type, TypePath, Block, MatchArm, QueryBody};

/// An expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExprKind {
    /// Literal value: `42`, `"hello"`, `true`, `none`
    Literal(Literal),

    /// Identifier: `x`, `user`
    Ident(String),

    /// Binary operation: `a + b`, `x = y`
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    /// Unary operation: `!x`, `-y`
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    /// Assignment: `x := 5`
    Assign {
        target: String,
        value: Box<Expr>,
    },

    /// Function call: `foo(a, b)`
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },

    /// Field access: `user.name`
    Field {
        object: Box<Expr>,
        field: String,
    },

    /// Index access: `arr[0]`
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },

    /// Array literal: `[1, 2, 3]`
    Array(Vec<Expr>),

    /// Struct literal: `User { name: "Alice", age: 30 }`
    Struct {
        path: Option<TypePath>,
        fields: Vec<FieldInit>,
    },

    /// Block expression: `{ stmt; stmt; expr }`
    Block(Block),

    /// Closure: `|x, y| x + y`
    Closure {
        params: Vec<ClosureParam>,
        body: Box<Expr>,
    },

    /// Handle expression (error catching): `expr handle { Err(e) => ... }`
    Handle {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
    },

    /// Query expression: `query app_db { select * from users }`
    Query {
        target: TypePath,
        body: Box<QueryBody>,
    },

    /// Insert expression: `insert into app_db.users { name, email }`
    Insert {
        target: TypePath,
        value: Box<Expr>,
    },

    /// Update expression: `update app_db.users set name: "Bob" where id = 1`
    Update {
        target: TypePath,
        assignments: Vec<FieldInit>,
        condition: Option<Box<Expr>>,
    },

    /// Delete expression: `delete from app_db.users where id = 1`
    Delete {
        target: TypePath,
        condition: Option<Box<Expr>>,
    },

    /// If expression: `if cond { ... } else { ... }`
    If {
        condition: Box<Expr>,
        then_branch: Block,
        else_branch: Option<Box<Expr>>,
    },

    /// Match expression: `match x { ... }`
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
}

/// A literal value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    None,
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    // Comparison (note: = is equality, not ==)
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Logical
    And,
    Or,

    // Special
    Contains,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

/// Field initialization in struct literal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInit {
    pub name: String,
    /// None means shorthand: `{ name }` same as `{ name: name }`
    pub value: Option<Expr>,
    pub span: Span,
}

/// Closure parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosureParam {
    pub name: String,
    pub ty: Option<Type>,
    pub span: Span,
}
