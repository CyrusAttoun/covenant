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

    #[error("unexpected: expected {expected}, found {found:?}")]
    Unexpected {
        expected: String,
        found: TokenKind,
        span: Span,
    },

    #[error("invalid snippet kind: {kind}")]
    InvalidSnippetKind { kind: String, span: Span },

    #[error("invalid step kind: {kind}")]
    InvalidStepKind { kind: String, span: Span },

    #[error("invalid operation: {name}")]
    InvalidOperation { name: String, span: Span },

    #[error("unexpected section: {section}")]
    UnexpectedSection { section: String, span: Span },
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
            ParseError::Unexpected { span, .. } => *span,
            ParseError::InvalidSnippetKind { span, .. } => *span,
            ParseError::InvalidStepKind { span, .. } => *span,
            ParseError::InvalidOperation { span, .. } => *span,
            ParseError::UnexpectedSection { span, .. } => *span,
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
