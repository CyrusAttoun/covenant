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
        let token = self.current();
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

    fn text(&self, token: &Token) -> &'a str {
        token.text(self.source)
    }

    fn span(&self) -> Span {
        self.current().span
    }

    // === Program ===

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let start = self.span();
        let mut declarations = Vec::new();

        while !self.at(TokenKind::Eof) {
            declarations.push(self.parse_declaration()?);
        }

        let end = self.span();
        Ok(Program {
            declarations,
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
        let source_token = self.consume(TokenKind::Ident)?;
        let source = self.text(source_token).to_string();
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
        let name_token = self.consume(TokenKind::Ident)?;
        let name = self.text(name_token).to_string();
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
        let name_token = self.consume(TokenKind::Ident)?;
        let name = self.text(name_token).to_string();

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
        while self.at(TokenKind::Ident) {
            let start = self.span();
            let name_token = self.consume(TokenKind::Ident)?;
            let name = self.text(name_token).to_string();
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
        let name_token = self.consume(TokenKind::Ident)?;
        let name = self.text(name_token).to_string();

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
            let name_token = self.consume(TokenKind::Ident)?;
            let name = self.text(name_token).to_string();

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
        let name_token = self.consume(TokenKind::Ident)?;
        let name = self.text(name_token).to_string();

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
        let name_token = self.consume(TokenKind::Ident)?;
        let name = self.text(name_token).to_string();

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
        let source_token = self.consume(TokenKind::String)?;
        let source = self.parse_string_literal(source_token);

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
        let name_token = self.consume(TokenKind::Ident)?;
        let name = self.text(name_token).to_string();

        let connection = if self.at(TokenKind::Connection) {
            self.advance();
            self.consume(TokenKind::Colon)?;
            let conn_token = self.consume(TokenKind::String)?;
            Some(self.parse_string_literal(conn_token))
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
        let name_token = self.consume(TokenKind::Ident)?;
        let name = self.text(name_token).to_string();

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
        let name_token = self.consume(TokenKind::Ident)?;
        let name = self.text(name_token).to_string();
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
        let token = self.consume(TokenKind::Ident)?;
        let text = self.text(token);
        Ok(match text {
            "Int" => ColumnType::Int,
            "String" => ColumnType::String,
            "Bool" => ColumnType::Bool,
            "Float" => ColumnType::Float,
            "DateTime" => ColumnType::DateTime,
            "Bytes" => ColumnType::Bytes,
            other => ColumnType::Reference(other.to_string()),
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
                let col_token = self.consume(TokenKind::Ident)?;
                let column = self.text(col_token).to_string();
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
        let name_token = self.consume(TokenKind::Ident)?;
        let name = self.text(name_token).to_string();

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
        let source_token = self.consume(TokenKind::Ident)?;
        let source = self.text(source_token).to_string();
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
            let name_token = self.consume(TokenKind::Ident)?;
            let name = self.text(name_token).to_string();
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
            let token = self.consume(TokenKind::Ident)?;
            idents.push(self.text(token).to_string());
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

        let first = self.consume(TokenKind::Ident)?;
        segments.push(self.text(first).to_string());

        while self.at(TokenKind::ColonColon) {
            self.advance();
            let seg = self.consume(TokenKind::Ident)?;
            segments.push(self.text(seg).to_string());
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
            let name_token = self.consume(TokenKind::Ident)?;
            let name = self.text(name_token).to_string();
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

        let name_token = self.consume(TokenKind::Ident)?;
        let name = self.text(name_token).to_string();

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
        let binding_token = self.consume(TokenKind::Ident)?;
        let binding = self.text(binding_token).to_string();
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
            let name_token = self.consume(TokenKind::Ident)?;
            let target = self.text(name_token).to_string();
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
                let field_token = self.consume(TokenKind::Ident)?;
                let field = self.text(field_token).to_string();
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
                let token = self.advance();
                let text = self.text(token);
                let value: i64 = text.parse().unwrap_or(0);
                Ok(Expr {
                    kind: ExprKind::Literal(Literal::Int(value)),
                    span: start,
                })
            }
            TokenKind::Float => {
                let token = self.advance();
                let text = self.text(token);
                let value: f64 = text.parse().unwrap_or(0.0);
                Ok(Expr {
                    kind: ExprKind::Literal(Literal::Float(value)),
                    span: start,
                })
            }
            TokenKind::String => {
                let token = self.advance();
                let value = self.parse_string_literal(token);
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
            let name_token = self.consume(TokenKind::Ident)?;
            let name = self.text(name_token).to_string();

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
            let name_token = self.consume(TokenKind::Ident)?;
            let name = self.text(name_token).to_string();

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
                        let name_token = self.consume(TokenKind::Ident)?;
                        let name = self.text(name_token).to_string();
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
            kind: ExprKind::Query { target, body },
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
                    let alias_token = self.consume(TokenKind::Ident)?;
                    Some(self.text(alias_token).to_string())
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
        let table_token = self.consume(TokenKind::Ident)?;
        let table = self.text(table_token).to_string();

        let alias = if self.at(TokenKind::As) {
            self.advance();
            let alias_token = self.consume(TokenKind::Ident)?;
            Some(self.text(alias_token).to_string())
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
        let table_token = self.consume(TokenKind::Ident)?;
        let table = self.text(table_token).to_string();
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

    // === Helpers ===

    fn parse_string_literal(&self, token: &Token) -> String {
        let text = self.text(token);
        // Remove quotes and handle escapes
        let inner = &text[1..text.len() - 1];
        inner
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\r", "\r")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    }
}
