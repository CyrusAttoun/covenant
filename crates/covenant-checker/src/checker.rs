//! Main type checker implementation

use covenant_ast::*;
use crate::{CheckError, CheckResult, SymbolTable, EffectTable, ResolvedType, SymbolKind};

pub struct Checker {
    pub symbols: SymbolTable,
    pub effects: EffectTable,
    errors: Vec<CheckError>,
}

impl Checker {
    pub fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            effects: EffectTable::new(),
            errors: Vec::new(),
        }
    }

    /// Check legacy declarations (for Program::Legacy)
    pub fn check_declarations(&mut self, declarations: &[Declaration]) -> Result<CheckResult, Vec<CheckError>> {
        // First pass: collect all type and function declarations
        for decl in declarations {
            self.collect_declaration(decl);
        }

        // Second pass: type check function bodies
        for decl in declarations {
            self.check_declaration_ref(decl);
        }

        if self.errors.is_empty() {
            Ok(CheckResult {
                symbols: std::mem::take(&mut self.symbols),
                effects: std::mem::take(&mut self.effects),
            })
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    fn check_declaration_ref(&mut self, decl: &Declaration) {
        match &decl.kind {
            DeclarationKind::Function(f) => {
                self.check_function_ref(f);
            }
            _ => {}
        }
    }

    fn check_function_ref(&mut self, func: &FunctionDecl) {
        self.symbols.enter_scope();

        // Add parameters to scope
        for param in &func.params {
            self.symbols.define(
                param.name.clone(),
                SymbolKind::Parameter,
                self.resolve_type(&param.ty),
            );
        }

        // Add imported symbols to scope
        for import in &func.imports {
            for name in &import.names {
                self.symbols.define(
                    name.clone(),
                    SymbolKind::Function {
                        params: vec![],
                        effects: vec![import.source.clone()],
                    },
                    ResolvedType::Unknown,
                );
            }
        }

        // Check body
        for stmt in &func.body.statements {
            self.check_statement(stmt);
        }

        self.symbols.exit_scope();
    }

    fn collect_declaration(&mut self, decl: &Declaration) {
        match &decl.kind {
            DeclarationKind::Struct(s) => {
                self.symbols.define(
                    s.name.clone(),
                    SymbolKind::Type,
                    ResolvedType::Named {
                        name: s.name.clone(),
                        id: SymbolId(0), // Will be fixed up
                        args: vec![],
                    },
                );
            }
            DeclarationKind::Enum(e) => {
                self.symbols.define(
                    e.name.clone(),
                    SymbolKind::Type,
                    ResolvedType::Named {
                        name: e.name.clone(),
                        id: SymbolId(0),
                        args: vec![],
                    },
                );
            }
            DeclarationKind::Function(f) => {
                // Register effects from imports
                let effects: Vec<String> = f
                    .imports
                    .iter()
                    .map(|i| i.source.clone())
                    .collect();

                let ret_type = f
                    .return_type
                    .as_ref()
                    .map(|t| self.resolve_type(t))
                    .unwrap_or(ResolvedType::Tuple(vec![]));

                let params: Vec<(String, ResolvedType)> = f
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), self.resolve_type(&p.ty)))
                    .collect();

                let id = self.symbols.define(
                    f.name.clone(),
                    SymbolKind::Function {
                        params: params.clone(),
                        effects: effects.clone(),
                    },
                    ResolvedType::Function {
                        params: params.iter().map(|(_, t)| t.clone()).collect(),
                        ret: Box::new(ret_type),
                    },
                );

                // Register effects
                for effect_name in effects {
                    let effect_id = self.effects.register(effect_name.clone(), effect_name);
                    self.effects.add_effect(id, effect_id);
                }
            }
            DeclarationKind::TypeAlias(t) => {
                self.symbols.define(
                    t.name.clone(),
                    SymbolKind::Type,
                    self.resolve_type(&t.ty),
                );
            }
            DeclarationKind::Extern(e) => {
                let ret_type = self.resolve_type(&e.return_type);
                let params: Vec<(String, ResolvedType)> = e
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), self.resolve_type(&p.ty)))
                    .collect();

                let id = self.symbols.define(
                    e.name.clone(),
                    SymbolKind::Function {
                        params: params.clone(),
                        effects: e.effects.clone(),
                    },
                    ResolvedType::Function {
                        params: params.iter().map(|(_, t)| t.clone()).collect(),
                        ret: Box::new(ret_type),
                    },
                );

                // Register effects
                for effect_name in &e.effects {
                    let effect_id = self.effects.register(effect_name.clone(), e.source.clone());
                    self.effects.add_effect(id, effect_id);
                }
            }
            DeclarationKind::Database(d) => {
                self.symbols.define(
                    d.name.clone(),
                    SymbolKind::Type,
                    ResolvedType::Named {
                        name: d.name.clone(),
                        id: SymbolId(0),
                        args: vec![],
                    },
                );
            }
            DeclarationKind::Import(_) | DeclarationKind::Module(_) => {
                // Handle in a separate pass if needed
            }
        }
    }

    fn check_statement(&mut self, stmt: &Statement) {
        match &stmt.kind {
            StatementKind::Let { name, ty, value, mutable } => {
                let value_type = self.infer_expr(value);
                let declared_type = ty.as_ref().map(|t| self.resolve_type(t));

                if let Some(ref decl_ty) = declared_type {
                    if !self.types_compatible(decl_ty, &value_type) {
                        self.errors.push(CheckError::TypeMismatch {
                            expected: decl_ty.display(),
                            found: value_type.display(),
                        });
                    }
                }

                let final_type = declared_type.unwrap_or(value_type);
                self.symbols.define(
                    name.clone(),
                    SymbolKind::Variable { mutable: *mutable },
                    final_type,
                );
            }
            StatementKind::Return(expr) => {
                if let Some(e) = expr {
                    self.infer_expr(e);
                }
            }
            StatementKind::Expr(expr) => {
                self.infer_expr(expr);
            }
            StatementKind::For { binding, iterable, body } => {
                let iter_type = self.infer_expr(iterable);
                let elem_type = match iter_type {
                    ResolvedType::List(inner) => *inner,
                    _ => ResolvedType::Unknown,
                };

                self.symbols.enter_scope();
                self.symbols.define(
                    binding.clone(),
                    SymbolKind::Variable { mutable: false },
                    elem_type,
                );

                for stmt in &body.statements {
                    self.check_statement(stmt);
                }

                self.symbols.exit_scope();
            }
        }
    }

    fn infer_expr(&mut self, expr: &Expr) -> ResolvedType {
        match &expr.kind {
            ExprKind::Literal(lit) => match lit {
                Literal::Int(_) => ResolvedType::Int,
                Literal::Float(_) => ResolvedType::Float,
                Literal::String(_) => ResolvedType::String,
                Literal::Bool(_) => ResolvedType::Bool,
                Literal::None => ResolvedType::None,
            },
            ExprKind::Ident(name) => {
                if let Some(symbol) = self.symbols.lookup(name) {
                    symbol.ty.clone()
                } else {
                    self.errors.push(CheckError::UndefinedSymbol {
                        name: name.clone(),
                    });
                    ResolvedType::Error
                }
            }
            ExprKind::Binary { op, left, right } => {
                let left_ty = self.infer_expr(left);
                let right_ty = self.infer_expr(right);

                match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                        if matches!(left_ty, ResolvedType::Int) && matches!(right_ty, ResolvedType::Int) {
                            ResolvedType::Int
                        } else if matches!(left_ty, ResolvedType::Float | ResolvedType::Int)
                            && matches!(right_ty, ResolvedType::Float | ResolvedType::Int)
                        {
                            ResolvedType::Float
                        } else {
                            ResolvedType::Error
                        }
                    }
                    BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge | BinaryOp::Contains => {
                        ResolvedType::Bool
                    }
                    BinaryOp::And | BinaryOp::Or => ResolvedType::Bool,
                }
            }
            ExprKind::Unary { op, operand } => {
                let operand_ty = self.infer_expr(operand);
                match op {
                    UnaryOp::Neg => operand_ty,
                    UnaryOp::Not => ResolvedType::Bool,
                }
            }
            ExprKind::Call { callee, args } => {
                let callee_ty = self.infer_expr(callee);
                for arg in args {
                    self.infer_expr(arg);
                }

                match callee_ty {
                    ResolvedType::Function { ret, .. } => *ret,
                    _ => ResolvedType::Unknown,
                }
            }
            ExprKind::Field { object, field: _ } => {
                let _obj_ty = self.infer_expr(object);
                // TODO: look up field type
                ResolvedType::Unknown
            }
            ExprKind::Index { object, index } => {
                let obj_ty = self.infer_expr(object);
                self.infer_expr(index);

                match obj_ty {
                    ResolvedType::List(inner) => *inner,
                    _ => ResolvedType::Unknown,
                }
            }
            ExprKind::Array(elements) => {
                let elem_ty = elements
                    .first()
                    .map(|e| self.infer_expr(e))
                    .unwrap_or(ResolvedType::Unknown);
                ResolvedType::List(Box::new(elem_ty))
            }
            ExprKind::Struct { path, fields } => {
                for field in fields {
                    if let Some(ref value) = field.value {
                        self.infer_expr(value);
                    }
                }
                if let Some(p) = path {
                    ResolvedType::Named {
                        name: p.name().to_string(),
                        id: SymbolId(0),
                        args: vec![],
                    }
                } else {
                    ResolvedType::Unknown
                }
            }
            ExprKind::Block(block) => {
                self.symbols.enter_scope();
                let mut last_ty = ResolvedType::Tuple(vec![]);
                for stmt in &block.statements {
                    self.check_statement(stmt);
                    if let StatementKind::Expr(e) = &stmt.kind {
                        last_ty = self.infer_expr(e);
                    }
                }
                self.symbols.exit_scope();
                last_ty
            }
            ExprKind::If { condition, then_branch, else_branch } => {
                self.infer_expr(condition);
                self.symbols.enter_scope();
                for stmt in &then_branch.statements {
                    self.check_statement(stmt);
                }
                self.symbols.exit_scope();

                if let Some(else_expr) = else_branch {
                    self.infer_expr(else_expr);
                }

                ResolvedType::Unknown
            }
            ExprKind::Match { scrutinee, arms } => {
                self.infer_expr(scrutinee);
                for arm in arms {
                    self.infer_expr(&arm.body);
                }
                ResolvedType::Unknown
            }
            ExprKind::Query { target: _, body: _ } => {
                // Query expressions return a list of the queried type
                ResolvedType::Unknown
            }
            ExprKind::Assign { target: _, value } => {
                self.infer_expr(value);
                ResolvedType::Tuple(vec![])
            }
            ExprKind::Closure { params, body } => {
                self.symbols.enter_scope();
                for param in params {
                    let ty = param
                        .ty
                        .as_ref()
                        .map(|t| self.resolve_type(t))
                        .unwrap_or(ResolvedType::Unknown);
                    self.symbols.define(
                        param.name.clone(),
                        SymbolKind::Parameter,
                        ty,
                    );
                }
                let ret = self.infer_expr(body);
                self.symbols.exit_scope();

                ResolvedType::Function {
                    params: params
                        .iter()
                        .map(|p| {
                            p.ty.as_ref()
                                .map(|t| self.resolve_type(t))
                                .unwrap_or(ResolvedType::Unknown)
                        })
                        .collect(),
                    ret: Box::new(ret),
                }
            }
            ExprKind::Handle { expr, arms: _ } => {
                self.infer_expr(expr)
            }
            ExprKind::Insert { .. } | ExprKind::Update { .. } | ExprKind::Delete { .. } => {
                ResolvedType::Unknown
            }
        }
    }

    fn resolve_type(&self, ty: &Type) -> ResolvedType {
        match &ty.kind {
            TypeKind::Named(path) => {
                let name = path.name();
                match name {
                    "Int" => ResolvedType::Int,
                    "Float" => ResolvedType::Float,
                    "Bool" => ResolvedType::Bool,
                    "String" => ResolvedType::String,
                    _ => ResolvedType::Named {
                        name: name.to_string(),
                        id: SymbolId(0),
                        args: path.generics.iter().map(|t| self.resolve_type(t)).collect(),
                    },
                }
            }
            TypeKind::Optional(inner) => {
                ResolvedType::Optional(Box::new(self.resolve_type(inner)))
            }
            TypeKind::List(inner) => {
                ResolvedType::List(Box::new(self.resolve_type(inner)))
            }
            TypeKind::Union(types) => {
                ResolvedType::Union(types.iter().map(|t| self.resolve_type(t)).collect())
            }
            TypeKind::Tuple(types) => {
                ResolvedType::Tuple(types.iter().map(|t| self.resolve_type(t)).collect())
            }
            TypeKind::Function { params, ret } => ResolvedType::Function {
                params: params.iter().map(|t| self.resolve_type(t)).collect(),
                ret: Box::new(self.resolve_type(ret)),
            },
            TypeKind::Struct(fields) => ResolvedType::Struct(
                fields
                    .iter()
                    .map(|f| (f.name.clone(), self.resolve_type(&f.ty)))
                    .collect(),
            ),
        }
    }

    fn types_compatible(&self, expected: &ResolvedType, found: &ResolvedType) -> bool {
        match (expected, found) {
            (ResolvedType::Unknown, _) | (_, ResolvedType::Unknown) => true,
            (ResolvedType::Error, _) | (_, ResolvedType::Error) => true,
            (ResolvedType::Int, ResolvedType::Int) => true,
            (ResolvedType::Float, ResolvedType::Float) => true,
            (ResolvedType::Float, ResolvedType::Int) => true, // Int can be used as Float
            (ResolvedType::Bool, ResolvedType::Bool) => true,
            (ResolvedType::String, ResolvedType::String) => true,
            (ResolvedType::None, ResolvedType::None) => true,
            (ResolvedType::Optional(_inner), ResolvedType::None) => true,
            (ResolvedType::Optional(e), ResolvedType::Optional(f)) => self.types_compatible(e, f),
            (ResolvedType::Optional(e), f) => self.types_compatible(e, f),
            (ResolvedType::List(e), ResolvedType::List(f)) => self.types_compatible(e, f),
            (ResolvedType::Named { name: n1, .. }, ResolvedType::Named { name: n2, .. }) => n1 == n2,
            _ => false,
        }
    }
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}
