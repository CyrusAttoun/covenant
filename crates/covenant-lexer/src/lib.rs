//! Covenant Lexer - Tokenization using logos
//!
//! Handles Covenant's unusual operators:
//! - `=` is equality (not `==`)
//! - `:=` is assignment
//! - `!=` is inequality

mod token;

pub use token::*;

use logos::Logos;
use covenant_ast::Span;

/// Tokenize a source string into a vector of tokens
pub fn tokenize(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut lexer = TokenKind::lexer(source);

    while let Some(result) = lexer.next() {
        let span = Span::new(lexer.span().start, lexer.span().end);
        let kind = match result {
            Ok(kind) => kind,
            Err(_) => TokenKind::Error,
        };
        tokens.push(Token { kind, span });
    }

    // Add EOF token
    let end = source.len();
    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span::new(end, end),
    });

    tokens
}

/// A token with its span
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.span.start..self.span.end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let tokens = tokenize("let x = 5");
        assert_eq!(tokens[0].kind, TokenKind::Let);
        assert_eq!(tokens[1].kind, TokenKind::Ident);
        assert_eq!(tokens[2].kind, TokenKind::Eq);
        assert_eq!(tokens[3].kind, TokenKind::Int);
    }

    #[test]
    fn test_assignment_vs_equality() {
        let tokens = tokenize("x := 5");
        assert_eq!(tokens[1].kind, TokenKind::ColonEq);

        let tokens = tokenize("x = 5");
        assert_eq!(tokens[1].kind, TokenKind::Eq);
    }

    #[test]
    fn test_function_no_fn_keyword() {
        let tokens = tokenize("main() { }");
        assert_eq!(tokens[0].kind, TokenKind::Ident);
        assert_eq!(tokens[1].kind, TokenKind::LParen);
        assert_eq!(tokens[2].kind, TokenKind::RParen);
        assert_eq!(tokens[3].kind, TokenKind::LBrace);
    }
}
