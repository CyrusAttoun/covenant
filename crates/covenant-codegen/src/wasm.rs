//! WASM code generation

use std::collections::HashMap;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection,
    Instruction, Module, TypeSection, ValType,
};
use covenant_ast::*;
use covenant_checker::{SymbolTable, SymbolKind, ResolvedType};
use crate::{CodegenError, IrType, IrBinOp, IrCmpOp};

/// WASM compiler
pub struct WasmCompiler<'a> {
    symbols: &'a SymbolTable,
    /// Function name to index mapping
    function_indices: HashMap<String, u32>,
    /// Local variable indices per function
    locals: HashMap<String, u32>,
    /// Current local count
    local_count: u32,
}

impl<'a> WasmCompiler<'a> {
    pub fn new(symbols: &'a SymbolTable) -> Self {
        Self {
            symbols,
            function_indices: HashMap::new(),
            locals: HashMap::new(),
            local_count: 0,
        }
    }

    /// Compile the entire program
    pub fn compile(&mut self, program: &Program) -> Result<Vec<u8>, CodegenError> {
        self.compile_pure_only(program)
    }

    /// Compile only pure functions
    pub fn compile_pure_only(&mut self, program: &Program) -> Result<Vec<u8>, CodegenError> {
        let mut module = Module::new();

        // Collect pure functions
        let pure_functions: Vec<&FunctionDecl> = program
            .declarations
            .iter()
            .filter_map(|d| {
                if let DeclarationKind::Function(f) = &d.kind {
                    if f.imports.is_empty() {
                        return Some(f);
                    }
                }
                None
            })
            .collect();

        if pure_functions.is_empty() {
            // Return minimal valid WASM module
            return Ok(module.finish());
        }

        // Build function index map
        for (i, func) in pure_functions.iter().enumerate() {
            self.function_indices.insert(func.name.clone(), i as u32);
        }

        // Type section
        let mut types = TypeSection::new();
        for func in &pure_functions {
            let params: Vec<ValType> = func
                .params
                .iter()
                .filter_map(|p| self.type_to_valtype(&p.ty))
                .collect();

            let results: Vec<ValType> = func
                .return_type
                .as_ref()
                .and_then(|t| self.type_to_valtype(t))
                .map(|t| vec![t])
                .unwrap_or_default();

            types.ty().function(params, results);
        }
        module.section(&types);

        // Function section
        let mut functions = FunctionSection::new();
        for i in 0..pure_functions.len() {
            functions.function(i as u32);
        }
        module.section(&functions);

        // Export section
        let mut exports = ExportSection::new();
        for (i, func) in pure_functions.iter().enumerate() {
            exports.export(&func.name, ExportKind::Func, i as u32);
        }
        module.section(&exports);

        // Code section
        let mut codes = CodeSection::new();
        for func in &pure_functions {
            let wasm_func = self.compile_function(func)?;
            codes.function(&wasm_func);
        }
        module.section(&codes);

        Ok(module.finish())
    }

    fn compile_function(&mut self, func: &FunctionDecl) -> Result<Function, CodegenError> {
        // Reset locals
        self.locals.clear();
        self.local_count = 0;

        // Add parameters as locals
        for param in &func.params {
            self.locals.insert(param.name.clone(), self.local_count);
            self.local_count += 1;
        }

        // Collect additional locals from let statements
        let additional_locals = self.count_locals(&func.body);

        let mut wasm_func = Function::new(vec![(additional_locals, ValType::I64)]);

        // Compile body
        self.compile_block(&func.body, &mut wasm_func)?;

        // Add implicit return if needed
        wasm_func.instruction(&Instruction::End);

        Ok(wasm_func)
    }

    fn count_locals(&self, block: &Block) -> u32 {
        let mut count = 0;
        for stmt in &block.statements {
            if let StatementKind::Let { .. } = &stmt.kind {
                count += 1;
            }
            // TODO: count nested blocks
        }
        count
    }

    fn compile_block(&mut self, block: &Block, func: &mut Function) -> Result<(), CodegenError> {
        for (i, stmt) in block.statements.iter().enumerate() {
            let is_last = i == block.statements.len() - 1;
            self.compile_statement(stmt, func, is_last)?;
        }
        Ok(())
    }

    fn compile_statement(
        &mut self,
        stmt: &Statement,
        func: &mut Function,
        is_last: bool,
    ) -> Result<(), CodegenError> {
        match &stmt.kind {
            StatementKind::Let { name, value, .. } => {
                // Allocate local
                let local_idx = self.local_count;
                self.locals.insert(name.clone(), local_idx);
                self.local_count += 1;

                // Compile value and store
                self.compile_expr(value, func)?;
                func.instruction(&Instruction::LocalSet(local_idx));
                Ok(())
            }
            StatementKind::Return(expr) => {
                if let Some(e) = expr {
                    self.compile_expr(e, func)?;
                }
                func.instruction(&Instruction::Return);
                Ok(())
            }
            StatementKind::Expr(expr) => {
                self.compile_expr(expr, func)?;
                // Drop result if not the last statement
                if !is_last {
                    func.instruction(&Instruction::Drop);
                }
                Ok(())
            }
            StatementKind::For { .. } => {
                // TODO: implement loops
                Err(CodegenError::UnsupportedExpression)
            }
        }
    }

