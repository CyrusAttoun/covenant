//! Type representations in Covenant

use serde::{Deserialize, Serialize};
use crate::Span;

/// A type expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeKind {
    /// Named type: `User`, `Int`, `String`
    Named(TypePath),

    /// Optional type: `User?`
    Optional(Box<Type>),

    /// List type: `User[]`
    List(Box<Type>),

    /// Union type: `User | DbError | NetworkError`
    Union(Vec<Type>),

    /// Tuple type: `(Int, String)`
    Tuple(Vec<Type>),

    /// Function type: `(Int, Int) -> Int`
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
    },

    /// Anonymous struct type: `{ name: String, age: Int }`
    Struct(Vec<FieldType>),
}

/// A path to a type: `User` or `module::User`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypePath {
    pub segments: Vec<String>,
    pub generics: Vec<Type>,
    pub span: Span,
}

impl TypePath {
    pub fn simple(name: impl Into<String>, span: Span) -> Self {
        Self {
            segments: vec![name.into()],
            generics: vec![],
            span,
        }
    }

    pub fn name(&self) -> &str {
        self.segments.last().map(|s| s.as_str()).unwrap_or("")
    }
}

/// A field in a struct type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldType {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}
