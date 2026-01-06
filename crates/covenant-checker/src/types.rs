//! Resolved type representations

use covenant_ast::SymbolId;

/// A resolved type (after type checking)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedType {
    /// Primitive types
    Int,
    Float,
    Bool,
    String,
    None,

    /// Named type with resolved ID
    Named {
        name: String,
        id: SymbolId,
        args: Vec<ResolvedType>,
    },

    /// Optional type
    Optional(Box<ResolvedType>),

    /// List type
    List(Box<ResolvedType>),

    /// Union type
    Union(Vec<ResolvedType>),

    /// Tuple type
    Tuple(Vec<ResolvedType>),

    /// Function type
    Function {
        params: Vec<ResolvedType>,
        ret: Box<ResolvedType>,
    },

    /// Struct type
    Struct(Vec<(String, ResolvedType)>),

    /// Unknown (for inference)
    Unknown,

    /// Error type (for error recovery)
    Error,
}

impl ResolvedType {
    pub fn is_error(&self) -> bool {
        matches!(self, ResolvedType::Error)
    }

    pub fn is_optional(&self) -> bool {
        matches!(self, ResolvedType::Optional(_))
    }

    pub fn display(&self) -> String {
        match self {
            ResolvedType::Int => "Int".to_string(),
            ResolvedType::Float => "Float".to_string(),
            ResolvedType::Bool => "Bool".to_string(),
            ResolvedType::String => "String".to_string(),
            ResolvedType::None => "none".to_string(),
            ResolvedType::Named { name, args, .. } => {
                if args.is_empty() {
                    name.clone()
                } else {
                    format!(
                        "{}<{}>",
                        name,
                        args.iter().map(|t| t.display()).collect::<Vec<_>>().join(", ")
                    )
                }
            }
            ResolvedType::Optional(inner) => format!("{}?", inner.display()),
            ResolvedType::List(inner) => format!("{}[]", inner.display()),
            ResolvedType::Union(types) => {
                types.iter().map(|t| t.display()).collect::<Vec<_>>().join(" | ")
            }
            ResolvedType::Tuple(types) => {
                format!(
                    "({})",
                    types.iter().map(|t| t.display()).collect::<Vec<_>>().join(", ")
                )
            }
            ResolvedType::Function { params, ret } => {
                format!(
                    "({}) -> {}",
                    params.iter().map(|t| t.display()).collect::<Vec<_>>().join(", "),
                    ret.display()
                )
            }
            ResolvedType::Struct(fields) => {
                format!(
                    "{{ {} }}",
                    fields
                        .iter()
                        .map(|(n, t)| format!("{}: {}", n, t.display()))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            ResolvedType::Unknown => "?".to_string(),
            ResolvedType::Error => "<error>".to_string(),
        }
    }
}
