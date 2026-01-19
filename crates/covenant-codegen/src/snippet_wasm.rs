//! WASM code generation for snippet-mode programs
//!
//! Compiles pure function snippets to WebAssembly.

use std::collections::HashMap;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection,
    Instruction, Module, TypeSection, ValType,
};
use covenant_ast::{
    Snippet, SnippetKind, Section, SignatureKind,
    Step, StepKind, ComputeStep, Operation, InputSource, CallStep,
    ReturnStep, ReturnValue, IfStep, BindStep, BindSource,
    FunctionSignature, ReturnType, Type, TypeKind, Literal,
};
use covenant_checker::SymbolTable;
use crate::CodegenError;

/// WASM compiler for snippet-mode programs
pub struct SnippetWasmCompiler<'a> {
    #[allow(dead_code)]
    symbols: &'a SymbolTable,
    /// Function name to index mapping
    function_indices: HashMap<String, u32>,
    /// Local variable indices per function
    locals: HashMap<String, u32>,
    /// Current local count
    local_count: u32,
}

impl<'a> SnippetWasmCompiler<'a> {
    pub fn new(symbols: &'a SymbolTable) -> Self {
        Self {
            symbols,
            function_indices: HashMap::new(),
            locals: HashMap::new(),
            local_count: 0,
        }
    }

