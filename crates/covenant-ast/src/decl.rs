//! Declaration AST nodes (top-level items)

use serde::{Deserialize, Serialize};
use crate::{Span, Type, TypePath, Block, Expr};

/// A top-level declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Declaration {
    pub kind: DeclarationKind,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeclarationKind {
    /// Module-level import: `import { foo, bar } from baz`
    Import(ImportDecl),

    /// Module declaration: `module foo { ... }`
    Module(ModuleDecl),

    /// Struct declaration: `struct User { ... }`
    Struct(StructDecl),

    /// Enum declaration: `enum Status { ... }`
    Enum(EnumDecl),

    /// Type alias: `type UserId = Int`
    TypeAlias(TypeAliasDecl),

    /// Function declaration: `foo(x: Int) -> Int { ... }`
    Function(FunctionDecl),

    /// External binding: `extern foo(...) -> T from "lib" effect [...]`
    Extern(ExternDecl),

    /// Database declaration: `database app_db { ... }`
    Database(DatabaseDecl),
}

/// Import declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDecl {
    pub names: Vec<String>,
    pub source: String,
    pub span: Span,
}

/// Module declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDecl {
    pub name: String,
    pub declarations: Vec<Declaration>,
    pub span: Span,
}

/// Struct declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

/// Field in a struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDecl {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
    pub span: Span,
}

/// Enum declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub variants: Vec<VariantDecl>,
    pub span: Span,
}

/// Enum variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantDecl {
    pub name: String,
    pub fields: VariantFields,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariantFields {
    /// Unit variant: `None`
    Unit,
    /// Tuple variant: `Some(T)`
    Tuple(Vec<Type>),
    /// Struct variant: `Error { code: Int, message: String }`
    Struct(Vec<FieldDecl>),
}

/// Type alias declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeAliasDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub ty: Type,
    pub span: Span,
}

/// Function declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub params: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub imports: Vec<ImportClause>,
    pub ensures: Option<Expr>,
    pub body: Block,
    pub span: Span,
}

/// Function parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

/// Function-level import clause (declares effects)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportClause {
    pub names: Vec<String>,
    pub source: String,
    pub span: Span,
}

/// External binding declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternDecl {
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Type,
    pub source: String,
    pub effects: Vec<String>,
    pub span: Span,
}

/// Database declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseDecl {
    pub name: String,
    pub connection: Option<String>,
    pub tables: Vec<TableDecl>,
    pub span: Span,
}

/// Table declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDecl {
    pub name: String,
    pub columns: Vec<ColumnDecl>,
    pub constraints: Vec<TableConstraint>,
    pub span: Span,
}

/// Column declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDecl {
    pub name: String,
    pub ty: ColumnType,
    pub attrs: ColumnAttrs,
    pub span: Span,
}

/// Column type (database-specific)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnType {
    Int,
    String,
    Bool,
    Float,
    DateTime,
    Bytes,
    /// Foreign key reference
    Reference(String),
}

/// Column attributes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ColumnAttrs {
    pub primary: bool,
    pub unique: bool,
    pub nullable: bool,
    pub auto: bool,
}

/// Table constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TableConstraint {
    Unique(Vec<String>),
    Index(Vec<String>),
    Foreign {
        column: String,
        target: TypePath,
    },
}
