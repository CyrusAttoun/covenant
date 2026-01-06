//! Covenant Parser - Recursive descent parser
//!
//! Parses Covenant source code into an AST.
//! Key parsing challenges:
//! - No `fn` keyword: functions are identified by signature shape
//! - `=` is equality, `:=` is assignment
//! - Query expressions with SQL-like syntax

mod error;
mod parser;

pub use error::*;
pub use parser::*;

use covenant_ast::Program;
use covenant_lexer::tokenize;

/// Parse a source string into a Program AST
pub fn parse(source: &str) -> Result<Program, ParseError> {
    let tokens = tokenize(source);
    let mut parser = Parser::new(source, tokens);
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hello_world() {
        let source = r#"
            main()
                import { println } from console
            {
                println("Hello, world!")
            }
        "#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_struct() {
        let source = r#"
            struct User {
                id: Int,
                name: String,
            }
        "#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_pure_function() {
        let source = r#"
            double(x: Int) -> Int {
                x * 2
            }
        "#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }
}