    /// Compile snippets to WASM
    pub fn compile_snippets(&mut self, snippets: &[Snippet]) -> Result<Vec<u8>, CodegenError> {
        let mut module = Module::new();

        // Filter to pure function snippets only (no effects section)
        let pure_functions: Vec<&Snippet> = snippets.iter()
            .filter(|s| s.kind == SnippetKind::Function && !has_effects(s))
            .collect();

        if pure_functions.is_empty() {
            // Return minimal valid WASM module
            return Ok(module.finish());
        }

        // Build function index map
        for (i, snippet) in pure_functions.iter().enumerate() {
            if let Some(sig) = find_function_signature(snippet) {
                self.function_indices.insert(sig.name.clone(), i as u32);
            }
        }

        // Type section
        let mut types = TypeSection::new();
        for snippet in &pure_functions {
            if let Some(sig) = find_function_signature(snippet) {
                let params: Vec<ValType> = sig.params.iter()
                    .filter_map(|p| self.type_to_valtype(&p.ty))
                    .collect();

                let results: Vec<ValType> = sig.returns.as_ref()
                    .and_then(|r| self.return_type_to_valtype(r))
                    .map(|t| vec![t])
                    .unwrap_or_default();

                types.function(params, results);
            }
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
        for (i, snippet) in pure_functions.iter().enumerate() {
            if let Some(sig) = find_function_signature(snippet) {
                exports.export(&sig.name, ExportKind::Func, i as u32);
            }
        }
        module.section(&exports);

        // Code section
        let mut codes = CodeSection::new();
        for snippet in &pure_functions {
            let wasm_func = self.compile_function_snippet(snippet)?;
            codes.function(&wasm_func);
        }
        module.section(&codes);

        Ok(module.finish())
    }

    /// Compile a single function snippet
    fn compile_function_snippet(&mut self, snippet: &Snippet) -> Result<Function, CodegenError> {
        let sig = find_function_signature(snippet)
            .ok_or_else(|| CodegenError::UndefinedFunction {
                name: snippet.id.clone(),
            })?;

        let body = find_body_section(snippet);

        // Reset locals
        self.locals.clear();
        self.local_count = 0;

        // Add parameters as locals
        for param in &sig.params {
            self.locals.insert(param.name.clone(), self.local_count);
            self.local_count += 1;
        }

        // Count additional locals needed from step bindings
        let additional_locals = if let Some(body) = body {
            self.count_step_bindings(&body.steps)
        } else {
            0
        };

        let mut wasm_func = Function::new(vec![(additional_locals, ValType::I64)]);

        // Compile body steps
        if let Some(body) = body {
            for step in &body.steps {
                self.compile_step(step, &mut wasm_func)?;
            }
        }

        // If function returns a value, we need something on the stack for the implicit return.
        // Push a dummy value (0) in case all paths returned early via explicit returns.
        // The unreachable instruction would be better but let's keep it simple.
        if sig.returns.is_some() {
            wasm_func.instruction(&Instruction::I64Const(0));
        }

        // Add end instruction
        wasm_func.instruction(&Instruction::End);

        Ok(wasm_func)
    }

    /// Count the number of step bindings that need locals
    fn count_step_bindings(&self, steps: &[Step]) -> u32 {
        let mut count = 0;
        for step in steps {
            if step.output_binding != "_" {
                count += 1;
            }
            // Count nested steps in if/match
            match &step.kind {
                StepKind::If(if_step) => {
                    count += self.count_step_bindings(&if_step.then_steps);
                    if let Some(else_steps) = &if_step.else_steps {
                        count += self.count_step_bindings(else_steps);
                    }
                }
                StepKind::Match(match_step) => {
                    for case in &match_step.cases {
                        count += self.count_step_bindings(&case.steps);
                    }
                }
                _ => {}
            }
        }
        count
    }

    /// Compile a single step
    fn compile_step(&mut self, step: &Step, func: &mut Function) -> Result<(), CodegenError> {
        match &step.kind {
            StepKind::Compute(compute) => {
                self.compile_compute_step(compute, func)?;
                // Store result if not discarded
                if step.output_binding != "_" {
                    let local = self.allocate_local(&step.output_binding);
                    func.instruction(&Instruction::LocalSet(local));
                }
            }
            StepKind::Call(call) => {
                self.compile_call_step(call, func)?;
                // Store result if not discarded
                if step.output_binding != "_" {
                    let local = self.allocate_local(&step.output_binding);
                    func.instruction(&Instruction::LocalSet(local));
                }
            }
            StepKind::Return(ret) => {
                self.compile_return_step(ret, func)?;
                func.instruction(&Instruction::Return);
            }
            StepKind::If(if_step) => {
                self.compile_if_step(if_step, func)?;
            }
            StepKind::Bind(bind) => {
                self.compile_bind_step(bind, func)?;
                // Store result if not discarded
                if step.output_binding != "_" {
                    let local = self.allocate_local(&step.output_binding);
                    func.instruction(&Instruction::LocalSet(local));
                }
            }
            StepKind::Match(_) => {
                // TODO: Implement match compilation
                return Err(CodegenError::UnsupportedExpression);
            }
            StepKind::For(_) | StepKind::Query(_) | StepKind::Insert(_) |
            StepKind::Update(_) | StepKind::Delete(_) | StepKind::Transaction(_) |
            StepKind::Traverse(_) => {
                // These require effects, shouldn't appear in pure functions
                return Err(CodegenError::UnsupportedExpression);
            }
        }
        Ok(())
    }

    /// Compile a compute step
    fn compile_compute_step(&mut self, compute: &ComputeStep, func: &mut Function) -> Result<(), CodegenError> {
        // Push inputs onto stack
        for input in &compute.inputs {
            self.compile_input(&input.source, func)?;
        }

        // Emit operation instruction
        // Note: Comparison operations return i32, we extend to i64 for uniform storage
        match compute.op {
            Operation::Add => { func.instruction(&Instruction::I64Add); }
            Operation::Sub => { func.instruction(&Instruction::I64Sub); }
            Operation::Mul => { func.instruction(&Instruction::I64Mul); }
            Operation::Div => { func.instruction(&Instruction::I64DivS); }
            Operation::Mod => { func.instruction(&Instruction::I64RemS); }
            // Comparison ops return i32, extend to i64
            Operation::Equals => {
                func.instruction(&Instruction::I64Eq);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            Operation::NotEquals => {
                func.instruction(&Instruction::I64Ne);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            Operation::Less => {
                func.instruction(&Instruction::I64LtS);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            Operation::Greater => {
                func.instruction(&Instruction::I64GtS);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            Operation::LessEq => {
                func.instruction(&Instruction::I64LeS);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            Operation::GreaterEq => {
                func.instruction(&Instruction::I64GeS);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            // Boolean ops: we treat bools as i64 (0 or 1)
            Operation::And => {
                func.instruction(&Instruction::I64And);
            }
            Operation::Or => {
                func.instruction(&Instruction::I64Or);
            }
            Operation::Not => {
                // Not: check if value is zero, result is i32, extend to i64
                func.instruction(&Instruction::I64Eqz);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            Operation::Neg => {
                // Negate: compute 0 - x
                // First swap the operand with 0
                func.instruction(&Instruction::I64Const(0));
                func.instruction(&Instruction::I64Sub);
            }
            Operation::Concat | Operation::Contains => {
                return Err(CodegenError::UnsupportedType { ty: "String".to_string() });
            }
            // All other operations are not yet supported in WASM codegen
            _ => {
                return Err(CodegenError::UnsupportedExpression);
            }
        }

        Ok(())
    }

    /// Compile a call step
    fn compile_call_step(&mut self, call: &CallStep, func: &mut Function) -> Result<(), CodegenError> {
        // Push arguments onto stack
        for arg in &call.args {
            self.compile_input(&arg.source, func)?;
        }

        // Get function index
        let idx = self.function_indices.get(&call.fn_name)
            .ok_or_else(|| CodegenError::UndefinedFunction { name: call.fn_name.clone() })?;

        func.instruction(&Instruction::Call(*idx));
        Ok(())
    }

    /// Compile a return step
    fn compile_return_step(&mut self, ret: &ReturnStep, func: &mut Function) -> Result<(), CodegenError> {
        match &ret.value {
            ReturnValue::Var(name) => {
                let local = self.locals.get(name)
                    .ok_or_else(|| CodegenError::UndefinedFunction { name: name.clone() })?;
                func.instruction(&Instruction::LocalGet(*local));
            }
            ReturnValue::Lit(lit) => {
                self.compile_literal(lit, func)?;
            }
            ReturnValue::Struct(_) | ReturnValue::Variant(_) => {
                // Structs and variants not yet supported in WASM
                return Err(CodegenError::UnsupportedExpression);
            }
        }
        Ok(())
    }

    /// Compile an if step
    fn compile_if_step(&mut self, if_step: &IfStep, func: &mut Function) -> Result<(), CodegenError> {
        // Load condition variable
        let cond_local = self.locals.get(&if_step.condition)
            .ok_or_else(|| CodegenError::UndefinedFunction { name: if_step.condition.clone() })?;
        func.instruction(&Instruction::LocalGet(*cond_local));

        // Wrap i64 to i32 for the if condition (if instruction expects i32)
        func.instruction(&Instruction::I32WrapI64);

        // Emit if block
        func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));

        // Compile then steps
        for step in &if_step.then_steps {
            self.compile_step(step, func)?;
        }

        // Compile else steps if present
        if let Some(else_steps) = &if_step.else_steps {
            func.instruction(&Instruction::Else);
            for step in else_steps {
                self.compile_step(step, func)?;
            }
        }

        func.instruction(&Instruction::End);
        Ok(())
    }

    /// Compile a bind step
    fn compile_bind_step(&mut self, bind: &BindStep, func: &mut Function) -> Result<(), CodegenError> {
        match &bind.source {
            BindSource::Var(name) => {
                let local = self.locals.get(name)
                    .ok_or_else(|| CodegenError::UndefinedFunction { name: name.clone() })?;
                func.instruction(&Instruction::LocalGet(*local));
            }
            BindSource::Lit(lit) => {
                self.compile_literal(lit, func)?;
            }
            BindSource::Field { .. } => {
                // Field access not yet supported
                return Err(CodegenError::UnsupportedExpression);
            }
        }
        Ok(())
    }

    /// Compile an input source
    fn compile_input(&mut self, source: &InputSource, func: &mut Function) -> Result<(), CodegenError> {
        match source {
            InputSource::Var(name) => {
                let local = self.locals.get(name)
                    .ok_or_else(|| CodegenError::UndefinedFunction { name: name.clone() })?;
                func.instruction(&Instruction::LocalGet(*local));
            }
            InputSource::Lit(lit) => {
                self.compile_literal(lit, func)?;
            }
            InputSource::Field { .. } => {
                // Field access not yet supported
                return Err(CodegenError::UnsupportedExpression);
            }
        }
        Ok(())
    }

    /// Compile a literal value
    fn compile_literal(&self, lit: &Literal, func: &mut Function) -> Result<(), CodegenError> {
        match lit {
            Literal::Int(n) => {
                func.instruction(&Instruction::I64Const(*n));
            }
            Literal::Float(n) => {
                func.instruction(&Instruction::F64Const(*n));
            }
            Literal::Bool(b) => {
                func.instruction(&Instruction::I32Const(if *b { 1 } else { 0 }));
            }
            Literal::None => {
                // Represent none as 0
                func.instruction(&Instruction::I64Const(0));
            }
            Literal::String(_) => {
                return Err(CodegenError::UnsupportedType { ty: "String".to_string() });
            }
        }
        Ok(())
    }

    /// Allocate a local variable
    fn allocate_local(&mut self, name: &str) -> u32 {
        if let Some(&idx) = self.locals.get(name) {
            return idx;
        }
        let idx = self.local_count;
        self.locals.insert(name.to_string(), idx);
        self.local_count += 1;
        idx
    }

    /// Convert an AST type to a WASM ValType
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

    /// Convert a return type to a WASM ValType
    fn return_type_to_valtype(&self, ret: &ReturnType) -> Option<ValType> {
        match ret {
            ReturnType::Single { ty, .. } => self.type_to_valtype(ty),
            ReturnType::Collection { of } => self.type_to_valtype(of),
            ReturnType::Union { types } => {
                // For unions, use the first type's representation
                types.first().and_then(|m| self.type_to_valtype(&m.ty))
            }
        }
    }
}

// Helper functions

/// Check if a snippet has effects
fn has_effects(snippet: &Snippet) -> bool {
    snippet.sections.iter().any(|s| matches!(s, Section::Effects(_)))
}

/// Find the function signature in a snippet
fn find_function_signature(snippet: &Snippet) -> Option<&FunctionSignature> {
    for section in &snippet.sections {
        if let Section::Signature(sig) = section {
            if let SignatureKind::Function(fn_sig) = &sig.kind {
                return Some(fn_sig);
            }
        }
    }
    None
}

/// Find the body section in a snippet
fn find_body_section(snippet: &Snippet) -> Option<&covenant_ast::BodySection> {
    for section in &snippet.sections {
        if let Section::Body(body) = section {
            return Some(body);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiler_creation() {
        let symbols = SymbolTable::new();
        let compiler = SnippetWasmCompiler::new(&symbols);
        assert!(compiler.function_indices.is_empty());
    }
}
