//! Query expression AST nodes

use serde::{Deserialize, Serialize};
use crate::{Span, Expr};

/// Body of a query expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryBody {
    pub select: SelectClause,
    pub from: Option<FromClause>,
    pub joins: Vec<JoinClause>,
    pub where_clause: Option<Expr>,
    pub order_by: Vec<OrderItem>,
    pub limit: Option<Expr>,
    pub offset: Option<Expr>,
    pub span: Span,
}

/// SELECT clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectClause {
    pub items: SelectItems,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SelectItems {
    /// SELECT *
    Star,
    /// SELECT expr AS alias, ...
    List(Vec<SelectItem>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectItem {
    pub expr: Expr,
    pub alias: Option<String>,
    pub span: Span,
}

/// FROM clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FromClause {
    pub table: String,
    pub alias: Option<String>,
    pub span: Span,
}

/// JOIN clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinClause {
    pub kind: JoinKind,
    pub table: String,
    pub condition: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JoinKind {
    Inner,
    Left,
    Right,
    Outer,
}

/// ORDER BY item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub expr: Expr,
    pub direction: OrderDirection,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OrderDirection {
    #[default]
    Asc,
    Desc,
}