    fn compile_expr(&mut self, expr: &Expr, func: &mut Function) -> Result<(), CodegenError> {
        match &expr.kind {
            ExprKind::Literal(lit) => match lit {
                Literal::Int(n) => {
                    func.instruction(&Instruction::I64Const(*n));
                    Ok(())
                }
                Literal::Float(n) => {
                    func.instruction(&Instruction::F64Const(*n));
                    Ok(())
                }
                Literal::Bool(b) => {
                    func.instruction(&Instruction::I32Const(if *b { 1 } else { 0 }));
                    Ok(())
                }
                Literal::None => {
                    // Represent none as 0
                    func.instruction(&Instruction::I64Const(0));
                    Ok(())
                }
                Literal::String(_) => {
                    // Strings not yet supported
                    Err(CodegenError::UnsupportedType {
                        ty: "String".to_string(),
                    })
                }
            },
            ExprKind::Ident(name) => {
                if let Some(&idx) = self.locals.get(name) {
                    func.instruction(&Instruction::LocalGet(idx));
                    Ok(())
                } else {
                    Err(CodegenError::UndefinedFunction {
                        name: name.clone(),
                    })
                }
            }
            ExprKind::Binary { op, left, right } => {
                self.compile_expr(left, func)?;
                self.compile_expr(right, func)?;

                // Emit appropriate instruction
                match op {
                    BinaryOp::Add => func.instruction(&Instruction::I64Add),
                    BinaryOp::Sub => func.instruction(&Instruction::I64Sub),
                    BinaryOp::Mul => func.instruction(&Instruction::I64Mul),
                    BinaryOp::Div => func.instruction(&Instruction::I64DivS),
                    BinaryOp::Mod => func.instruction(&Instruction::I64RemS),
                    BinaryOp::Eq => func.instruction(&Instruction::I64Eq),
                    BinaryOp::Ne => func.instruction(&Instruction::I64Ne),
                    BinaryOp::Lt => func.instruction(&Instruction::I64LtS),
                    BinaryOp::Le => func.instruction(&Instruction::I64LeS),
                    BinaryOp::Gt => func.instruction(&Instruction::I64GtS),
                    BinaryOp::Ge => func.instruction(&Instruction::I64GeS),
                    BinaryOp::And => func.instruction(&Instruction::I32And),
                    BinaryOp::Or => func.instruction(&Instruction::I32Or),
                    BinaryOp::Contains => {
                        return Err(CodegenError::UnsupportedExpression);
                    }
                };
                Ok(())
            }
            ExprKind::Unary { op, operand } => {
                self.compile_expr(operand, func)?;
                match op {
                    UnaryOp::Neg => {
                        // Negate: 0 - x
                        func.instruction(&Instruction::I64Const(0));
                        func.instruction(&Instruction::I64Sub);
                    }
                    UnaryOp::Not => {
                        // Logical not: x == 0
                        func.instruction(&Instruction::I64Eqz);
                    }
                }
                Ok(())
            }
            ExprKind::Call { callee, args } => {
                // Compile arguments
                for arg in args {
                    self.compile_expr(arg, func)?;
                }

                // Get function index
                if let ExprKind::Ident(name) = &callee.kind {
                    if let Some(&idx) = self.function_indices.get(name) {
                        func.instruction(&Instruction::Call(idx));
                        Ok(())
                    } else {
                        Err(CodegenError::UndefinedFunction {
                            name: name.clone(),
                        })
                    }
                } else {
                    Err(CodegenError::UnsupportedExpression)
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.compile_expr(condition, func)?;

                // If with result type
                func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
                self.compile_block(then_branch, func)?;

                if let Some(else_expr) = else_branch {
                    func.instruction(&Instruction::Else);
                    self.compile_expr(else_expr, func)?;
                }

                func.instruction(&Instruction::End);
                Ok(())
            }
            ExprKind::Block(block) => {
                self.compile_block(block, func)?;
                Ok(())
            }
            _ => Err(CodegenError::UnsupportedExpression),
        }
    }

    fn type_to_valtype(&self, ty: &Type) -> Option<ValType> {
        match &ty.kind {
            TypeKind::Named(path) => match path.name() {
                "Int" => Some(ValType::I64),
                "Float" => Some(ValType::F64),
                "Bool" => Some(ValType::I32),
                _ => None,
            },
            TypeKind::Optional(inner) => self.type_to_valtype(inner),
            _ => None,
        }
    }
}
