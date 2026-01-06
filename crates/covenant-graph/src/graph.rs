//! Graph building utilities

use covenant_ast::*;
use covenant_checker::SymbolTable;
use crate::ReferenceGraph;

/// Visitor that builds the reference graph from an AST
pub struct GraphBuilder<'a> {
    pub graph: ReferenceGraph,
    pub symbols: &'a SymbolTable,
    current_function: Option<SymbolId>,
}

impl<'a> GraphBuilder<'a> {
    pub fn new(symbols: &'a SymbolTable) -> Self {
        Self {
            graph: ReferenceGraph::new(),
            symbols,
            current_function: None,
        }
    }

    pub fn build(mut self, program: &Program) -> ReferenceGraph {
        for decl in &program.declarations {
            self.visit_declaration(decl);
        }
        self.graph
    }

    fn visit_declaration(&mut self, decl: &Declaration) {
        match &decl.kind {
            DeclarationKind::Function(f) => {
                if let Some(symbol) = self.symbols.lookup(&f.name) {
                    self.current_function = Some(symbol.id);
                    self.visit_block(&f.body);
                    self.current_function = None;
                }
            }
            DeclarationKind::Module(m) => {
                for decl in &m.declarations {
                    self.visit_declaration(decl);
                }
            }
            _ => {}
        }
    }

    fn visit_block(&mut self, block: &Block) {
        for stmt in &block.statements {
            self.visit_statement(stmt);
        }
    }

    fn visit_statement(&mut self, stmt: &Statement) {
        match &stmt.kind {
            StatementKind::Let { value, .. } => {
                self.visit_expr(value);
            }
            StatementKind::Return(Some(expr)) => {
                self.visit_expr(expr);
            }
            StatementKind::Return(None) => {}
            StatementKind::Expr(expr) => {
                self.visit_expr(expr);
            }
            StatementKind::For { iterable, body, .. } => {
                self.visit_expr(iterable);
                self.visit_block(body);
            }
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                // Record the call
                if let ExprKind::Ident(name) = &callee.kind {
                    if let Some(caller) = self.current_function {
                        if let Some(callee_symbol) = self.symbols.lookup(name) {
                            self.graph.add_call(caller, callee_symbol.id);
                        }
                    }
                }
                self.visit_expr(callee);
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            ExprKind::Unary { operand, .. } => {
                self.visit_expr(operand);
            }
            ExprKind::Field { object, .. } => {
                self.visit_expr(object);
            }
            ExprKind::Index { object, index } => {
                self.visit_expr(object);
                self.visit_expr(index);
            }
            ExprKind::Array(elements) => {
                for elem in elements {
                    self.visit_expr(elem);
                }
            }
            ExprKind::Struct { fields, .. } => {
                for field in fields {
                    if let Some(ref value) = field.value {
                        self.visit_expr(value);
                    }
                }
            }
            ExprKind::Block(block) => {
                self.visit_block(block);
            }
            ExprKind::If { condition, then_branch, else_branch } => {
                self.visit_expr(condition);
                self.visit_block(then_branch);
                if let Some(else_expr) = else_branch {
                    self.visit_expr(else_expr);
                }
            }
            ExprKind::Match { scrutinee, arms } => {
                self.visit_expr(scrutinee);
                for arm in arms {
                    self.visit_expr(&arm.body);
                }
            }
            ExprKind::Closure { body, .. } => {
                self.visit_expr(body);
            }
            ExprKind::Handle { expr, arms } => {
                self.visit_expr(expr);
                for arm in arms {
                    self.visit_expr(&arm.body);
                }
            }
            ExprKind::Query { body, .. } => {
                if let Some(ref where_clause) = body.where_clause {
                    self.visit_expr(where_clause);
                }
            }
            ExprKind::Assign { value, .. } => {
                self.visit_expr(value);
            }
            ExprKind::Insert { value, .. } => {
                self.visit_expr(value);
            }
            ExprKind::Update { condition, .. } => {
                if let Some(ref cond) = condition {
                    self.visit_expr(cond);
                }
            }
            ExprKind::Delete { condition, .. } => {
                if let Some(ref cond) = condition {
                    self.visit_expr(cond);
                }
            }
            _ => {}
        }
    }
}
