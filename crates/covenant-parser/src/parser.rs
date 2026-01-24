//! Recursive descent parser implementation

use covenant_ast::*;
use covenant_lexer::{Token, TokenKind};

use crate::ParseError;

pub struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str, tokens: Vec<Token>) -> Self {
        Self {
            source,
            tokens,
            pos: 0,
        }
    }

    // === Utilities ===

    fn current(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or_else(|| {
            self.tokens.last().expect("tokens should have at least EOF")
        })
    }

    fn peek(&self) -> TokenKind {
        self.current().kind
    }

    fn peek_ahead(&self, n: usize) -> TokenKind {
        self.tokens
            .get(self.pos + n)
            .map(|t| t.kind)
            .unwrap_or(TokenKind::Eof)
    }

    fn advance(&mut self) -> &Token {
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        // Return the token we just passed
        &self.tokens[self.pos - 1]
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.peek() == kind
    }

    fn at_any(&self, kinds: &[TokenKind]) -> bool {
        kinds.contains(&self.peek())
    }

    /// Check if current token can be used as an identifier (including contextual keywords)
    fn at_ident_like(&self) -> bool {
        matches!(self.peek(), TokenKind::Ident | TokenKind::Id | TokenKind::Type)
    }

    /// Consume a token that can be used as an identifier (including contextual keywords)
    fn consume_ident_like(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            TokenKind::Ident => self.consume_text(TokenKind::Ident),
            TokenKind::Id => { self.advance(); Ok("id".to_string()) }
            TokenKind::Type => { self.advance(); Ok("type".to_string()) }
            _ => Err(ParseError::unexpected(
                "identifier",
                self.peek(),
                self.span(),
            ))
        }
    }

    fn consume(&mut self, kind: TokenKind) -> Result<&Token, ParseError> {
        if self.at(kind) {
            Ok(self.advance())
        } else {
            Err(ParseError::unexpected(
                kind.describe(),
                self.peek(),
                self.current().span,
            ))
        }
    }

    fn span(&self) -> Span {
        self.current().span
    }

    // Helper to consume a token and extract its text in one step
    fn consume_text(&mut self, kind: TokenKind) -> Result<String, ParseError> {
        let span = self.consume(kind)?.span;
        Ok(self.source[span.start..span.end].to_string())
    }

    // Helper to consume a relation type (can be identifier or keyword like 'contains')
    fn consume_relation_type(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            TokenKind::Ident => Ok(self.advance_text()),
            TokenKind::Contains => { self.advance(); Ok("contains".to_string()) }
            _ => {
                // Try to get the text of any token
                let text = self.advance_text();
                Ok(text)
            }
        }
    }

    // Helper to advance and extract text in one step
    fn advance_text(&mut self) -> String {
        let span = self.advance().span;
        self.source[span.start..span.end].to_string()
    }

    // Helper to consume and parse string literal
    fn consume_string_literal(&mut self) -> Result<String, ParseError> {
        if self.at(TokenKind::TripleString) {
            let span = self.advance().span;
            let text = &self.source[span.start..span.end];
            return Ok(self.parse_triple_string_from_text(text));
        }
        let span = self.consume(TokenKind::String)?.span;
        let text = &self.source[span.start..span.end];
        Ok(self.parse_string_literal_from_text(text))
    }

    // Helper to advance and parse string literal
    fn advance_string_literal(&mut self) -> String {
        if self.at(TokenKind::TripleString) {
            let span = self.advance().span;
            let text = &self.source[span.start..span.end];
            return self.parse_triple_string_from_text(text);
        }
        let span = self.advance().span;
        let text = &self.source[span.start..span.end];
        self.parse_string_literal_from_text(text)
    }

    // Helper to parse triple-quoted string literal from text
    fn parse_triple_string_from_text(&self, text: &str) -> String {
        // Remove surrounding """ and return content
        if text.len() >= 6 {
            text[3..text.len() - 3].to_string()
        } else {
            text.to_string()
        }
    }

    // Helper to parse string literal from text
    fn parse_string_literal_from_text(&self, text: &str) -> String {
        // Remove surrounding quotes and unescape
        let inner = &text[1..text.len() - 1];
        inner
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    }

    // === Program ===

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let start = self.span();

        // Detect mode by first token
        if self.at(TokenKind::Snippet) {
            // Snippet mode - IR-based parsing
            self.parse_snippet_program(start)
        } else {
            // Legacy mode - traditional parsing
            self.parse_legacy_program(start)
        }
    }

    fn parse_legacy_program(&mut self, start: Span) -> Result<Program, ParseError> {
        let mut declarations = Vec::new();

        while !self.at(TokenKind::Eof) {
            declarations.push(self.parse_declaration()?);
        }

        let end = self.span();
        Ok(Program::Legacy {
            declarations,
            span: start.merge(end),
        })
    }

    fn parse_snippet_program(&mut self, start: Span) -> Result<Program, ParseError> {
        let mut snippets = Vec::new();

        while !self.at(TokenKind::Eof) {
            snippets.push(self.parse_snippet()?);
        }

        let end = self.span();
        Ok(Program::Snippets {
            snippets,
            span: start.merge(end),
        })
    }

    // === Declarations ===

    fn parse_declaration(&mut self) -> Result<Declaration, ParseError> {
        let start = self.span();

        let kind = match self.peek() {
            TokenKind::Import => DeclarationKind::Import(self.parse_import_decl()?),
            TokenKind::Module => DeclarationKind::Module(self.parse_module_decl()?),
            TokenKind::Struct => DeclarationKind::Struct(self.parse_struct_decl()?),
            TokenKind::Enum => DeclarationKind::Enum(self.parse_enum_decl()?),
            TokenKind::Type => DeclarationKind::TypeAlias(self.parse_type_alias()?),
            TokenKind::Extern => DeclarationKind::Extern(self.parse_extern_decl()?),
            TokenKind::Database => DeclarationKind::Database(self.parse_database_decl()?),
            TokenKind::Ident => {
                // Could be a function: name(...) -> Type { }
                DeclarationKind::Function(self.parse_function_decl()?)
            }
            _ => {
                return Err(ParseError::ExpectedDeclaration { span: start });
            }
        };

        let end = self.span();
        Ok(Declaration {
            kind,
            span: start.merge(end),
        })
    }

    fn parse_import_decl(&mut self) -> Result<ImportDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Import)?;
        self.consume(TokenKind::LBrace)?;
        let names = self.parse_ident_list()?;
        self.consume(TokenKind::RBrace)?;
        self.consume(TokenKind::From)?;
        let source = self.consume_text(TokenKind::Ident)?;
        let end = self.span();

        Ok(ImportDecl {
            names,
            source,
            span: start.merge(end),
        })
    }

    fn parse_module_decl(&mut self) -> Result<ModuleDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Module)?;
        let name = self.consume_text(TokenKind::Ident)?;
        self.consume(TokenKind::LBrace)?;

        let mut declarations = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            declarations.push(self.parse_declaration()?);
        }

        self.consume(TokenKind::RBrace)?;
        let end = self.span();

        Ok(ModuleDecl {
            name,
            declarations,
            span: start.merge(end),
        })
    }

    fn parse_struct_decl(&mut self) -> Result<StructDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Struct)?;
        let name = self.consume_text(TokenKind::Ident)?;

        let generics = if self.at(TokenKind::Lt) {
            self.parse_generic_params()?
        } else {
            vec![]
        };

        self.consume(TokenKind::LBrace)?;
        let fields = self.parse_field_decls()?;
        self.consume(TokenKind::RBrace)?;
        let end = self.span();

        Ok(StructDecl {
            name,
            generics,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_field_decls(&mut self) -> Result<Vec<FieldDecl>, ParseError> {
        let mut fields = Vec::new();
        while self.at_ident_like() {
            let start = self.span();
            let name = self.consume_ident_like()?;
            self.consume(TokenKind::Colon)?;
            let ty = self.parse_type()?;

            let default = if self.at(TokenKind::Eq) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };

            // Optional comma
            if self.at(TokenKind::Comma) {
                self.advance();
            }

            let end = self.span();
            fields.push(FieldDecl {
                name,
                ty,
                default,
                span: start.merge(end),
            });
        }
        Ok(fields)
    }

    fn parse_enum_decl(&mut self) -> Result<EnumDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Enum)?;
        let name = self.consume_text(TokenKind::Ident)?;

        let generics = if self.at(TokenKind::Lt) {
            self.parse_generic_params()?
        } else {
            vec![]
        };

        self.consume(TokenKind::LBrace)?;
        let variants = self.parse_variants()?;
        self.consume(TokenKind::RBrace)?;
        let end = self.span();

        Ok(EnumDecl {
            name,
            generics,
            variants,
            span: start.merge(end),
        })
    }

    fn parse_variants(&mut self) -> Result<Vec<VariantDecl>, ParseError> {
        let mut variants = Vec::new();
        while self.at(TokenKind::Ident) {
            let start = self.span();
            let name = self.consume_text(TokenKind::Ident)?;

            let fields = if self.at(TokenKind::LParen) {
                self.advance();
                let types = self.parse_type_list()?;
                self.consume(TokenKind::RParen)?;
                VariantFields::Tuple(types)
            } else if self.at(TokenKind::LBrace) {
                self.advance();
                let fields = self.parse_field_decls()?;
                self.consume(TokenKind::RBrace)?;
                VariantFields::Struct(fields)
            } else {
                VariantFields::Unit
            };

            // Optional comma
            if self.at(TokenKind::Comma) {
                self.advance();
            }

            let end = self.span();
            variants.push(VariantDecl {
                name,
                fields,
                span: start.merge(end),
            });
        }
        Ok(variants)
    }

    fn parse_type_alias(&mut self) -> Result<TypeAliasDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Type)?;
        let name = self.consume_text(TokenKind::Ident)?;

        let generics = if self.at(TokenKind::Lt) {
            self.parse_generic_params()?
        } else {
            vec![]
        };

        self.consume(TokenKind::Eq)?;
        let ty = self.parse_type()?;
        let end = self.span();

        Ok(TypeAliasDecl {
            name,
            generics,
            ty,
            span: start.merge(end),
        })
    }

    fn parse_extern_decl(&mut self) -> Result<ExternDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Extern)?;
        let name = self.consume_text(TokenKind::Ident)?;

        self.consume(TokenKind::LParen)?;
        let params = if self.at(TokenKind::RParen) {
            vec![]
        } else {
            self.parse_params()?
        };
        self.consume(TokenKind::RParen)?;

        self.consume(TokenKind::Arrow)?;
        let return_type = self.parse_type()?;

        self.consume(TokenKind::From)?;
        let source = self.consume_string_literal()?;

        self.consume(TokenKind::Effect)?;
        self.consume(TokenKind::LBracket)?;
        let effects = self.parse_ident_list()?;
        self.consume(TokenKind::RBracket)?;
        let end = self.span();

        Ok(ExternDecl {
            name,
            params,
            return_type,
            source,
            effects,
            span: start.merge(end),
        })
    }

    fn parse_database_decl(&mut self) -> Result<DatabaseDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Database)?;
        let name = self.consume_text(TokenKind::Ident)?;

        let connection = if self.at(TokenKind::Connection) {
            self.advance();
            self.consume(TokenKind::Colon)?;
            Some(self.consume_string_literal()?)
        } else {
            None
        };

        self.consume(TokenKind::LBrace)?;
        let mut tables = Vec::new();
        while self.at(TokenKind::Table) {
            tables.push(self.parse_table_decl()?);
        }
        self.consume(TokenKind::RBrace)?;
        let end = self.span();

        Ok(DatabaseDecl {
            name,
            connection,
            tables,
            span: start.merge(end),
        })
    }

    fn parse_table_decl(&mut self) -> Result<TableDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Table)?;
        let name = self.consume_text(TokenKind::Ident)?;

        self.consume(TokenKind::LBrace)?;
        let mut columns = Vec::new();
        let mut constraints = Vec::new();

        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            match self.peek() {
                TokenKind::Unique | TokenKind::Index | TokenKind::Foreign => {
                    constraints.push(self.parse_table_constraint()?);
                }
                TokenKind::Ident => {
                    columns.push(self.parse_column_decl()?);
                }
                _ => break,
            }
        }

        self.consume(TokenKind::RBrace)?;
        let end = self.span();

        Ok(TableDecl {
            name,
            columns,
            constraints,
            span: start.merge(end),
        })
    }

    fn parse_column_decl(&mut self) -> Result<ColumnDecl, ParseError> {
        let start = self.span();
        let name = self.consume_text(TokenKind::Ident)?;
        self.consume(TokenKind::Colon)?;

        let ty = self.parse_column_type()?;
        let attrs = self.parse_column_attrs()?;

        let end = self.span();
        Ok(ColumnDecl {
            name,
            ty,
            attrs,
            span: start.merge(end),
        })
    }

    fn parse_column_type(&mut self) -> Result<ColumnType, ParseError> {
        let text = self.consume_text(TokenKind::Ident)?;
        Ok(match text.as_str() {
            "Int" => ColumnType::Int,
            "String" => ColumnType::String,
            "Bool" => ColumnType::Bool,
            "Float" => ColumnType::Float,
            "DateTime" => ColumnType::DateTime,
            "Bytes" => ColumnType::Bytes,
            _ => ColumnType::Reference(text),
        })
    }

    fn parse_column_attrs(&mut self) -> Result<ColumnAttrs, ParseError> {
        let mut attrs = ColumnAttrs::default();
        loop {
            match self.peek() {
                TokenKind::Primary => {
                    self.advance();
                    attrs.primary = true;
                }
                TokenKind::Unique => {
                    self.advance();
                    attrs.unique = true;
                }
                TokenKind::Nullable => {
                    self.advance();
                    attrs.nullable = true;
                }
                TokenKind::Auto => {
                    self.advance();
                    attrs.auto = true;
                }
                _ => break,
            }
        }
        Ok(attrs)
    }

    fn parse_table_constraint(&mut self) -> Result<TableConstraint, ParseError> {
        match self.peek() {
            TokenKind::Unique => {
                self.advance();
                self.consume(TokenKind::LParen)?;
                let columns = self.parse_ident_list()?;
                self.consume(TokenKind::RParen)?;
                Ok(TableConstraint::Unique(columns))
            }
            TokenKind::Index => {
                self.advance();
                self.consume(TokenKind::LParen)?;
                let columns = self.parse_ident_list()?;
                self.consume(TokenKind::RParen)?;
                Ok(TableConstraint::Index(columns))
            }
            TokenKind::Foreign => {
                self.advance();
                let column = self.consume_text(TokenKind::Ident)?;
                self.consume(TokenKind::Arrow)?;
                let target = self.parse_type_path()?;
                Ok(TableConstraint::Foreign { column, target })
            }
            _ => Err(ParseError::unexpected(
                "constraint",
                self.peek(),
                self.span(),
            )),
        }
    }

    fn parse_function_decl(&mut self) -> Result<FunctionDecl, ParseError> {
        let start = self.span();
        let name = self.consume_text(TokenKind::Ident)?;

        let generics = if self.at(TokenKind::Lt) {
            self.parse_generic_params()?
        } else {
            vec![]
        };

        self.consume(TokenKind::LParen)?;
        let params = if self.at(TokenKind::RParen) {
            vec![]
        } else {
            self.parse_params()?
        };
        self.consume(TokenKind::RParen)?;

        let return_type = if self.at(TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        let mut imports = Vec::new();
        while self.at(TokenKind::Import) {
            imports.push(self.parse_import_clause()?);
        }

        let ensures = if self.at(TokenKind::Ensures) {
            self.advance();
            self.consume(TokenKind::Colon)?;
            Some(self.parse_expr()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        let end = self.span();

        Ok(FunctionDecl {
            name,
            generics,
            params,
            return_type,
            imports,
            ensures,
            body,
            span: start.merge(end),
        })
    }

    fn parse_import_clause(&mut self) -> Result<ImportClause, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Import)?;
        self.consume(TokenKind::LBrace)?;
        let names = self.parse_ident_list()?;
        self.consume(TokenKind::RBrace)?;
        self.consume(TokenKind::From)?;
        let source = self.consume_text(TokenKind::Ident)?;
        let end = self.span();

        Ok(ImportClause {
            names,
            source,
            span: start.merge(end),
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Parameter>, ParseError> {
        let mut params = Vec::new();
        loop {
            let start = self.span();
            let name = self.consume_text(TokenKind::Ident)?;
            self.consume(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            let end = self.span();

            params.push(Parameter {
                name,
                ty,
                span: start.merge(end),
            });

            if !self.at(TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        Ok(params)
    }

    fn parse_generic_params(&mut self) -> Result<Vec<String>, ParseError> {
        self.consume(TokenKind::Lt)?;
        let params = self.parse_ident_list()?;
        self.consume(TokenKind::Gt)?;
        Ok(params)
    }

    fn parse_ident_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut idents = Vec::new();
        loop {
            idents.push(self.consume_text(TokenKind::Ident)?);
            if !self.at(TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        Ok(idents)
    }

    // === Types ===

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        self.parse_union_type()
    }

    fn parse_union_type(&mut self) -> Result<Type, ParseError> {
        let start = self.span();
        let first = self.parse_base_type()?;

        if !self.at(TokenKind::Pipe) {
            return Ok(first);
        }

        let mut types = vec![first];
        while self.at(TokenKind::Pipe) {
            self.advance();
            types.push(self.parse_base_type()?);
        }

        let end = self.span();
        Ok(Type {
            kind: TypeKind::Union(types),
            span: start.merge(end),
        })
    }

    fn parse_base_type(&mut self) -> Result<Type, ParseError> {
        let start = self.span();
        let mut ty = self.parse_primary_type()?;

        // Handle postfix type operators: ? and []
        loop {
            if self.at(TokenKind::Question) {
                self.advance();
                let end = self.span();
                ty = Type {
                    kind: TypeKind::Optional(Box::new(ty)),
                    span: start.merge(end),
                };
            } else if self.at(TokenKind::LBracket) && self.peek_ahead(1) == TokenKind::RBracket {
                self.advance();
                self.advance();
                let end = self.span();
                ty = Type {
                    kind: TypeKind::List(Box::new(ty)),
                    span: start.merge(end),
                };
            } else {
                break;
            }
        }

        Ok(ty)
    }

    fn parse_primary_type(&mut self) -> Result<Type, ParseError> {
        let start = self.span();

        if self.at(TokenKind::LParen) {
            self.advance();
            if self.at(TokenKind::RParen) {
                // Unit tuple
                self.advance();
                let end = self.span();
                return Ok(Type {
                    kind: TypeKind::Tuple(vec![]),
                    span: start.merge(end),
                });
            }

            let first = self.parse_type()?;
            if self.at(TokenKind::Comma) {
                // Tuple type
                let mut types = vec![first];
                while self.at(TokenKind::Comma) {
                    self.advance();
                    if self.at(TokenKind::RParen) {
                        break;
                    }
                    types.push(self.parse_type()?);
                }
                self.consume(TokenKind::RParen)?;

                if self.at(TokenKind::Arrow) {
                    // Function type
                    self.advance();
                    let ret = self.parse_type()?;
                    let end = self.span();
                    return Ok(Type {
                        kind: TypeKind::Function {
                            params: types,
                            ret: Box::new(ret),
                        },
                        span: start.merge(end),
                    });
                }

                let end = self.span();
                return Ok(Type {
                    kind: TypeKind::Tuple(types),
                    span: start.merge(end),
                });
            }

            self.consume(TokenKind::RParen)?;

            if self.at(TokenKind::Arrow) {
                // Function type with single param
                self.advance();
                let ret = self.parse_type()?;
                let end = self.span();
                return Ok(Type {
                    kind: TypeKind::Function {
                        params: vec![first],
                        ret: Box::new(ret),
                    },
                    span: start.merge(end),
                });
            }

            // Just parenthesized type
            return Ok(first);
        }

        if self.at(TokenKind::LBrace) {
            // Anonymous struct type
            self.advance();
            let fields = self.parse_field_types()?;
            self.consume(TokenKind::RBrace)?;
            let end = self.span();
            return Ok(Type {
                kind: TypeKind::Struct(fields),
                span: start.merge(end),
            });
        }

        // Named type
        let path = self.parse_type_path()?;
        let end = self.span();
        Ok(Type {
            kind: TypeKind::Named(path),
            span: start.merge(end),
        })
    }

    fn parse_type_path(&mut self) -> Result<TypePath, ParseError> {
        let start = self.span();
        let mut segments = Vec::new();

        segments.push(self.consume_text(TokenKind::Ident)?);

        while self.at(TokenKind::ColonColon) {
            self.advance();
            segments.push(self.consume_text(TokenKind::Ident)?);
        }

        let generics = if self.at(TokenKind::Lt) {
            self.advance();
            let types = self.parse_type_list()?;
            self.consume(TokenKind::Gt)?;
            types
        } else {
            vec![]
        };

        let end = self.span();
        Ok(TypePath {
            segments,
            generics,
            span: start.merge(end),
        })
    }

    fn parse_type_list(&mut self) -> Result<Vec<Type>, ParseError> {
        let mut types = Vec::new();
        if self.at(TokenKind::Gt) || self.at(TokenKind::RParen) {
            return Ok(types);
        }
        loop {
            types.push(self.parse_type()?);
            if !self.at(TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        Ok(types)
    }

    fn parse_field_types(&mut self) -> Result<Vec<FieldType>, ParseError> {
        let mut fields = Vec::new();
        while self.at(TokenKind::Ident) {
            let start = self.span();
            let name = self.consume_text(TokenKind::Ident)?;
            self.consume(TokenKind::Colon)?;
            let ty = self.parse_type()?;

            if self.at(TokenKind::Comma) {
                self.advance();
            }

            let end = self.span();
            fields.push(FieldType {
                name,
                ty,
                span: start.merge(end),
            });
        }
        Ok(fields)
    }

    // === Statements ===

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let start = self.span();
        self.consume(TokenKind::LBrace)?;

        let mut statements = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            statements.push(self.parse_statement()?);
        }

        self.consume(TokenKind::RBrace)?;
        let end = self.span();

        Ok(Block {
            statements,
            span: start.merge(end),
        })
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        let start = self.span();

        let kind = match self.peek() {
            TokenKind::Let => self.parse_let_stmt()?,
            TokenKind::Return => self.parse_return_stmt()?,
            TokenKind::For => self.parse_for_stmt()?,
            _ => StatementKind::Expr(self.parse_expr()?),
        };

        let end = self.span();
        Ok(Statement {
            kind,
            span: start.merge(end),
        })
    }

    fn parse_let_stmt(&mut self) -> Result<StatementKind, ParseError> {
        self.consume(TokenKind::Let)?;

        let mutable = if self.at(TokenKind::Mut) {
            self.advance();
            true
        } else {
            false
        };

        let name = self.consume_text(TokenKind::Ident)?;

        let ty = if self.at(TokenKind::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(TokenKind::Eq)?;
        let value = self.parse_expr()?;

        Ok(StatementKind::Let {
            name,
            mutable,
            ty,
            value,
        })
    }

    fn parse_return_stmt(&mut self) -> Result<StatementKind, ParseError> {
        self.consume(TokenKind::Return)?;

        let value = if self.at(TokenKind::RBrace) || self.at(TokenKind::Eof) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        Ok(StatementKind::Return(value))
    }

    fn parse_for_stmt(&mut self) -> Result<StatementKind, ParseError> {
        self.consume(TokenKind::For)?;
        let binding = self.consume_text(TokenKind::Ident)?;
        self.consume(TokenKind::In)?;
        let iterable = self.parse_expr()?;
        let body = self.parse_block()?;

        Ok(StatementKind::For {
            binding,
            iterable,
            body,
        })
    }

    // === Expressions ===

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();

        // Check for assignment: ident := expr
        if self.at(TokenKind::Ident) && self.peek_ahead(1) == TokenKind::ColonEq {
            let target = self.consume_text(TokenKind::Ident)?;
            self.consume(TokenKind::ColonEq)?;
            let value = self.parse_expr()?;
            let end = self.span();
            return Ok(Expr {
                kind: ExprKind::Assign {
                    target,
                    value: Box::new(value),
                },
                span: start.merge(end),
            });
        }

        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        let mut left = self.parse_and()?;

        while self.at(TokenKind::OrOr) {
            self.advance();
            let right = self.parse_and()?;
            let end = self.span();
            left = Expr {
                kind: ExprKind::Binary {
                    op: BinaryOp::Or,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: start.merge(end),
            };
        }

        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        let mut left = self.parse_equality()?;

        while self.at(TokenKind::AndAnd) {
            self.advance();
            let right = self.parse_equality()?;
            let end = self.span();
            left = Expr {
                kind: ExprKind::Binary {
                    op: BinaryOp::And,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: start.merge(end),
            };
        }

        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        let mut left = self.parse_comparison()?;

        loop {
            let op = match self.peek() {
                TokenKind::Eq => BinaryOp::Eq,
                TokenKind::Ne => BinaryOp::Ne,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            let end = self.span();
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: start.merge(end),
            };
        }

        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        let mut left = self.parse_term()?;

        loop {
            let op = match self.peek() {
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::Le => BinaryOp::Le,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::Ge => BinaryOp::Ge,
                TokenKind::Contains => BinaryOp::Contains,
                _ => break,
            };
            self.advance();
            let right = self.parse_term()?;
            let end = self.span();
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: start.merge(end),
            };
        }

        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        let mut left = self.parse_factor()?;

        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_factor()?;
            let end = self.span();
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: start.merge(end),
            };
        }

        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        let mut left = self.parse_unary()?;

        loop {
            let op = match self.peek() {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            let end = self.span();
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: start.merge(end),
            };
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();

        let op = match self.peek() {
            TokenKind::Bang => Some(UnaryOp::Not),
            TokenKind::Minus => Some(UnaryOp::Neg),
            _ => None,
        };

        if let Some(op) = op {
            self.advance();
            let operand = self.parse_unary()?;
            let end = self.span();
            return Ok(Expr {
                kind: ExprKind::Unary {
                    op,
                    operand: Box::new(operand),
                },
                span: start.merge(end),
            });
        }

        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        let mut expr = self.parse_primary()?;

        loop {
            if self.at(TokenKind::LParen) {
                // Function call
                self.advance();
                let args = if self.at(TokenKind::RParen) {
                    vec![]
                } else {
                    self.parse_expr_list()?
                };
                self.consume(TokenKind::RParen)?;
                let end = self.span();
                expr = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    },
                    span: start.merge(end),
                };
            } else if self.at(TokenKind::LBracket) {
                // Index
                self.advance();
                let index = self.parse_expr()?;
                self.consume(TokenKind::RBracket)?;
                let end = self.span();
                expr = Expr {
                    kind: ExprKind::Index {
                        object: Box::new(expr),
                        index: Box::new(index),
                    },
                    span: start.merge(end),
                };
            } else if self.at(TokenKind::Dot) {
                // Field access
                self.advance();
                let field = self.consume_text(TokenKind::Ident)?;
                let end = self.span();
                expr = Expr {
                    kind: ExprKind::Field {
                        object: Box::new(expr),
                        field,
                    },
                    span: start.merge(end),
                };
            } else if self.at(TokenKind::Handle) {
                // Handle expression
                self.advance();
                self.consume(TokenKind::LBrace)?;
                let arms = self.parse_match_arms()?;
                self.consume(TokenKind::RBrace)?;
                let end = self.span();
                expr = Expr {
                    kind: ExprKind::Handle {
                        expr: Box::new(expr),
                        arms,
                    },
                    span: start.merge(end),
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();

        match self.peek() {
            TokenKind::Int => {
                let text = self.advance_text();
                let value: i64 = text.parse().unwrap_or(0);
                Ok(Expr {
                    kind: ExprKind::Literal(Literal::Int(value)),
                    span: start,
                })
            }
            TokenKind::Float => {
                let text = self.advance_text();
                let value: f64 = text.parse().unwrap_or(0.0);
                Ok(Expr {
                    kind: ExprKind::Literal(Literal::Float(value)),
                    span: start,
                })
            }
            TokenKind::String => {
                let value = self.advance_string_literal();
                Ok(Expr {
                    kind: ExprKind::Literal(Literal::String(value)),
                    span: start,
                })
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Literal(Literal::Bool(true)),
                    span: start,
                })
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Literal(Literal::Bool(false)),
                    span: start,
                })
            }
            TokenKind::None => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Literal(Literal::None),
                    span: start,
                })
            }
            TokenKind::Ident => {
                // Could be: identifier, struct literal, or path
                let path = self.parse_type_path()?;

                if self.at(TokenKind::LBrace) && !self.is_block_start() {
                    // Struct literal
                    self.advance();
                    let fields = self.parse_field_inits()?;
                    self.consume(TokenKind::RBrace)?;
                    let end = self.span();
                    Ok(Expr {
                        kind: ExprKind::Struct {
                            path: Some(path),
                            fields,
                        },
                        span: start.merge(end),
                    })
                } else if path.segments.len() == 1 && path.generics.is_empty() {
                    // Simple identifier
                    Ok(Expr {
                        kind: ExprKind::Ident(path.segments.into_iter().next().unwrap()),
                        span: start,
                    })
                } else {
                    // Path expression (for now treat as ident)
                    Ok(Expr {
                        kind: ExprKind::Ident(path.segments.join("::")),
                        span: start,
                    })
                }
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.consume(TokenKind::RParen)?;
                Ok(expr)
            }
            TokenKind::LBrace => {
                // Block expression or anonymous struct
                if self.peek_ahead(1) == TokenKind::Ident && self.peek_ahead(2) == TokenKind::Colon {
                    // Anonymous struct
                    self.advance();
                    let fields = self.parse_field_inits()?;
                    self.consume(TokenKind::RBrace)?;
                    let end = self.span();
                    Ok(Expr {
                        kind: ExprKind::Struct { path: None, fields },
                        span: start.merge(end),
                    })
                } else {
                    // Block expression
                    let block = self.parse_block()?;
                    Ok(Expr {
                        kind: ExprKind::Block(block),
                        span: start,
                    })
                }
            }
            TokenKind::LBracket => {
                // Array literal
                self.advance();
                let elements = if self.at(TokenKind::RBracket) {
                    vec![]
                } else {
                    self.parse_expr_list()?
                };
                self.consume(TokenKind::RBracket)?;
                let end = self.span();
                Ok(Expr {
                    kind: ExprKind::Array(elements),
                    span: start.merge(end),
                })
            }
            TokenKind::Pipe => {
                // Closure
                self.advance();
                let params = if self.at(TokenKind::Pipe) {
                    vec![]
                } else {
                    self.parse_closure_params()?
                };
                self.consume(TokenKind::Pipe)?;
                let body = self.parse_expr()?;
                let end = self.span();
                Ok(Expr {
                    kind: ExprKind::Closure {
                        params,
                        body: Box::new(body),
                    },
                    span: start.merge(end),
                })
            }
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::Query => self.parse_query_expr(),
            TokenKind::Insert => self.parse_insert_expr(),
            TokenKind::Update => self.parse_update_expr(),
            TokenKind::Delete => self.parse_delete_expr(),
            _ => Err(ParseError::InvalidExpression { span: start }),
        }
    }

    fn is_block_start(&self) -> bool {
        // Heuristic: if we see { followed by let/return/if/for/}, it's a block
        if !self.at(TokenKind::LBrace) {
            return false;
        }
        matches!(
            self.peek_ahead(1),
            TokenKind::Let
                | TokenKind::Return
                | TokenKind::If
                | TokenKind::For
                | TokenKind::Match
                | TokenKind::RBrace
        )
    }

    fn parse_field_inits(&mut self) -> Result<Vec<FieldInit>, ParseError> {
        let mut fields = Vec::new();
        while self.at(TokenKind::Ident) {
            let start = self.span();
            let name = self.consume_text(TokenKind::Ident)?;

            let value = if self.at(TokenKind::Colon) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };

            if self.at(TokenKind::Comma) {
                self.advance();
            }

            let end = self.span();
            fields.push(FieldInit {
                name,
                value,
                span: start.merge(end),
            });
        }
        Ok(fields)
    }

    fn parse_closure_params(&mut self) -> Result<Vec<ClosureParam>, ParseError> {
        let mut params = Vec::new();
        loop {
            let start = self.span();
            let name = self.consume_text(TokenKind::Ident)?;

            let ty = if self.at(TokenKind::Colon) {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };

            let end = self.span();
            params.push(ClosureParam {
                name,
                ty,
                span: start.merge(end),
            });

            if !self.at(TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        Ok(params)
    }

    fn parse_expr_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut exprs = Vec::new();
        loop {
            exprs.push(self.parse_expr()?);
            if !self.at(TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        Ok(exprs)
    }

    fn parse_if_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        self.consume(TokenKind::If)?;
        let condition = self.parse_expr()?;
        let then_branch = self.parse_block()?;

        let else_branch = if self.at(TokenKind::Else) {
            self.advance();
            if self.at(TokenKind::If) {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                let block = self.parse_block()?;
                Some(Box::new(Expr {
                    kind: ExprKind::Block(block.clone()),
                    span: block.span,
                }))
            }
        } else {
            None
        };

        let end = self.span();
        Ok(Expr {
            kind: ExprKind::If {
                condition: Box::new(condition),
                then_branch,
                else_branch,
            },
            span: start.merge(end),
        })
    }

    fn parse_match_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Match)?;
        let scrutinee = self.parse_expr()?;
        self.consume(TokenKind::LBrace)?;
        let arms = self.parse_match_arms()?;
        self.consume(TokenKind::RBrace)?;
        let end = self.span();

        Ok(Expr {
            kind: ExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span: start.merge(end),
        })
    }

    fn parse_match_arms(&mut self) -> Result<Vec<MatchArm>, ParseError> {
        let mut arms = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let start = self.span();
            let pattern = self.parse_pattern()?;
            self.consume(TokenKind::FatArrow)?;
            let body = self.parse_expr()?;

            // Optional comma
            if self.at(TokenKind::Comma) {
                self.advance();
            }

            let end = self.span();
            arms.push(MatchArm {
                pattern,
                body,
                span: start.merge(end),
            });
        }
        Ok(arms)
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        let start = self.span();

        match self.peek() {
            TokenKind::Ident => {
                let path = self.parse_type_path()?;

                if self.at(TokenKind::LParen) {
                    // Variant with tuple fields
                    self.advance();
                    let mut patterns = Vec::new();
                    if !self.at(TokenKind::RParen) {
                        loop {
                            patterns.push(self.parse_pattern()?);
                            if !self.at(TokenKind::Comma) {
                                break;
                            }
                            self.advance();
                        }
                    }
                    self.consume(TokenKind::RParen)?;
                    let end = self.span();
                    Ok(Pattern {
                        kind: PatternKind::Variant {
                            path,
                            fields: PatternFields::Positional(patterns),
                        },
                        span: start.merge(end),
                    })
                } else if self.at(TokenKind::LBrace) {
                    // Variant with struct fields
                    self.advance();
                    let mut fields = Vec::new();
                    while self.at(TokenKind::Ident) {
                        let name = self.consume_text(TokenKind::Ident)?;
                        let pattern = if self.at(TokenKind::Colon) {
                            self.advance();
                            self.parse_pattern()?
                        } else {
                            Pattern {
                                kind: PatternKind::Binding(name.clone()),
                                span: start,
                            }
                        };
                        fields.push((name, pattern));
                        if self.at(TokenKind::Comma) {
                            self.advance();
                        }
                    }
                    self.consume(TokenKind::RBrace)?;
                    let end = self.span();
                    Ok(Pattern {
                        kind: PatternKind::Variant {
                            path,
                            fields: PatternFields::Named(fields),
                        },
                        span: start.merge(end),
                    })
                } else if path.segments.len() == 1 && path.generics.is_empty() {
                    // Simple binding
                    let name = path.segments.into_iter().next().unwrap();
                    if name == "_" {
                        Ok(Pattern {
                            kind: PatternKind::Wildcard,
                            span: start,
                        })
                    } else {
                        Ok(Pattern {
                            kind: PatternKind::Binding(name),
                            span: start,
                        })
                    }
                } else {
                    // Unit variant
                    Ok(Pattern {
                        kind: PatternKind::Variant {
                            path,
                            fields: PatternFields::Unit,
                        },
                        span: start,
                    })
                }
            }
            TokenKind::Int | TokenKind::Float | TokenKind::String | TokenKind::True
            | TokenKind::False | TokenKind::None => {
                let expr = self.parse_primary()?;
                if let ExprKind::Literal(lit) = expr.kind {
                    Ok(Pattern {
                        kind: PatternKind::Literal(lit),
                        span: start,
                    })
                } else {
                    Err(ParseError::InvalidPattern { span: start })
                }
            }
            _ => Err(ParseError::InvalidPattern { span: start }),
        }
    }

    // === Query expressions ===

    fn parse_query_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Query)?;
        let target = self.parse_type_path()?;
        self.consume(TokenKind::LBrace)?;
        let body = self.parse_query_body()?;
        self.consume(TokenKind::RBrace)?;
        let end = self.span();

        Ok(Expr {
            kind: ExprKind::Query { target, body: Box::new(body) },
            span: start.merge(end),
        })
    }

    fn parse_query_body(&mut self) -> Result<QueryBody, ParseError> {
        let start = self.span();

        // SELECT clause (required)
        self.consume(TokenKind::Select)?;
        let select = self.parse_select_clause()?;

        // FROM clause (optional)
        let from = if self.at(TokenKind::From) {
            Some(self.parse_from_clause()?)
        } else {
            None
        };

        // JOIN clauses
        let mut joins = Vec::new();
        while self.at_any(&[
            TokenKind::Inner,
            TokenKind::Left,
            TokenKind::Right,
            TokenKind::Outer,
            TokenKind::Join,
        ]) {
            joins.push(self.parse_join_clause()?);
        }

        // WHERE clause
        let where_clause = if self.at(TokenKind::Where) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        // ORDER BY clause
        let order_by = if self.at(TokenKind::Order) {
            self.advance();
            self.consume(TokenKind::By)?;
            self.parse_order_items()?
        } else {
            vec![]
        };

        // LIMIT clause
        let limit = if self.at(TokenKind::Limit) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        // OFFSET clause
        let offset = if self.at(TokenKind::Offset) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        let end = self.span();
        Ok(QueryBody {
            select,
            from,
            joins,
            where_clause,
            order_by,
            limit,
            offset,
            span: start.merge(end),
        })
    }

    fn parse_select_clause(&mut self) -> Result<SelectClause, ParseError> {
        let start = self.span();

        let items = if self.at(TokenKind::Star) {
            self.advance();
            SelectItems::Star
        } else {
            let mut items = Vec::new();
            loop {
                let item_start = self.span();
                let expr = self.parse_expr()?;
                let alias = if self.at(TokenKind::As) {
                    self.advance();
                    Some(self.consume_text(TokenKind::Ident)?)
                } else {
                    None
                };
                let item_end = self.span();
                items.push(SelectItem {
                    expr,
                    alias,
                    span: item_start.merge(item_end),
                });

                if !self.at(TokenKind::Comma) {
                    break;
                }
                self.advance();
            }
            SelectItems::List(items)
        };

        let end = self.span();
        Ok(SelectClause {
            items,
            span: start.merge(end),
        })
    }

    fn parse_from_clause(&mut self) -> Result<FromClause, ParseError> {
        let start = self.span();
        self.consume(TokenKind::From)?;
        let table = self.consume_text(TokenKind::Ident)?;

        let alias = if self.at(TokenKind::As) {
            self.advance();
            Some(self.consume_text(TokenKind::Ident)?)
        } else {
            None
        };

        let end = self.span();
        Ok(FromClause {
            table,
            alias,
            span: start.merge(end),
        })
    }

    fn parse_join_clause(&mut self) -> Result<JoinClause, ParseError> {
        let start = self.span();

        let kind = match self.peek() {
            TokenKind::Inner => {
                self.advance();
                JoinKind::Inner
            }
            TokenKind::Left => {
                self.advance();
                JoinKind::Left
            }
            TokenKind::Right => {
                self.advance();
                JoinKind::Right
            }
            TokenKind::Outer => {
                self.advance();
                JoinKind::Outer
            }
            _ => JoinKind::Inner,
        };

        self.consume(TokenKind::Join)?;
        let table = self.consume_text(TokenKind::Ident)?;
        self.consume(TokenKind::On)?;
        let condition = self.parse_expr()?;
        let end = self.span();

        Ok(JoinClause {
            kind,
            table,
            condition,
            span: start.merge(end),
        })
    }

    fn parse_order_items(&mut self) -> Result<Vec<OrderItem>, ParseError> {
        let mut items = Vec::new();
        loop {
            let start = self.span();
            let expr = self.parse_expr()?;
            let direction = match self.peek() {
                TokenKind::Asc => {
                    self.advance();
                    OrderDirection::Asc
                }
                TokenKind::Desc => {
                    self.advance();
                    OrderDirection::Desc
                }
                _ => OrderDirection::Asc,
            };
            let end = self.span();
            items.push(OrderItem {
                expr,
                direction,
                span: start.merge(end),
            });

            if !self.at(TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        Ok(items)
    }

    fn parse_insert_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Insert)?;
        self.consume(TokenKind::Into)?;
        let target = self.parse_type_path()?;

        // Parse struct literal value
        self.consume(TokenKind::LBrace)?;
        let fields = self.parse_field_inits()?;
        self.consume(TokenKind::RBrace)?;

        let value_span = start.merge(self.span());
        let value = Expr {
            kind: ExprKind::Struct {
                path: None,
                fields,
            },
            span: value_span,
        };

        let end = self.span();
        Ok(Expr {
            kind: ExprKind::Insert {
                target,
                value: Box::new(value),
            },
            span: start.merge(end),
        })
    }

    fn parse_update_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Update)?;
        let target = self.parse_type_path()?;
        self.consume(TokenKind::Set)?;

        let assignments = self.parse_field_inits()?;

        let condition = if self.at(TokenKind::Where) {
            self.advance();
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        let end = self.span();
        Ok(Expr {
            kind: ExprKind::Update {
                target,
                assignments,
                condition,
            },
            span: start.merge(end),
        })
    }

    fn parse_delete_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Delete)?;
        self.consume(TokenKind::From)?;
        let target = self.parse_type_path()?;

        let condition = if self.at(TokenKind::Where) {
            self.advance();
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        let end = self.span();
        Ok(Expr {
            kind: ExprKind::Delete { target, condition },
            span: start.merge(end),
        })
    }

    // === Snippet Parsing ===

    fn parse_snippet(&mut self) -> Result<Snippet, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Snippet)?;

        // Parse: id="..." kind="..."
        let id = self.parse_attribute("id")?;
        let kind = self.parse_snippet_kind()?;

        // For extern-impl snippets: parse implements="..." and platform="..."
        let (implements, platform) = if kind == SnippetKind::ExternImpl {
            let implements = self.parse_optional_attribute("implements")?;
            let platform = self.parse_optional_attribute("platform")?;
            (implements, platform)
        } else {
            (None, None)
        };

        // Parse optional notes
        let mut notes = Vec::new();
        while self.at(TokenKind::Note) {
            notes.push(self.parse_note()?);
        }

        // Parse sections (order-independent)
        let mut sections = Vec::new();
        while !self.at(TokenKind::End) && !self.at(TokenKind::Eof) {
            sections.push(self.parse_section()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(Snippet {
            id,
            kind,
            notes,
            sections,
            implements,
            platform,
            span: start.merge(end),
        })
    }

    fn parse_attribute(&mut self, expected_name: &str) -> Result<String, ParseError> {
        // Handle attribute names that are also keywords
        let attr_name = match self.peek() {
            TokenKind::Id => { self.advance(); "id".to_string() }
            TokenKind::Kind => { self.advance(); "kind".to_string() }
            TokenKind::Type => { self.advance(); "type".to_string() }
            TokenKind::Fn => { self.advance(); "fn".to_string() }
            TokenKind::As => { self.advance(); "as".to_string() }
            TokenKind::From => { self.advance(); "from".to_string() }
            TokenKind::On => { self.advance(); "on".to_string() }
            TokenKind::Var => { self.advance(); "var".to_string() }
            TokenKind::Database => { self.advance(); "database".to_string() }
            TokenKind::Ident => self.consume_text(TokenKind::Ident)?,
            _ => {
                return Err(ParseError::unexpected(
                    "identifier",
                    self.peek(),
                    self.span(),
                ));
            }
        };
        if attr_name != expected_name {
            return Err(ParseError::Unexpected {
                expected: format!("attribute '{}'", expected_name),
                found: self.peek(),
                span: self.span(),
            });
        }
        self.consume(TokenKind::Eq)?;
        self.consume_string_literal()
    }

    /// Parse an optional attribute like `implements="..."` or `platform="..."`
    /// Returns None if the attribute isn't present
    fn parse_optional_attribute(&mut self, expected_name: &str) -> Result<Option<String>, ParseError> {
        // Check if the next token is an identifier or keyword matching our expected name
        let is_match = match self.peek() {
            TokenKind::Ident => {
                // Get the token text to check if it matches
                let token = self.current();
                let text = &self.source[token.span.start..token.span.end];
                text == expected_name
            }
            // Handle attribute names that are also keywords
            TokenKind::Platform => expected_name == "platform",
            _ => false,
        };

        if is_match {
            self.advance();
            self.consume(TokenKind::Eq)?;
            Ok(Some(self.consume_string_literal()?))
        } else {
            Ok(None)
        }
    }

    fn parse_snippet_kind(&mut self) -> Result<SnippetKind, ParseError> {
        let kind_str = self.parse_attribute("kind")?;
        match kind_str.as_str() {
            "fn" => Ok(SnippetKind::Function),
            "struct" => Ok(SnippetKind::Struct),
            "enum" => Ok(SnippetKind::Enum),
            "module" => Ok(SnippetKind::Module),
            "database" => Ok(SnippetKind::Database),
            "extern" => Ok(SnippetKind::Extern),
            "extern-abstract" => Ok(SnippetKind::ExternAbstract),
            "extern-impl" => Ok(SnippetKind::ExternImpl),
            "test" => Ok(SnippetKind::Test),
            "data" => Ok(SnippetKind::Data),
            _ => Err(ParseError::InvalidSnippetKind {
                kind: kind_str,
                span: self.span(),
            }),
        }
    }

    fn parse_note(&mut self) -> Result<Note, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Note)?;

        // Optional: lang="en" - lang is a keyword TokenKind::Lang
        let lang = if self.at(TokenKind::Lang) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            Some(self.consume_string_literal()?)
        } else {
            None
        };

        // Check if it's a triple-quoted string (which requires 'end')
        let is_triple = self.at(TokenKind::TripleString);
        let content = self.consume_string_literal()?;

        // Notes with triple-quoted strings have 'end' keyword
        if is_triple {
            self.consume(TokenKind::End)?;
        }

        let end = self.span();

        Ok(Note {
            lang,
            content,
            span: start.merge(end),
        })
    }

    fn parse_section(&mut self) -> Result<Section, ParseError> {
        match self.peek() {
            TokenKind::Signature => Ok(Section::Signature(self.parse_signature_section()?)),
            TokenKind::Body => Ok(Section::Body(self.parse_body_section()?)),
            TokenKind::Effects => Ok(Section::Effects(self.parse_effects_section()?)),
            TokenKind::Metadata => Ok(Section::Metadata(self.parse_metadata_section()?)),
            TokenKind::Requires => Ok(Section::Requires(self.parse_requires_section()?)),
            TokenKind::Tests => Ok(Section::Tests(self.parse_tests_section()?)),
            TokenKind::Relations => Ok(Section::Relations(self.parse_relations_section()?)),
            TokenKind::Content => Ok(Section::Content(self.parse_content_section()?)),
            TokenKind::Schema => Ok(Section::Schema(self.parse_schema_section()?)),
            TokenKind::Types => Ok(Section::Types(self.parse_types_section()?)),
            TokenKind::Platforms => Ok(Section::Platforms(self.parse_platforms_section()?)),
            _ => Err(ParseError::UnexpectedSection {
                section: self.peek().describe().to_string(),
                span: self.span(),
            }),
        }
    }

    fn parse_signature_section(&mut self) -> Result<SignatureSection, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Signature)?;

        let kind = match self.peek() {
            TokenKind::Fn => SignatureKind::Function(self.parse_function_signature()?),
            TokenKind::Struct => SignatureKind::Struct(self.parse_struct_signature()?),
            TokenKind::Enum => SignatureKind::Enum(self.parse_enum_signature()?),
            _ => {
                return Err(ParseError::Unexpected {
                    expected: "'fn', 'struct', or 'enum'".to_string(),
                    found: self.peek(),
                    span: self.span(),
                })
            }
        };

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(SignatureSection {
            kind,
            span: start.merge(end),
        })
    }

    fn parse_function_signature(&mut self) -> Result<FunctionSignature, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Fn)?;
        let name = self.parse_attribute("name")?;

        // Parse params, returns, generics until "end"
        let mut params = Vec::new();
        let mut returns = None;
        let mut generics = Vec::new();

        while !self.at(TokenKind::End) && !self.at(TokenKind::Eof) {
            match self.peek() {
                TokenKind::Param => params.push(self.parse_param_decl()?),
                TokenKind::Returns => returns = Some(self.parse_returns_decl()?),
                TokenKind::Generic => generics.push(self.parse_generic_param()?),
                _ => break,
            }
        }

        self.consume(TokenKind::End)?; // fn end
        let end = self.span();

        Ok(FunctionSignature {
            name,
            params,
            returns,
            generics,
            span: start.merge(end),
        })
    }

    fn parse_generic_param(&mut self) -> Result<GenericParam, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Generic)?;
        let name = self.parse_attribute("name")?;
        let end = self.span();
        Ok(GenericParam {
            name,
            span: start.merge(end),
        })
    }

    fn parse_param_decl(&mut self) -> Result<ParamDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Param)?;
        let name = self.parse_attribute("name")?;
        let ty = self.parse_attribute_type("type")?;
        let end = self.span();

        Ok(ParamDecl {
            name,
            ty,
            span: start.merge(end),
        })
    }

    fn parse_attribute_type(&mut self, attr_name: &str) -> Result<Type, ParseError> {
        let start = self.span();
        let type_str = self.parse_attribute(attr_name)?;
        let end = self.span();
        // Simple type parsing for now - just convert string to Type
        Ok(Type {
            kind: TypeKind::Named(TypePath {
                segments: vec![type_str],
                generics: Vec::new(),
                span: start.merge(end),
            }),
            span: start.merge(end),
        })
    }

    fn parse_returns_decl(&mut self) -> Result<ReturnType, ParseError> {
        self.consume(TokenKind::Returns)?;

        // Check for union (as identifier)
        if self.at(TokenKind::Ident) && self.peek_text() == "union" {
            self.advance();
            let mut types = Vec::new();
            while self.at(TokenKind::Type) {
                let ty = self.parse_attribute_type("type")?;
                let optional = self.at(TokenKind::Ident) && self.peek_text() == "optional";
                if optional {
                    self.advance();
                }
                types.push(UnionMember { ty, optional });
            }
            self.consume(TokenKind::End)?;
            return Ok(ReturnType::Union { types });
        }

        // Check for collection of="Type"
        if self.at(TokenKind::Collection) {
            self.advance();
            self.consume(TokenKind::Of)?;
            self.consume(TokenKind::Eq)?;
            let type_name = self.consume_string_literal()?;
            let start = self.span();
            let ty = Type {
                kind: TypeKind::Named(TypePath {
                    segments: vec![type_name],
                    generics: Vec::new(),
                    span: start,
                }),
                span: start,
            };
            return Ok(ReturnType::Collection { of: ty });
        }

        // Single type or collection
        let ty = self.parse_attribute_type("type")?;
        let optional = self.at(TokenKind::Ident) && self.peek_text() == "optional";
        if optional {
            self.advance();
        }

        Ok(ReturnType::Single { ty, optional })
    }

    fn peek_text(&self) -> String {
        let span = self.current().span;
        self.source[span.start..span.end].to_string()
    }

    fn parse_struct_signature(&mut self) -> Result<StructSignature, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Struct)?;
        let name = self.parse_attribute("name")?;

        let mut fields = Vec::new();
        while self.at(TokenKind::Field) {
            fields.push(self.parse_field_decl()?);
        }

        self.consume(TokenKind::End)?; // struct end
        let end = self.span();

        Ok(StructSignature {
            name,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_field_decl(&mut self) -> Result<SnippetFieldDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Field)?;
        let name = self.parse_attribute("name")?;
        let ty = self.parse_attribute_type("type")?;

        // Optional attributes - can be flags or key=value
        // These can be keywords (Primary, Auto, Unique) or identifiers
        let mut primary = false;
        let mut auto = false;
        let mut unique = false;
        let mut optional = false;

        // Handle keyword flags first
        loop {
            match self.peek() {
                TokenKind::Primary => { self.advance(); primary = true; }
                TokenKind::Auto => { self.advance(); auto = true; }
                TokenKind::Unique => { self.advance(); unique = true; }
                TokenKind::Nullable => { self.advance(); optional = true; }
                _ => break,
            }
        }

        // Also handle key=value form like primary_key=true or identifier flags
        while self.at(TokenKind::Ident) {
            let key = self.peek_text();
            match key.as_str() {
                "primary_key" | "primary" => {
                    self.advance();
                    if self.at(TokenKind::Eq) {
                        self.advance();
                        if self.at(TokenKind::True) { self.advance(); primary = true; }
                        else if self.at(TokenKind::False) { self.advance(); }
                        else { self.advance(); } // skip other values
                    } else {
                        primary = true;
                    }
                }
                "auto" | "auto_increment" => {
                    self.advance();
                    if self.at(TokenKind::Eq) {
                        self.advance();
                        if self.at(TokenKind::True) { self.advance(); auto = true; }
                        else if self.at(TokenKind::False) { self.advance(); }
                        else { self.advance(); }
                    } else {
                        auto = true;
                    }
                }
                "unique" => {
                    self.advance();
                    if self.at(TokenKind::Eq) {
                        self.advance();
                        if self.at(TokenKind::True) { self.advance(); unique = true; }
                        else if self.at(TokenKind::False) { self.advance(); }
                        else { self.advance(); }
                    } else {
                        unique = true;
                    }
                }
                "optional" | "nullable" => {
                    self.advance();
                    if self.at(TokenKind::Eq) {
                        self.advance();
                        if self.at(TokenKind::True) { self.advance(); optional = true; }
                        else if self.at(TokenKind::False) { self.advance(); }
                        else { self.advance(); }
                    } else {
                        optional = true;
                    }
                }
                _ => break,
            }
        }

        let end = self.span();

        Ok(SnippetFieldDecl {
            name,
            ty,
            primary,
            auto,
            unique,
            optional,
            span: start.merge(end),
        })
    }

    fn parse_enum_signature(&mut self) -> Result<EnumSignature, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Enum)?;
        let name = self.parse_attribute("name")?;

        let mut variants = Vec::new();
        while self.at(TokenKind::Ident) && self.peek_text() == "variant" {
            variants.push(self.parse_variant_decl()?);
        }

        self.consume(TokenKind::End)?; // enum end
        let end = self.span();

        Ok(EnumSignature {
            name,
            variants,
            span: start.merge(end),
        })
    }

    fn parse_variant_decl(&mut self) -> Result<SnippetVariantDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Ident)?; // "variant"
        let name = self.parse_attribute("name")?;

        // Optional fields
        let fields = if self.at(TokenKind::Field) {
            let mut field_list = Vec::new();
            while self.at(TokenKind::Field) {
                field_list.push(self.parse_field_decl()?);
            }
            Some(field_list)
        } else {
            None
        };

        self.consume(TokenKind::End)?; // variant end
        let end = self.span();

        Ok(SnippetVariantDecl {
            name,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_body_section(&mut self) -> Result<BodySection, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Body)?;

        let mut steps = Vec::new();
        while self.at(TokenKind::Step) {
            steps.push(self.parse_step()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(BodySection {
            steps,
            span: start.merge(end),
        })
    }

    fn parse_step(&mut self) -> Result<Step, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Step)?;

        let id = self.parse_attribute("id")?;
        let step_kind_str = self.parse_attribute("kind")?;

        let kind = match step_kind_str.as_str() {
            "return" => StepKind::Return(self.parse_return_step()?),
            "compute" => StepKind::Compute(self.parse_compute_step()?),
            "call" => StepKind::Call(self.parse_call_step()?),
            "bind" => StepKind::Bind(self.parse_bind_step()?),
            "if" => StepKind::If(self.parse_if_step()?),
            "match" => StepKind::Match(self.parse_match_step()?),
            "query" => StepKind::Query(self.parse_query_step()?),
            "insert" => StepKind::Insert(self.parse_insert_step()?),
            "update" => StepKind::Update(self.parse_update_step()?),
            "delete" => StepKind::Delete(self.parse_delete_step()?),
            "for" => StepKind::For(self.parse_for_step()?),
            "transaction" => StepKind::Transaction(self.parse_transaction_step()?),
            "traverse" => StepKind::Traverse(self.parse_traverse_step()?),
            "construct" => StepKind::Construct(self.parse_construct_step()?),
            _ => {
                return Err(ParseError::InvalidStepKind {
                    kind: step_kind_str,
                    span: self.span(),
                })
            }
        };

        // Skip optional 'mut' modifier
        if self.at(TokenKind::Mut) {
            self.advance();
        }

        let output_binding = self.parse_attribute("as")?;

        // Parse optional handle block (only valid for call steps)
        let kind = if self.at(TokenKind::Handle) {
            match kind {
                StepKind::Call(mut call_step) => {
                    call_step.handle = Some(self.parse_handle_block()?);
                    StepKind::Call(call_step)
                }
                _ => {
                    return Err(ParseError::Unexpected {
                        expected: "'end'".to_string(),
                        found: self.peek(),
                        span: self.span(),
                    });
                }
            }
        } else {
            kind
        };

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(Step {
            id,
            kind,
            output_binding,
            span: start.merge(end),
        })
    }

    fn parse_return_step(&mut self) -> Result<ReturnStep, ParseError> {
        let start = self.span();

        let value = if self.at(TokenKind::From) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            ReturnValue::Var(self.consume_string_literal()?)
        } else if self.at(TokenKind::Lit) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            ReturnValue::Lit(self.parse_literal()?)
        } else if self.at(TokenKind::Struct) {
            ReturnValue::Struct(self.parse_struct_construction()?)
        } else if self.at(TokenKind::Ident) && self.peek_text() == "variant" {
            ReturnValue::Variant(self.parse_variant_construction()?)
        } else {
            return Err(ParseError::Unexpected {
                expected: "'from', 'lit', 'struct', or 'variant'".to_string(),
                found: self.peek(),
                span: self.span(),
            });
        };

        let end = self.span();

        Ok(ReturnStep {
            value,
            span: start.merge(end),
        })
    }

    fn parse_struct_construction(&mut self) -> Result<StructConstruction, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Struct)?;
        let ty = self.parse_attribute_type("type")?;

        // Parse field assignments
        let mut fields = Vec::new();
        while self.at(TokenKind::Field) {
            fields.push(self.parse_inline_field_assignment()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(StructConstruction {
            ty,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_variant_construction(&mut self) -> Result<VariantConstruction, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Ident)?; // "variant"
        let ty = self.parse_attribute("type")?;

        // Parse field assignments
        let mut fields = Vec::new();
        while self.at(TokenKind::Field) {
            fields.push(self.parse_inline_field_assignment()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(VariantConstruction {
            ty,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_inline_field_assignment(&mut self) -> Result<FieldAssignment, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Field)?;
        let name = self.parse_attribute("name")?;
        let value = self.parse_input_source()?;
        let end = self.span();

        Ok(FieldAssignment {
            name,
            value,
            span: start.merge(end),
        })
    }

    fn parse_compute_step(&mut self) -> Result<ComputeStep, ParseError> {
        let start = self.span();

        // op=add
        self.consume(TokenKind::Op)?;
        self.consume(TokenKind::Eq)?;
        let op = self.parse_operation()?;

        // Parse inputs
        let mut inputs = Vec::new();
        while self.at(TokenKind::Input) {
            inputs.push(self.parse_input()?);
        }

        let end = self.span();

        Ok(ComputeStep {
            op,
            inputs,
            span: start.merge(end),
        })
    }

    fn parse_operation(&mut self) -> Result<Operation, ParseError> {
        // Operations are keywords, not identifiers
        match self.peek() {
            TokenKind::Add => {
                self.advance();
                Ok(Operation::Add)
            }
            TokenKind::Sub => {
                self.advance();
                Ok(Operation::Sub)
            }
            TokenKind::Mul => {
                self.advance();
                Ok(Operation::Mul)
            }
            TokenKind::Div => {
                self.advance();
                Ok(Operation::Div)
            }
            TokenKind::Equals => {
                self.advance();
                Ok(Operation::Equals)
            }
            TokenKind::And => {
                self.advance();
                Ok(Operation::And)
            }
            TokenKind::Or => {
                self.advance();
                Ok(Operation::Or)
            }
            TokenKind::Not => {
                self.advance();
                Ok(Operation::Not)
            }
            TokenKind::Contains => {
                self.advance();
                Ok(Operation::Contains)
            }
            TokenKind::Ident => {
                // Also support identifiers for extended operations
                let op_name = self.consume_text(TokenKind::Ident)?;
                match op_name.as_str() {
                    // Comparison
                    "less_eq" => Ok(Operation::LessEq),
                    "less" => Ok(Operation::Less),
                    "greater_eq" => Ok(Operation::GreaterEq),
                    "greater" => Ok(Operation::Greater),
                    "not_equals" => Ok(Operation::NotEquals),

                    // Arithmetic
                    "mod" => Ok(Operation::Mod),
                    "neg" => Ok(Operation::Neg),

                    // String operations
                    "concat" => Ok(Operation::Concat),
                    "contains" => Ok(Operation::Contains),
                    "slice" => Ok(Operation::Slice),
                    "upper" => Ok(Operation::Upper),
                    "lower" => Ok(Operation::Lower),
                    "trim" => Ok(Operation::Trim),
                    "trim_start" => Ok(Operation::TrimStart),
                    "trim_end" => Ok(Operation::TrimEnd),
                    "replace" => Ok(Operation::Replace),
                    "split" => Ok(Operation::Split),
                    "join" => Ok(Operation::Join),
                    "repeat" => Ok(Operation::Repeat),
                    "str_len" => Ok(Operation::StrLen),
                    "byte_len" => Ok(Operation::ByteLen),
                    "is_empty" => Ok(Operation::IsEmpty),
                    "starts_with" => Ok(Operation::StartsWith),
                    "ends_with" => Ok(Operation::EndsWith),
                    "index_of" => Ok(Operation::IndexOf),
                    "char_at" => Ok(Operation::CharAt),
                    "str_reverse" => Ok(Operation::StrReverse),
                    "pad_start" => Ok(Operation::PadStart),
                    "pad_end" => Ok(Operation::PadEnd),

                    // Numeric operations
                    "abs" => Ok(Operation::Abs),
                    "min" => Ok(Operation::Min),
                    "max" => Ok(Operation::Max),
                    "clamp" => Ok(Operation::Clamp),
                    "pow" => Ok(Operation::Pow),
                    "sqrt" => Ok(Operation::Sqrt),
                    "floor" => Ok(Operation::Floor),
                    "ceil" => Ok(Operation::Ceil),
                    "round" => Ok(Operation::Round),
                    "trunc" => Ok(Operation::Trunc),
                    "sign" => Ok(Operation::Sign),

                    // Bitwise operations
                    "bit_and" => Ok(Operation::BitAnd),
                    "bit_or" => Ok(Operation::BitOr),
                    "bit_xor" => Ok(Operation::BitXor),
                    "bit_not" => Ok(Operation::BitNot),
                    "bit_shl" => Ok(Operation::BitShl),
                    "bit_shr" => Ok(Operation::BitShr),
                    "bit_ushr" => Ok(Operation::BitUshr),

                    // Conversion operations
                    "to_int" => Ok(Operation::ToInt),
                    "to_float" => Ok(Operation::ToFloat),
                    "to_string" => Ok(Operation::ToString),
                    "parse_int" => Ok(Operation::ParseInt),
                    "parse_float" => Ok(Operation::ParseFloat),

                    // List operations
                    "list_len" => Ok(Operation::ListLen),
                    "list_get" => Ok(Operation::ListGet),
                    "list_first" => Ok(Operation::ListFirst),
                    "list_last" => Ok(Operation::ListLast),
                    "list_append" => Ok(Operation::ListAppend),
                    "list_prepend" => Ok(Operation::ListPrepend),
                    "list_concat" => Ok(Operation::ListConcat),
                    "list_slice" => Ok(Operation::ListSlice),
                    "list_reverse" => Ok(Operation::ListReverse),
                    "list_take" => Ok(Operation::ListTake),
                    "list_drop" => Ok(Operation::ListDrop),
                    "list_contains" => Ok(Operation::ListContains),
                    "list_index_of" => Ok(Operation::ListIndexOf),
                    "list_is_empty" => Ok(Operation::ListIsEmpty),
                    "list_sort" => Ok(Operation::ListSort),
                    "list_dedup" => Ok(Operation::ListDedup),
                    "list_flatten" => Ok(Operation::ListFlatten),

                    // Map operations
                    "map_len" => Ok(Operation::MapLen),
                    "map_get" => Ok(Operation::MapGet),
                    "map_has" => Ok(Operation::MapHas),
                    "map_insert" => Ok(Operation::MapInsert),
                    "map_remove" => Ok(Operation::MapRemove),
                    "map_keys" => Ok(Operation::MapKeys),
                    "map_values" => Ok(Operation::MapValues),
                    "map_entries" => Ok(Operation::MapEntries),
                    "map_merge" => Ok(Operation::MapMerge),
                    "map_is_empty" => Ok(Operation::MapIsEmpty),

                    // Set operations
                    "set_len" => Ok(Operation::SetLen),
                    "set_has" => Ok(Operation::SetHas),
                    "set_add" => Ok(Operation::SetAdd),
                    "set_remove" => Ok(Operation::SetRemove),
                    "set_union" => Ok(Operation::SetUnion),
                    "set_intersect" => Ok(Operation::SetIntersect),
                    "set_diff" => Ok(Operation::SetDiff),
                    "set_symmetric_diff" => Ok(Operation::SetSymmetricDiff),
                    "set_is_subset" => Ok(Operation::SetIsSubset),
                    "set_is_superset" => Ok(Operation::SetIsSuperset),
                    "set_is_empty" => Ok(Operation::SetIsEmpty),
                    "set_to_list" => Ok(Operation::SetToList),

                    // DateTime operations
                    "dt_year" => Ok(Operation::DtYear),
                    "dt_month" => Ok(Operation::DtMonth),
                    "dt_day" => Ok(Operation::DtDay),
                    "dt_hour" => Ok(Operation::DtHour),
                    "dt_minute" => Ok(Operation::DtMinute),
                    "dt_second" => Ok(Operation::DtSecond),
                    "dt_weekday" => Ok(Operation::DtWeekday),
                    "dt_unix" => Ok(Operation::DtUnix),
                    "dt_add_days" => Ok(Operation::DtAddDays),
                    "dt_add_hours" => Ok(Operation::DtAddHours),
                    "dt_add_minutes" => Ok(Operation::DtAddMinutes),
                    "dt_add_seconds" => Ok(Operation::DtAddSeconds),
                    "dt_diff" => Ok(Operation::DtDiff),
                    "dt_format" => Ok(Operation::DtFormat),

                    // Bytes operations
                    "bytes_len" => Ok(Operation::BytesLen),
                    "bytes_get" => Ok(Operation::BytesGet),
                    "bytes_slice" => Ok(Operation::BytesSlice),
                    "bytes_concat" => Ok(Operation::BytesConcat),
                    "bytes_to_string" => Ok(Operation::BytesToString),
                    "bytes_to_base64" => Ok(Operation::BytesToBase64),
                    "bytes_to_hex" => Ok(Operation::BytesToHex),
                    "bytes_is_empty" => Ok(Operation::BytesIsEmpty),

                    _ => Err(ParseError::InvalidOperation {
                        name: op_name,
                        span: self.span(),
                    }),
                }
            }
            _ => Err(ParseError::Unexpected {
                expected: "operation (add, sub, mul, div, equals, and, or, not)".to_string(),
                found: self.peek(),
                span: self.span(),
            }),
        }
    }

    fn parse_input(&mut self) -> Result<Input, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Input)?;

        let source = match self.peek() {
            TokenKind::Var => {
                self.advance();
                self.consume(TokenKind::Eq)?;
                InputSource::Var(self.consume_string_literal()?)
            }
            TokenKind::Lit => {
                self.advance();
                self.consume(TokenKind::Eq)?;
                InputSource::Lit(self.parse_literal()?)
            }
            TokenKind::Field => {
                // field="count" of="found"
                self.advance();
                self.consume(TokenKind::Eq)?;
                let field = self.consume_string_literal()?;
                self.consume(TokenKind::Of)?;
                self.consume(TokenKind::Eq)?;
                let of = self.consume_string_literal()?;
                InputSource::Field { of, field }
            }
            _ => {
                return Err(ParseError::Unexpected {
                    expected: "'var', 'lit', or 'field'".to_string(),
                    found: self.peek(),
                    span: self.span(),
                })
            }
        };

        let end = self.span();

        Ok(Input {
            source,
            span: start.merge(end),
        })
    }

    fn parse_call_step(&mut self) -> Result<CallStep, ParseError> {
        let start = self.span();
        let fn_name = self.parse_attribute("fn")?;

        let mut args = Vec::new();
        while self.at(TokenKind::Ident) && self.peek_text() == "arg" {
            args.push(self.parse_call_arg()?);
        }

        let end = self.span();

        // Note: handle block is parsed by parse_step and injected separately
        Ok(CallStep {
            fn_name,
            args,
            handle: None,
            span: start.merge(end),
        })
    }

    fn parse_handle_block(&mut self) -> Result<HandleBlock, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Handle)?;

        let mut cases = Vec::new();
        while self.at(TokenKind::Ident) && self.peek_text() == "case" {
            cases.push(self.parse_handle_case()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(HandleBlock {
            cases,
            span: start.merge(end),
        })
    }

    fn parse_handle_case(&mut self) -> Result<HandleCase, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Ident)?; // "case"
        let error_type = self.parse_attribute("type")?;

        let mut steps = Vec::new();
        while self.at(TokenKind::Step) {
            steps.push(self.parse_step()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(HandleCase {
            error_type,
            steps,
            span: start.merge(end),
        })
    }

    fn parse_call_arg(&mut self) -> Result<CallArg, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Ident)?; // "arg"
        let name = self.parse_attribute("name")?;

        let source = if self.at(TokenKind::From) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            InputSource::Var(self.consume_string_literal()?)
        } else if self.at(TokenKind::Lit) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            InputSource::Lit(self.parse_literal()?)
        } else if self.at(TokenKind::Field) {
            // field="x" of="y"
            self.advance();
            self.consume(TokenKind::Eq)?;
            let field = self.consume_string_literal()?;
            self.consume(TokenKind::Of)?;
            self.consume(TokenKind::Eq)?;
            let of = self.consume_string_literal()?;
            InputSource::Field { of, field }
        } else if self.at(TokenKind::Fn) {
            // fn="|f| equals(f.name, field_name)" - lambda syntax
            self.advance();
            self.consume(TokenKind::Eq)?;
            InputSource::Var(self.consume_string_literal()?)
        } else {
            return Err(ParseError::Unexpected {
                expected: "'from', 'lit', 'field', or 'fn'".to_string(),
                found: self.peek(),
                span: self.span(),
            });
        };

        let end = self.span();

        Ok(CallArg {
            name,
            source,
            span: start.merge(end),
        })
    }

    fn parse_bind_step(&mut self) -> Result<BindStep, ParseError> {
        let start = self.span();

        // Parse: field="year" of="d" OR var="x" OR from="x" OR lit=123
        let source = if self.at(TokenKind::Field) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            let field = self.consume_string_literal()?;
            self.consume(TokenKind::Of)?;
            self.consume(TokenKind::Eq)?;
            let of = self.consume_string_literal()?;
            BindSource::Field { of, field }
        } else if self.at(TokenKind::Var) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            BindSource::Var(self.consume_string_literal()?)
        } else if self.at(TokenKind::From) {
            // from="value" - bind from a variable
            self.advance();
            self.consume(TokenKind::Eq)?;
            BindSource::Var(self.consume_string_literal()?)
        } else if self.at(TokenKind::Lit) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            BindSource::Lit(self.parse_literal()?)
        } else {
            return Err(ParseError::Unexpected {
                expected: "'field', 'var', 'from', or 'lit'".to_string(),
                found: self.peek(),
                span: self.span(),
            });
        };

        let end = self.span();

        Ok(BindStep {
            source,
            span: start.merge(end),
        })
    }

    fn parse_if_step(&mut self) -> Result<IfStep, ParseError> {
        let start = self.span();

        // condition="is_base"
        let condition = self.parse_attribute("condition")?;

        // then ... end
        self.consume(TokenKind::Ident)?; // "then" is not a keyword
        let mut then_steps = Vec::new();
        while self.at(TokenKind::Step) {
            then_steps.push(self.parse_step()?);
        }
        self.consume(TokenKind::End)?;

        // else ... end (optional)
        let else_steps = if self.at(TokenKind::Else) {
            self.advance();
            let mut steps = Vec::new();
            while self.at(TokenKind::Step) {
                steps.push(self.parse_step()?);
            }
            self.consume(TokenKind::End)?;
            Some(steps)
        } else {
            None
        };

        let end = self.span();

        Ok(IfStep {
            condition,
            then_steps,
            else_steps,
            span: start.merge(end),
        })
    }

    fn parse_match_step(&mut self) -> Result<MatchStep, ParseError> {
        let start = self.span();

        // on="value"
        let on = self.parse_attribute("on")?;

        // Parse cases
        let mut cases = Vec::new();
        while self.at(TokenKind::Ident) && self.peek_text() == "case" {
            cases.push(self.parse_match_case()?);
        }

        let end = self.span();

        Ok(MatchStep {
            on,
            cases,
            span: start.merge(end),
        })
    }

    fn parse_match_case(&mut self) -> Result<MatchCase, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Ident)?; // "case"

        // Parse pattern: variant type="Json::Null" bindings=("s") OR wildcard
        let pattern = if self.at(TokenKind::Ident) {
            match self.peek_text().as_str() {
                "variant" => {
                    self.advance();
                    let variant = self.parse_attribute("type")?;
                    // Optional bindings=("a", "b")
                    let bindings = if self.at(TokenKind::Ident) && self.peek_text() == "bindings" {
                        self.advance();
                        self.consume(TokenKind::Eq)?;
                        self.parse_bindings_list()?
                    } else {
                        Vec::new()
                    };
                    MatchPattern::Variant { variant, bindings }
                }
                "wildcard" => {
                    self.advance();
                    MatchPattern::Wildcard
                }
                _ => {
                    return Err(ParseError::Unexpected {
                        expected: "'variant' or 'wildcard'".to_string(),
                        found: self.peek(),
                        span: self.span(),
                    });
                }
            }
        } else {
            return Err(ParseError::Unexpected {
                expected: "'variant' or 'wildcard'".to_string(),
                found: self.peek(),
                span: self.span(),
            });
        };

        // Parse steps until end
        let mut steps = Vec::new();
        while self.at(TokenKind::Step) {
            steps.push(self.parse_step()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(MatchCase {
            pattern,
            steps,
            span: start.merge(end),
        })
    }

    fn parse_bindings_list(&mut self) -> Result<Vec<String>, ParseError> {
        // Parse ("a", "b", "c")
        self.consume(TokenKind::LParen)?;
        let mut bindings = Vec::new();
        if !self.at(TokenKind::RParen) {
            bindings.push(self.consume_string_literal()?);
            while self.at(TokenKind::Comma) {
                self.advance();
                bindings.push(self.consume_string_literal()?);
            }
        }
        self.consume(TokenKind::RParen)?;
        Ok(bindings)
    }

    fn parse_query_step(&mut self) -> Result<QueryStep, ParseError> {
        let start = self.span();

        // Check for dialect="..." first (SQL dialect mode)
        let dialect = if self.at(TokenKind::Dialect) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            Some(self.consume_string_literal()?)
        } else {
            None
        };

        // target="project" or target="db"
        let target = self.parse_attribute("target")?;

        // If we have a dialect, parse SQL body
        if dialect.is_some() {
            // Parse body ... end with raw SQL
            self.consume(TokenKind::Body)?;
            let mut sql_body = String::new();
            let mut depth = 1;
            while depth > 0 && !self.at(TokenKind::Eof) {
                if self.at(TokenKind::Body) {
                    depth += 1;
                    sql_body.push_str("body ");
                    self.advance();
                } else if self.at(TokenKind::End) {
                    depth -= 1;
                    if depth > 0 {
                        sql_body.push_str("end ");
                    }
                    self.advance();
                } else {
                    let text = self.advance_text();
                    sql_body.push_str(&text);
                    sql_body.push(' ');
                }
            }

            // Parse params ... end (optional)
            let mut params = Vec::new();
            if self.at(TokenKind::Ident) && self.peek_text() == "params" {
                self.advance();
                while self.at(TokenKind::Param) {
                    self.advance();
                    let param_name = self.parse_attribute("name")?;
                    let from = self.parse_attribute("from")?;
                    params.push(ParamBinding {
                        name: param_name,
                        from,
                        span: self.span(),
                    });
                }
                self.consume(TokenKind::End)?;
            }

            // Parse returns ... (optional)
            let returns = if self.at(TokenKind::Returns) {
                self.parse_returns_decl()?
            } else {
                ReturnType::Single {
                    ty: Type {
                        kind: TypeKind::Named(TypePath {
                            segments: vec!["Any".to_string()],
                            generics: Vec::new(),
                            span: start,
                        }),
                        span: start,
                    },
                    optional: false,
                }
            };

            let end = self.span();
            return Ok(QueryStep {
                dialect,
                target,
                content: QueryContent::Dialect(DialectQuery {
                    body: sql_body.trim().to_string(),
                    params,
                    returns,
                    span: start.merge(end),
                }),
                span: start.merge(end),
            });
        }

        // Covenant query format
        // select all or select field="..."
        self.consume(TokenKind::Select)?;
        let select = if self.at(TokenKind::Ident) && self.peek_text() == "all" {
            self.advance();
            SnippetSelectClause::All
        } else if self.at(TokenKind::Field) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            SnippetSelectClause::Field(self.consume_string_literal()?)
        } else {
            return Err(ParseError::Unexpected {
                expected: "'all' or 'field'".to_string(),
                found: self.peek(),
                span: self.span(),
            });
        };

        // from="..."
        self.consume(TokenKind::From)?;
        self.consume(TokenKind::Eq)?;
        let from = self.consume_string_literal()?;

        // Optional where clause
        let where_clause = if self.at(TokenKind::Where) {
            Some(self.parse_where_clause()?)
        } else {
            None
        };

        // Optional order
        let order = if self.at(TokenKind::Order) {
            Some(self.parse_order_clause()?)
        } else {
            None
        };

        // Optional limit
        let limit = if self.at(TokenKind::Limit) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            let text = self.advance_text();
            Some(text.parse::<u64>().unwrap_or(0))
        } else {
            None
        };

        let end = self.span();

        Ok(QueryStep {
            dialect: None,
            target,
            content: QueryContent::Covenant(CovenantQuery {
                select,
                from,
                where_clause,
                order,
                limit,
                span: start.merge(end),
            }),
            span: start.merge(end),
        })
    }

    fn parse_where_clause(&mut self) -> Result<Condition, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Where)?;

        let kind = self.parse_condition_kind()?;

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(Condition {
            kind,
            span: start.merge(end),
        })
    }

    fn parse_condition_kind(&mut self) -> Result<ConditionKind, ParseError> {
        // equals field="id" var="id" / lit=123
        // contains field="effects" lit="database"
        // and ... end / or ... end
        match self.peek() {
            TokenKind::Equals => {
                self.advance();
                self.consume(TokenKind::Field)?;
                self.consume(TokenKind::Eq)?;
                let field = self.consume_string_literal()?;
                let value = self.parse_input_source()?;
                Ok(ConditionKind::Equals { field, value })
            }
            TokenKind::Contains => {
                self.advance();
                self.consume(TokenKind::Field)?;
                self.consume(TokenKind::Eq)?;
                let field = self.consume_string_literal()?;
                let value = self.parse_input_source()?;
                Ok(ConditionKind::Contains { field, value })
            }
            TokenKind::And => {
                self.advance();
                // Parse multiple conditions inside and block
                let mut conditions = Vec::new();
                while !self.at(TokenKind::End) && !self.at(TokenKind::Eof) {
                    conditions.push(self.parse_condition_kind()?);
                }
                self.consume(TokenKind::End)?;
                // Build nested And structure
                if conditions.is_empty() {
                    return Err(ParseError::Unexpected {
                        expected: "condition".to_string(),
                        found: self.peek(),
                        span: self.span(),
                    });
                }
                let mut result = conditions.pop().unwrap();
                while let Some(cond) = conditions.pop() {
                    let start = self.span();
                    result = ConditionKind::And(
                        Box::new(Condition { kind: cond, span: start }),
                        Box::new(Condition { kind: result, span: start }),
                    );
                }
                Ok(result)
            }
            TokenKind::Or => {
                self.advance();
                // Parse multiple conditions inside or block
                let mut conditions = Vec::new();
                while !self.at(TokenKind::End) && !self.at(TokenKind::Eof) {
                    conditions.push(self.parse_condition_kind()?);
                }
                self.consume(TokenKind::End)?;
                // Build nested Or structure
                if conditions.is_empty() {
                    return Err(ParseError::Unexpected {
                        expected: "condition".to_string(),
                        found: self.peek(),
                        span: self.span(),
                    });
                }
                let mut result = conditions.pop().unwrap();
                while let Some(cond) = conditions.pop() {
                    let start = self.span();
                    result = ConditionKind::Or(
                        Box::new(Condition { kind: cond, span: start }),
                        Box::new(Condition { kind: result, span: start }),
                    );
                }
                Ok(result)
            }
            TokenKind::Ident => {
                // Handle identifiers like "rel_to", "rel_from", "not_equals", etc.
                let ident = self.peek_text();
                match ident.as_str() {
                    "rel_to" => {
                        // rel_to target=code_id type=describes
                        self.advance();
                        // target= (where target is an identifier attribute)
                        self.consume(TokenKind::Ident)?; // "target"
                        self.consume(TokenKind::Eq)?;
                        let target = self.consume_text(TokenKind::Ident)?;
                        // type= (where type is a keyword)
                        self.consume(TokenKind::Type)?;
                        self.consume(TokenKind::Eq)?;
                        let rel_type = self.consume_text(TokenKind::Ident)?;
                        Ok(ConditionKind::RelTo { target, rel_type })
                    }
                    "rel_from" => {
                        // rel_from source=doc_id type=described_by
                        self.advance();
                        // source= (where source is an identifier attribute)
                        self.consume(TokenKind::Ident)?; // "source"
                        self.consume(TokenKind::Eq)?;
                        let source = self.consume_text(TokenKind::Ident)?;
                        // type= (where type is a keyword)
                        self.consume(TokenKind::Type)?;
                        self.consume(TokenKind::Eq)?;
                        let rel_type = self.consume_text(TokenKind::Ident)?;
                        Ok(ConditionKind::RelFrom { source, rel_type })
                    }
                    "not_equals" => {
                        self.advance();
                        self.consume(TokenKind::Field)?;
                        self.consume(TokenKind::Eq)?;
                        let field = self.consume_string_literal()?;
                        let value = self.parse_input_source()?;
                        Ok(ConditionKind::NotEquals { field, value })
                    }
                    "less" | "greater" | "matches" => {
                        self.advance();
                        self.consume(TokenKind::Field)?;
                        self.consume(TokenKind::Eq)?;
                        let field = self.consume_string_literal()?;
                        let value = self.parse_input_source()?;
                        // Map to appropriate condition - for now use Equals as placeholder
                        Ok(ConditionKind::Equals { field, value })
                    }
                    _ => Err(ParseError::Unexpected {
                        expected: "'equals', 'contains', 'and', 'or', 'rel_to', or 'rel_from'".to_string(),
                        found: self.peek(),
                        span: self.span(),
                    }),
                }
            }
            _ => Err(ParseError::Unexpected {
                expected: "'equals', 'contains', 'and', 'or', 'rel_to', or 'rel_from'".to_string(),
                found: self.peek(),
                span: self.span(),
            }),
        }
    }

    fn parse_input_source(&mut self) -> Result<InputSource, ParseError> {
        match self.peek() {
            TokenKind::Var => {
                self.advance();
                self.consume(TokenKind::Eq)?;
                Ok(InputSource::Var(self.consume_string_literal()?))
            }
            TokenKind::Lit => {
                self.advance();
                self.consume(TokenKind::Eq)?;
                Ok(InputSource::Lit(self.parse_literal()?))
            }
            TokenKind::From => {
                // from="var_name" is equivalent to var="var_name"
                self.advance();
                self.consume(TokenKind::Eq)?;
                Ok(InputSource::Var(self.consume_string_literal()?))
            }
            TokenKind::Field => {
                // field="x" of="y"
                self.advance();
                self.consume(TokenKind::Eq)?;
                let field = self.consume_string_literal()?;
                self.consume(TokenKind::Of)?;
                self.consume(TokenKind::Eq)?;
                let of = self.consume_string_literal()?;
                Ok(InputSource::Field { of, field })
            }
            TokenKind::Struct => {
                // struct=[...] - complex inline struct literal, consume as placeholder
                self.advance();
                self.consume(TokenKind::Eq)?;
                // Skip complex inline value (e.g. [{name: "x", type: y}])
                let mut bracket_depth = 0;
                let mut brace_depth = 0;
                loop {
                    match self.peek() {
                        TokenKind::LBracket => { bracket_depth += 1; self.advance(); }
                        TokenKind::RBracket => {
                            bracket_depth -= 1;
                            self.advance();
                            if bracket_depth == 0 && brace_depth == 0 { break; }
                        }
                        TokenKind::LBrace => { brace_depth += 1; self.advance(); }
                        TokenKind::RBrace => {
                            brace_depth -= 1;
                            self.advance();
                            if bracket_depth == 0 && brace_depth == 0 { break; }
                        }
                        TokenKind::Eof => break,
                        _ => { self.advance(); }
                    }
                }
                Ok(InputSource::Var("_struct_literal".to_string()))
            }
            _ => Err(ParseError::Unexpected {
                expected: "'var', 'lit', 'from', 'field', or 'struct'".to_string(),
                found: self.peek(),
                span: self.span(),
            }),
        }
    }

    fn parse_order_clause(&mut self) -> Result<OrderClause, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Order)?;
        self.consume(TokenKind::By)?;
        self.consume(TokenKind::Eq)?;
        let field = self.consume_string_literal()?;

        // Optional: dir="asc"/"desc" OR dir=asc OR just asc/desc keywords
        let direction = if self.at(TokenKind::Ident) && self.peek_text() == "dir" {
            self.advance();
            self.consume(TokenKind::Eq)?;
            // Can be keyword or string
            if self.at(TokenKind::Asc) {
                self.advance();
                SnippetOrderDirection::Asc
            } else if self.at(TokenKind::Desc) {
                self.advance();
                SnippetOrderDirection::Desc
            } else {
                let dir_str = self.consume_string_literal()?;
                match dir_str.as_str() {
                    "asc" => SnippetOrderDirection::Asc,
                    "desc" => SnippetOrderDirection::Desc,
                    _ => SnippetOrderDirection::Asc,
                }
            }
        } else if self.at(TokenKind::Asc) {
            self.advance();
            SnippetOrderDirection::Asc
        } else if self.at(TokenKind::Desc) {
            self.advance();
            SnippetOrderDirection::Desc
        } else {
            SnippetOrderDirection::Asc
        };

        let end = self.span();

        Ok(OrderClause {
            field,
            direction,
            span: start.merge(end),
        })
    }

    fn parse_insert_step(&mut self) -> Result<InsertStep, ParseError> {
        let start = self.span();

        // into="project.data_nodes"
        self.consume(TokenKind::Into)?;
        self.consume(TokenKind::Eq)?;
        let target = self.consume_string_literal()?;

        // set field="name" from="name"
        let mut assignments = Vec::new();
        while self.at(TokenKind::Set) {
            assignments.push(self.parse_field_assignment()?);
        }

        let end = self.span();

        Ok(InsertStep {
            target,
            assignments,
            span: start.merge(end),
        })
    }

    fn parse_update_step(&mut self) -> Result<UpdateStep, ParseError> {
        let start = self.span();

        // target="project.data_nodes"
        let target = self.parse_attribute("target")?;

        // set field="content" from="updated_content"
        let mut assignments = Vec::new();
        while self.at(TokenKind::Set) {
            assignments.push(self.parse_field_assignment()?);
        }

        // Optional where clause
        let where_clause = if self.at(TokenKind::Where) {
            Some(self.parse_where_clause()?)
        } else {
            None
        };

        let end = self.span();

        Ok(UpdateStep {
            target,
            assignments,
            where_clause,
            span: start.merge(end),
        })
    }

    fn parse_delete_step(&mut self) -> Result<DeleteStep, ParseError> {
        let start = self.span();

        // from="project.data_nodes"
        self.consume(TokenKind::From)?;
        self.consume(TokenKind::Eq)?;
        let target = self.consume_string_literal()?;

        // Optional where clause
        let where_clause = if self.at(TokenKind::Where) {
            Some(self.parse_where_clause()?)
        } else {
            None
        };

        let end = self.span();

        Ok(DeleteStep {
            target,
            where_clause,
            span: start.merge(end),
        })
    }

    fn parse_field_assignment(&mut self) -> Result<FieldAssignment, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Set)?;
        self.consume(TokenKind::Field)?;
        self.consume(TokenKind::Eq)?;
        let name = self.consume_string_literal()?;

        // Skip optional op=append or similar modifiers
        if self.at(TokenKind::Op) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            self.advance(); // consume the op value (like "append")
        }

        let value = self.parse_input_source()?;
        let end = self.span();

        Ok(FieldAssignment {
            name,
            value,
            span: start.merge(end),
        })
    }

    fn parse_construct_step(&mut self) -> Result<StructConstruction, ParseError> {
        let start = self.span();

        // type="Point"
        let ty = self.parse_attribute_type("type")?;

        // field name="x" from="x"
        let mut fields = Vec::new();
        while self.at(TokenKind::Field) {
            fields.push(self.parse_inline_field_assignment()?);
        }

        let end = self.span();

        Ok(StructConstruction {
            ty,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_for_step(&mut self) -> Result<ForStep, ParseError> {
        let start = self.span();

        // var="req" in="request_stream"
        let var = self.parse_attribute("var")?;
        self.consume(TokenKind::In)?;
        self.consume(TokenKind::Eq)?;
        let collection = self.consume_string_literal()?;

        // Parse nested steps
        let mut steps = Vec::new();
        while self.at(TokenKind::Step) {
            steps.push(self.parse_step()?);
        }

        let end = self.span();

        Ok(ForStep {
            var,
            collection,
            steps,
            span: start.merge(end),
        })
    }

    fn parse_transaction_step(&mut self) -> Result<TransactionStep, ParseError> {
        let start = self.span();

        // Optional isolation="serializable"
        let isolation = if self.at(TokenKind::Ident) && self.peek_text() == "isolation" {
            self.advance();
            self.consume(TokenKind::Eq)?;
            let level_str = self.consume_string_literal()?;
            Some(match level_str.as_str() {
                "read_uncommitted" => IsolationLevel::ReadUncommitted,
                "read_committed" => IsolationLevel::ReadCommitted,
                "repeatable_read" => IsolationLevel::RepeatableRead,
                "serializable" => IsolationLevel::Serializable,
                _ => IsolationLevel::ReadCommitted,
            })
        } else {
            None
        };

        // Parse nested steps
        let mut steps = Vec::new();
        while self.at(TokenKind::Step) {
            steps.push(self.parse_step()?);
        }

        let end = self.span();

        Ok(TransactionStep {
            isolation,
            steps,
            span: start.merge(end),
        })
    }

    fn parse_traverse_step(&mut self) -> Result<TraverseStep, ParseError> {
        let start = self.span();

        // target="project" from="node_id" follow type=contained_by depth=unbounded direction=outgoing
        let target = self.parse_attribute("target")?;
        let from = self.parse_attribute("from")?;

        // Support both "relation=" and "follow type=" syntax
        let relation_type = if self.at(TokenKind::Ident) && self.peek_text() == "follow" {
            self.advance();
            // type=... (where 'type' is a keyword)
            self.consume(TokenKind::Type)?;
            self.consume(TokenKind::Eq)?;
            // Relation type can be identifier or keyword like 'contains'
            self.consume_relation_type()?
        } else if self.at(TokenKind::Ident) && self.peek_text() == "relation" {
            self.parse_attribute("relation")?
        } else {
            return Err(ParseError::Unexpected {
                expected: "attribute 'follow' or 'relation'".to_string(),
                found: self.peek(),
                span: self.span(),
            });
        };

        // Optional depth: depth=2 or depth=unbounded or depth=max_depth (var reference)
        let depth = if self.at(TokenKind::Ident) && self.peek_text() == "depth" {
            self.advance();
            self.consume(TokenKind::Eq)?;
            if self.at(TokenKind::Int) {
                let text = self.advance_text();
                TraverseDepth::Bounded(text.parse().unwrap_or(1))
            } else if self.at(TokenKind::Ident) {
                let text = self.advance_text();
                if text == "unbounded" {
                    TraverseDepth::Unbounded
                } else {
                    // Variable reference like max_depth - treat as unbounded for now
                    TraverseDepth::Unbounded
                }
            } else {
                let text = self.consume_string_literal()?;
                if text == "unbounded" || text == "*" {
                    TraverseDepth::Unbounded
                } else {
                    TraverseDepth::Bounded(text.parse().unwrap_or(1))
                }
            }
        } else {
            TraverseDepth::Bounded(1)
        };

        // Optional direction: direction=outgoing/incoming/both
        let direction = if self.at(TokenKind::Ident) && self.peek_text() == "direction" {
            self.advance();
            self.consume(TokenKind::Eq)?;
            let dir_str = if self.at(TokenKind::Ident) {
                self.advance_text()
            } else {
                self.consume_string_literal()?
            };
            match dir_str.as_str() {
                "outgoing" => TraverseDirection::Outgoing,
                "incoming" => TraverseDirection::Incoming,
                "both" => TraverseDirection::Both,
                _ => TraverseDirection::Outgoing,
            }
        } else {
            TraverseDirection::Outgoing
        };

        let end = self.span();

        Ok(TraverseStep {
            target,
            from,
            relation_type,
            depth,
            direction,
            span: start.merge(end),
        })
    }

    fn parse_effects_section(&mut self) -> Result<EffectsSection, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Effects)?; // "effects" section keyword

        let mut effects = Vec::new();
        while self.at(TokenKind::Effect) {
            effects.push(self.parse_effect_decl()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(EffectsSection {
            effects,
            span: start.merge(end),
        })
    }

    fn parse_effect_decl(&mut self) -> Result<EffectDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Effect)?;
        // Effect name can be a keyword like "database" or an identifier
        let name = match self.peek() {
            TokenKind::Database => { self.advance(); "database".to_string() }
            TokenKind::Query => { self.advance(); "query".to_string() }
            TokenKind::Ident => self.consume_text(TokenKind::Ident)?,
            _ => {
                return Err(ParseError::unexpected(
                    "effect name",
                    self.peek(),
                    self.span(),
                ));
            }
        };
        let end = self.span();

        Ok(EffectDecl {
            name,
            params: Vec::new(),
            span: start.merge(end),
        })
    }

    fn parse_platforms_section(&mut self) -> Result<PlatformsSection, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Platforms)?; // "platforms" section keyword

        let mut platforms = Vec::new();
        while self.at(TokenKind::Platform) {
            platforms.push(self.parse_platform_decl()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(PlatformsSection {
            platforms,
            span: start.merge(end),
        })
    }

    fn parse_platform_decl(&mut self) -> Result<PlatformDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Platform)?;
        self.consume(TokenKind::Eq)?;
        let name = self.consume_string_literal()?;
        let end = self.span();

        Ok(PlatformDecl {
            name,
            span: start.merge(end),
        })
    }

    fn parse_metadata_section(&mut self) -> Result<MetadataSection, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Metadata)?;

        let mut entries = Vec::new();
        while !self.at(TokenKind::End) && !self.at(TokenKind::Eof) {
            entries.push(self.parse_metadata_entry()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(MetadataSection {
            entries,
            span: start.merge(end),
        })
    }

    fn parse_metadata_entry(&mut self) -> Result<MetadataEntry, ParseError> {
        let start = self.span();
        // key="value" or key=value format - key can be identifier or keyword
        let key = match self.peek() {
            TokenKind::Type => { self.advance(); "type".to_string() }
            TokenKind::Database => { self.advance(); "database".to_string() }
            TokenKind::Connection => { self.advance(); "connection".to_string() }
            TokenKind::Dialect => { self.advance(); "dialect".to_string() }
            TokenKind::Ident => self.consume_text(TokenKind::Ident)?,
            _ => {
                return Err(ParseError::unexpected(
                    "metadata key",
                    self.peek(),
                    self.span(),
                ));
            }
        };
        self.consume(TokenKind::Eq)?;
        // Value can be string, identifier, boolean, number, or array
        let value = match self.peek() {
            TokenKind::String => self.consume_string_literal()?,
            TokenKind::Ident => self.consume_text(TokenKind::Ident)?,
            TokenKind::Int => self.advance_text(),
            TokenKind::True => { self.advance(); "true".to_string() }
            TokenKind::False => { self.advance(); "false".to_string() }
            TokenKind::LBracket => {
                // Array value: ["item1", "item2", ...]
                self.advance(); // consume '['
                let mut items = Vec::new();
                while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
                    let item = self.consume_string_literal()?;
                    items.push(format!("\"{}\"", item));
                    if self.at(TokenKind::Comma) {
                        self.advance();
                    }
                }
                self.consume(TokenKind::RBracket)?;
                format!("[{}]", items.join(", "))
            }
            _ => self.consume_string_literal()?,
        };
        let end = self.span();

        Ok(MetadataEntry {
            key,
            value,
            span: start.merge(end),
        })
    }

    fn parse_requires_section(&mut self) -> Result<RequiresSection, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Requires)?;

        let mut requirements = Vec::new();
        while self.at(TokenKind::Ident) && self.peek_text() == "req" {
            requirements.push(self.parse_requirement()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(RequiresSection {
            requirements,
            span: start.merge(end),
        })
    }

    fn parse_requirement(&mut self) -> Result<Requirement, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Ident)?; // "req"
        let id = self.parse_attribute("id")?;

        // Parse req contents until "end"
        let mut text = None;
        let mut priority = None;

        while !self.at(TokenKind::End) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Ident) && self.peek_text() == "text" {
                self.advance();
                text = Some(self.consume_string_literal()?);
            } else if self.at(TokenKind::Ident) && self.peek_text() == "priority" {
                self.advance();
                let p_str = self.consume_text(TokenKind::Ident)?;
                priority = match p_str.as_str() {
                    "critical" => Some(Priority::Critical),
                    "high" => Some(Priority::High),
                    "medium" => Some(Priority::Medium),
                    "low" => Some(Priority::Low),
                    _ => None,
                };
            } else {
                break;
            }
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(Requirement {
            id,
            text,
            priority,
            status: None,
            span: start.merge(end),
        })
    }

    fn parse_tests_section(&mut self) -> Result<TestsSection, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Tests)?;

        let mut tests = Vec::new();
        while self.at(TokenKind::Ident) && self.peek_text() == "test" {
            tests.push(self.parse_test_decl()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(TestsSection {
            tests,
            span: start.merge(end),
        })
    }

    fn parse_test_decl(&mut self) -> Result<TestDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Ident)?; // "test"
        let id = self.parse_attribute("id")?;
        let kind_str = self.parse_attribute("kind")?;
        let kind = match kind_str.as_str() {
            "unit" => TestKind::Unit,
            "integration" => TestKind::Integration,
            "golden" => TestKind::Golden,
            "property" => TestKind::Property,
            _ => TestKind::Unit,
        };

        // Optional covers
        let mut covers = Vec::new();
        if self.at(TokenKind::Ident) && self.peek_text() == "covers" {
            self.advance();
            self.consume(TokenKind::Eq)?;
            covers.push(self.consume_string_literal()?);
        }

        // Parse test steps
        let mut steps = Vec::new();
        while self.at(TokenKind::Step) {
            steps.push(self.parse_step()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(TestDecl {
            id,
            kind,
            covers,
            steps,
            span: start.merge(end),
        })
    }

    fn parse_relations_section(&mut self) -> Result<RelationsSection, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Relations)?;

        let mut relations = Vec::new();
        while self.at(TokenKind::Rel) {
            relations.push(self.parse_relation_decl()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(RelationsSection {
            relations,
            span: start.merge(end),
        })
    }

    fn parse_relation_decl(&mut self) -> Result<RelationDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Rel)?;

        // rel to="target" type=describes OR rel from="source" type=described_by
        let kind = if self.at(TokenKind::Ident) && self.peek_text() == "to" {
            self.advance();
            self.consume(TokenKind::Eq)?;
            RelationKind::To
        } else if self.at(TokenKind::From) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            RelationKind::From
        } else {
            return Err(ParseError::Unexpected {
                expected: "'to' or 'from'".to_string(),
                found: self.peek(),
                span: self.span(),
            });
        };

        let target = self.consume_string_literal()?;

        // Optional type=... attribute
        if self.at(TokenKind::Type) {
            self.advance();
            self.consume(TokenKind::Eq)?;
            // Consume the relation type (identifier, not string)
            let _ = self.advance_text();
        }

        let end = self.span();

        Ok(RelationDecl {
            kind,
            target,
            span: start.merge(end),
        })
    }

    fn parse_content_section(&mut self) -> Result<ContentSection, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Content)?;

        // Content can be a triple-quoted string or regular string or key-value pairs
        let mut content = String::new();
        while !self.at(TokenKind::End) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::TripleString) || self.at(TokenKind::String) {
                content = self.consume_string_literal()?;
                break;
            } else if self.at(TokenKind::Ident) || self.at(TokenKind::Id) || self.at(TokenKind::Type) {
                // key value pairs like: name "Alice Smith" or id "R-001"
                // 'id' and 'type' are keywords so handle them specially
                let key = self.advance_text();
                if self.at(TokenKind::String) || self.at(TokenKind::TripleString) {
                    let value = self.consume_string_literal()?;
                    content.push_str(&format!("{}: {}\n", key, value));
                } else if self.at(TokenKind::LBracket) {
                    // Array values like ["item1", "item2"]
                    self.advance(); // consume '['
                    let mut items = Vec::new();
                    while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
                        if self.at(TokenKind::String) {
                            items.push(self.consume_string_literal()?);
                        }
                        if self.at(TokenKind::Comma) {
                            self.advance();
                        }
                    }
                    self.consume(TokenKind::RBracket)?;
                    content.push_str(&format!("{}: [{}]\n", key, items.join(", ")));
                } else {
                    // Skip unknown value
                    self.advance();
                }
            } else {
                self.advance();
            }
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(ContentSection {
            content,
            span: start.merge(end),
        })
    }

    fn parse_schema_section(&mut self) -> Result<SchemaSection, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Schema)?;

        let mut tables = Vec::new();
        while self.at(TokenKind::Table) || self.at(TokenKind::Field) {
            if self.at(TokenKind::Table) {
                tables.push(self.parse_snippet_table_decl()?);
            } else if self.at(TokenKind::Field) {
                // Schema can also have inline fields without table wrapper
                let mut fields = Vec::new();
                while self.at(TokenKind::Field) {
                    fields.push(self.parse_field_decl()?);
                }
                // Create a synthetic table for loose fields
                let field_span = if fields.is_empty() { start } else { fields[0].span };
                tables.push(SnippetTableDecl {
                    name: "_schema".to_string(),
                    fields,
                    span: field_span,
                });
            }
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(SchemaSection {
            tables,
            span: start.merge(end),
        })
    }

    fn parse_snippet_table_decl(&mut self) -> Result<SnippetTableDecl, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Table)?;
        let name = self.parse_attribute("name")?;

        let mut fields = Vec::new();
        while self.at(TokenKind::Field) {
            fields.push(self.parse_field_decl()?);
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(SnippetTableDecl {
            name,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_types_section(&mut self) -> Result<TypesSection, ParseError> {
        let start = self.span();
        self.consume(TokenKind::Types)?;

        let mut types = Vec::new();
        while self.at(TokenKind::Struct) || self.at(TokenKind::Enum) {
            if self.at(TokenKind::Struct) {
                // Parse struct as a type declaration
                let struct_sig = self.parse_struct_signature()?;
                let type_span = struct_sig.span;
                types.push(TypeDecl {
                    name: struct_sig.name.clone(),
                    ty: Type {
                        kind: TypeKind::Named(TypePath {
                            segments: vec![struct_sig.name],
                            generics: Vec::new(),
                            span: type_span,
                        }),
                        span: type_span,
                    },
                    span: type_span,
                });
            } else if self.at(TokenKind::Enum) {
                let enum_sig = self.parse_enum_signature()?;
                let type_span = enum_sig.span;
                types.push(TypeDecl {
                    name: enum_sig.name.clone(),
                    ty: Type {
                        kind: TypeKind::Named(TypePath {
                            segments: vec![enum_sig.name],
                            generics: Vec::new(),
                            span: type_span,
                        }),
                        span: type_span,
                    },
                    span: type_span,
                });
            }
        }

        self.consume(TokenKind::End)?;
        let end = self.span();

        Ok(TypesSection {
            types,
            span: start.merge(end),
        })
    }

    // === Helpers ===

    fn parse_literal(&mut self) -> Result<Literal, ParseError> {
        match self.peek() {
            TokenKind::Int => {
                let text = self.advance_text();
                let value: i64 = text.parse().unwrap_or(0);
                Ok(Literal::Int(value))
            }
            TokenKind::Float => {
                let text = self.advance_text();
                let value: f64 = text.parse().unwrap_or(0.0);
                Ok(Literal::Float(value))
            }
            TokenKind::String | TokenKind::TripleString => {
                let value = self.advance_string_literal();
                Ok(Literal::String(value))
            }
            TokenKind::True => {
                self.advance();
                Ok(Literal::Bool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Literal::Bool(false))
            }
            TokenKind::None => {
                self.advance();
                Ok(Literal::None)
            }
            TokenKind::LBracket => {
                // Array literal - consume and represent as string for now
                // [1, 2, 3] or []
                let mut contents = String::from("[");
                self.advance(); // consume [
                while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
                    let text = self.advance_text();
                    contents.push_str(&text);
                    if self.at(TokenKind::Comma) {
                        contents.push_str(", ");
                        self.advance();
                    }
                }
                self.consume(TokenKind::RBracket)?;
                contents.push(']');
                Ok(Literal::String(contents))
            }
            TokenKind::LBrace => {
                // Object/map literal - consume and represent as string for now
                // {"key": "value"} or {}
                let mut contents = String::from("{");
                self.advance(); // consume {
                while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                    let text = self.advance_text();
                    contents.push_str(&text);
                    if self.at(TokenKind::Colon) {
                        contents.push_str(": ");
                        self.advance();
                    } else if self.at(TokenKind::Comma) {
                        contents.push_str(", ");
                        self.advance();
                    }
                }
                self.consume(TokenKind::RBrace)?;
                contents.push('}');
                Ok(Literal::String(contents))
            }
            _ => Err(ParseError::Unexpected {
                expected: "literal".to_string(),
                found: self.peek(),
                span: self.span(),
            }),
        }
    }
}
