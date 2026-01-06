//! Parser error types

use covenant_ast::Span;
use covenant_lexer::TokenKind;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected token: expected {expected}, found {found}")]
    UnexpectedToken {
        expected: String,
        found: String,
        span: Span,
    },

    #[error("unexpected end of file")]
    UnexpectedEof { span: Span },

    #[error("invalid expression")]
    InvalidExpression { span: Span },

    #[error("invalid pattern")]
    InvalidPattern { span: Span },

    #[error("invalid type")]
    InvalidType { span: Span },

    #[error("expected declaration")]
    ExpectedDeclaration { span: Span },
}

impl ParseError {
    pub fn span(&self) -> Span {
        match self {
            ParseError::UnexpectedToken { span, .. } => *span,
            ParseError::UnexpectedEof { span } => *span,
            ParseError::InvalidExpression { span } => *span,
            ParseError::InvalidPattern { span } => *span,
            ParseError::InvalidType { span } => *span,
            ParseError::ExpectedDeclaration { span } => *span,
        }
    }

    pub fn unexpected(expected: impl Into<String>, found: TokenKind, span: Span) -> Self {
        ParseError::UnexpectedToken {
            expected: expected.into(),
            found: found.describe().to_string(),
            span,
        }
    }
}
