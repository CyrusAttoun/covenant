//! Snippet mode type checker
//!
//! Type checks snippets with IR-based step syntax (the primary Covenant format).

use std::collections::HashMap;
use covenant_ast::{
    Snippet, SnippetKind, Section, SignatureKind, BodySection,
    Step, StepKind, ComputeStep, Operation, Input, InputSource, CallStep,
    ReturnStep, ReturnValue, IfStep, BindStep, BindSource, MatchStep, MatchPattern,
    FunctionSignature, ReturnType, Type, TypeKind, Literal,
};
use crate::{CheckError, CheckResult, ResolvedType, SymbolTable, SymbolKind, EffectTable};

/// Checker for snippet-mode programs
pub struct SnippetChecker {
    symbols: SymbolTable,
    effects: EffectTable,
    errors: Vec<CheckError>,
    /// Local scope for current function body
    locals: HashMap<String, ResolvedType>,
    /// Map of function names to their return types (for recursive calls)
    function_returns: HashMap<String, ResolvedType>,
}

impl SnippetChecker {
    pub fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            effects: EffectTable::new(),
            errors: Vec::new(),
            locals: HashMap::new(),
            function_returns: HashMap::new(),
        }
    }

    /// Check all snippets and return the result
    pub fn check_snippets(mut self, snippets: &[Snippet]) -> Result<CheckResult, Vec<CheckError>> {
        // First pass: register all function signatures
        for snippet in snippets {
            if snippet.kind == SnippetKind::Function {
                self.register_function_signature(snippet);
            }
        }

        // Second pass: type check bodies
        for snippet in snippets {
            match snippet.kind {
                SnippetKind::Function => self.check_function_snippet(snippet),
                SnippetKind::Struct => self.check_struct_snippet(snippet),
                SnippetKind::Enum => self.check_enum_snippet(snippet),
                _ => {} // Skip other kinds for now
            }
        }

        if self.errors.is_empty() {
            Ok(CheckResult {
                symbols: self.symbols,
                effects: self.effects,
            })
        } else {
            Err(self.errors)
        }
    }

    /// Register a function's signature (first pass)
    fn register_function_signature(&mut self, snippet: &Snippet) {
        // Extract signature info without holding references
        let (fn_name, snippet_id, params, return_type, effects) = {
            let sig = match find_function_signature(snippet) {
                Some(s) => s,
                None => return,
            };

            let return_type = sig.returns.as_ref()
                .map(|r| self.resolve_return_type(r))
                .unwrap_or(ResolvedType::None);

            let params: Vec<(String, ResolvedType)> = sig.params.iter()
                .map(|p| (p.name.clone(), self.resolve_type(&p.ty)))
                .collect();

            let effects = collect_snippet_effects(snippet);

            (sig.name.clone(), snippet.id.clone(), params, return_type, effects)
        };

        // Register by short name (for backwards compatibility)
        self.symbols.define(
            fn_name.clone(),
            SymbolKind::Function { params: params.clone(), effects: effects.clone() },
            return_type.clone(),
        );
        self.function_returns.insert(fn_name.clone(), return_type.clone());

        // Also register by snippet ID (fully-qualified name) for canonical call syntax
        if snippet_id != fn_name {
            self.symbols.define(
                snippet_id.clone(),
                SymbolKind::Function { params, effects },
                return_type.clone(),
            );
            self.function_returns.insert(snippet_id, return_type);
        }
    }

    /// Check a function snippet
    fn check_function_snippet(&mut self, snippet: &Snippet) {
        // Extract what we need without holding references
        let (params_info, steps_cloned) = {
            let sig = match find_function_signature(snippet) {
                Some(s) => s,
                None => {
                    self.errors.push(CheckError::UndefinedSymbol {
                        name: format!("missing signature for {}", snippet.id),
                    });
                    return;
                }
            };

            let body = match find_body_section(snippet) {
                Some(b) => b,
                None => return, // External functions may not have bodies
            };

            let params_info: Vec<(String, ResolvedType)> = sig.params.iter()
                .map(|p| (p.name.clone(), self.resolve_type(&p.ty)))
                .collect();

            (params_info, body.steps.clone())
        };

        // Set up local scope with parameters
        self.locals.clear();
        for (name, ty) in params_info {
            self.locals.insert(name, ty);
        }

        // Check each step
        for step in &steps_cloned {
            self.check_step(step);
        }
    }

    /// Check a single step and add its binding to locals
    fn check_step(&mut self, step: &Step) {
        let step_type = self.infer_step_type(step);

        // Add binding to locals if not discarded
        if step.output_binding != "_" {
            self.locals.insert(step.output_binding.clone(), step_type);
        }
    }

    /// Infer the type of a step
    fn infer_step_type(&mut self, step: &Step) -> ResolvedType {
        match &step.kind {
            StepKind::Compute(compute) => self.infer_compute_step(compute),
            StepKind::Call(call) => self.infer_call_step(call),
            StepKind::Return(ret) => self.infer_return_step(ret),
            StepKind::If(if_step) => self.infer_if_step(if_step),
            StepKind::Bind(bind) => self.infer_bind_step(bind),
            StepKind::Match(match_step) => self.infer_match_step(match_step),
            StepKind::For(_) => ResolvedType::None, // For loops don't produce a value
            StepKind::Query(_) => ResolvedType::Unknown, // Would need query result type
            StepKind::Insert(_) => ResolvedType::Unknown,
            StepKind::Update(_) => ResolvedType::Unknown,
            StepKind::Delete(_) => ResolvedType::None,
            StepKind::Transaction(_) => ResolvedType::Unknown,
            StepKind::Traverse(_) => ResolvedType::Unknown,
        }
    }

    /// Infer type of a compute step
    fn infer_compute_step(&mut self, compute: &ComputeStep) -> ResolvedType {
        // First, resolve all input types
        let input_types: Vec<ResolvedType> = compute.inputs.iter()
            .map(|i| self.resolve_input_type(i))
            .collect();

        match compute.op {
            // Arithmetic operations: return numeric type
            Operation::Add | Operation::Sub | Operation::Mul | Operation::Div | Operation::Mod => {
                // If any input is Float, result is Float; otherwise Int
                if input_types.iter().any(|t| matches!(t, ResolvedType::Float)) {
                    ResolvedType::Float
                } else {
                    ResolvedType::Int
                }
            }

            // Comparison operations: return Bool
            Operation::Equals | Operation::NotEquals |
            Operation::Less | Operation::Greater |
            Operation::LessEq | Operation::GreaterEq => ResolvedType::Bool,

            // Boolean operations: return Bool
            Operation::And | Operation::Or | Operation::Not => ResolvedType::Bool,

            // Negation: same as input type
            Operation::Neg => input_types.first().cloned().unwrap_or(ResolvedType::Int),

            // String operations
            Operation::Concat => ResolvedType::String,
            Operation::Contains => ResolvedType::Bool,
        }
    }

    /// Infer type of a call step
    fn infer_call_step(&mut self, call: &CallStep) -> ResolvedType {
        // Look up function return type
        if let Some(return_type) = self.function_returns.get(&call.fn_name) {
            return_type.clone()
        } else if let Some(symbol) = self.symbols.lookup(&call.fn_name) {
            symbol.ty.clone()
        } else {
            self.errors.push(CheckError::UndefinedSymbol {
                name: call.fn_name.clone(),
            });
            ResolvedType::Error
        }
    }

    /// Infer type of a return step
    fn infer_return_step(&mut self, ret: &ReturnStep) -> ResolvedType {
        match &ret.value {
            ReturnValue::Var(name) => {
                self.locals.get(name).cloned().unwrap_or_else(|| {
                    self.errors.push(CheckError::UndefinedSymbol { name: name.clone() });
                    ResolvedType::Error
                })
            }
            ReturnValue::Lit(lit) => self.literal_type(lit),
            ReturnValue::Struct(s) => {
                // Return the struct type
                self.resolve_type(&s.ty)
            }
            ReturnValue::Variant(v) => {
                // Return a named type for the variant
                ResolvedType::Named {
                    name: v.ty.clone(),
                    id: covenant_ast::SymbolId(0), // Placeholder
                    args: vec![],
                }
            }
        }
    }

    /// Infer type of an if step
    fn infer_if_step(&mut self, if_step: &IfStep) -> ResolvedType {
        // Check condition exists and is bool
        if let Some(cond_type) = self.locals.get(&if_step.condition) {
            if !matches!(cond_type, ResolvedType::Bool) {
                self.errors.push(CheckError::TypeMismatch {
                    expected: "Bool".to_string(),
                    found: cond_type.display(),
                });
            }
        } else {
            self.errors.push(CheckError::UndefinedSymbol {
                name: if_step.condition.clone(),
            });
        }

        // Clone steps to avoid borrow issues
        let then_steps = if_step.then_steps.clone();
        let else_steps = if_step.else_steps.clone();

        // Check then branch
        for step in &then_steps {
            self.check_step(step);
        }

        // Check else branch if present
        if let Some(else_steps) = &else_steps {
            for step in else_steps {
                self.check_step(step);
            }
        }

        // If statements don't produce a value in SSA form
        ResolvedType::None
    }

    /// Infer type of a bind step
    fn infer_bind_step(&mut self, bind: &BindStep) -> ResolvedType {
        match &bind.source {
            BindSource::Var(name) => {
                self.locals.get(name).cloned().unwrap_or_else(|| {
                    self.errors.push(CheckError::UndefinedSymbol { name: name.clone() });
                    ResolvedType::Error
                })
            }
            BindSource::Lit(lit) => self.literal_type(lit),
            BindSource::Field { of, field } => {
                // Look up the struct type and field
                if let Some(struct_type) = self.locals.get(of) {
                    if let ResolvedType::Struct(fields) = struct_type {
                        fields.iter()
                            .find(|(name, _)| name == field)
                            .map(|(_, ty)| ty.clone())
                            .unwrap_or(ResolvedType::Unknown)
                    } else {
                        ResolvedType::Unknown
                    }
                } else {
                    self.errors.push(CheckError::UndefinedSymbol { name: of.clone() });
                    ResolvedType::Error
                }
            }
        }
    }

    /// Infer type of a match step
    fn infer_match_step(&mut self, match_step: &MatchStep) -> ResolvedType {
        // Check the match target exists
        if !self.locals.contains_key(&match_step.on) {
            self.errors.push(CheckError::UndefinedSymbol {
                name: match_step.on.clone(),
            });
        }

        // Clone cases to avoid borrow issues
        let cases = match_step.cases.clone();

        // Check each case
        for case in &cases {
            // Add pattern bindings to scope
            if let MatchPattern::Variant { bindings, .. } = &case.pattern {
                for binding in bindings {
                    self.locals.insert(binding.clone(), ResolvedType::Unknown);
                }
            }

            // Check case steps
            for step in &case.steps {
                self.check_step(step);
            }
        }

        ResolvedType::None
    }

    /// Resolve an input's type
    fn resolve_input_type(&self, input: &Input) -> ResolvedType {
        match &input.source {
            InputSource::Var(name) => {
                self.locals.get(name).cloned().unwrap_or(ResolvedType::Unknown)
            }
            InputSource::Lit(lit) => self.literal_type(lit),
            InputSource::Field { of, field } => {
                if let Some(struct_type) = self.locals.get(of) {
                    if let ResolvedType::Struct(fields) = struct_type {
                        fields.iter()
                            .find(|(name, _)| name == field)
                            .map(|(_, ty)| ty.clone())
                            .unwrap_or(ResolvedType::Unknown)
                    } else {
                        ResolvedType::Unknown
                    }
                } else {
                    ResolvedType::Unknown
                }
            }
        }
    }

    /// Get the type of a literal
    fn literal_type(&self, lit: &Literal) -> ResolvedType {
        match lit {
            Literal::Int(_) => ResolvedType::Int,
            Literal::Float(_) => ResolvedType::Float,
            Literal::Bool(_) => ResolvedType::Bool,
            Literal::String(_) => ResolvedType::String,
            Literal::None => ResolvedType::None,
        }
    }

    /// Resolve an AST Type to a ResolvedType
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
                        id: covenant_ast::SymbolId(0), // Placeholder
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
            TypeKind::Function { params, ret } => {
                ResolvedType::Function {
                    params: params.iter().map(|t| self.resolve_type(t)).collect(),
                    ret: Box::new(self.resolve_type(ret)),
                }
            }
            TypeKind::Struct(fields) => {
                ResolvedType::Struct(
                    fields.iter()
                        .map(|f| (f.name.clone(), self.resolve_type(&f.ty)))
                        .collect()
                )
            }
        }
    }

    /// Resolve a ReturnType to ResolvedType
    fn resolve_return_type(&self, ret: &ReturnType) -> ResolvedType {
        match ret {
            ReturnType::Single { ty, optional } => {
                let resolved = self.resolve_type(ty);
                if *optional {
                    ResolvedType::Optional(Box::new(resolved))
                } else {
                    resolved
                }
            }
            ReturnType::Collection { of } => {
                ResolvedType::List(Box::new(self.resolve_type(of)))
            }
            ReturnType::Union { types } => {
                ResolvedType::Union(
                    types.iter()
                        .map(|m| {
                            let resolved = self.resolve_type(&m.ty);
                            if m.optional {
                                ResolvedType::Optional(Box::new(resolved))
                            } else {
                                resolved
                            }
                        })
                        .collect()
                )
            }
        }
    }

    /// Check a struct snippet (just register it for now)
    fn check_struct_snippet(&mut self, _snippet: &Snippet) {
        // TODO: Register struct type
    }

    /// Check an enum snippet (just register it for now)
    fn check_enum_snippet(&mut self, _snippet: &Snippet) {
        // TODO: Register enum type
    }
}

impl Default for SnippetChecker {
    fn default() -> Self {
        Self::new()
    }
}

// Free functions to avoid borrow issues

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
fn find_body_section(snippet: &Snippet) -> Option<&BodySection> {
    for section in &snippet.sections {
        if let Section::Body(body) = section {
            return Some(body);
        }
    }
    None
}

/// Collect effect names from a snippet
fn collect_snippet_effects(snippet: &Snippet) -> Vec<String> {
    for section in &snippet.sections {
        if let Section::Effects(effects_section) = section {
            return effects_section.effects.iter()
                .map(|e| e.name.clone())
                .collect();
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checker_creation() {
        let checker = SnippetChecker::new();
        assert!(checker.errors.is_empty());
    }
}
