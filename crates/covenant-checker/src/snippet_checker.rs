//! Snippet mode type checker
//!
//! Type checks snippets with IR-based step syntax (the primary Covenant format).

use std::collections::{HashMap, HashSet};
use covenant_ast::{
    Snippet, SnippetKind, Section, SignatureKind, BodySection,
    Step, StepKind, ComputeStep, Operation, Input, InputSource, CallStep,
    ReturnStep, ReturnValue, IfStep, BindStep, BindSource, MatchStep, MatchPattern,
    FunctionSignature, ReturnType, Type, TypeKind, Literal, QueryStep, QueryContent,
    StructSignature, EnumSignature, StructConstruction,
};
use crate::{CheckError, CheckResult, ResolvedType, SymbolTable, SymbolKind, EffectTable, TypeRegistry, VariantDef};

/// Checker for snippet-mode programs
pub struct SnippetChecker {
    symbols: SymbolTable,
    effects: EffectTable,
    errors: Vec<CheckError>,
    /// Local scope for current function body
    locals: HashMap<String, ResolvedType>,
    /// Map of function names to their return types (for recursive calls)
    function_returns: HashMap<String, ResolvedType>,
    /// Registry of struct and enum type definitions
    type_registry: TypeRegistry,
    /// Expected return type for current function being checked
    current_return_type: Option<ResolvedType>,
}

impl SnippetChecker {
    pub fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            effects: EffectTable::new(),
            errors: Vec::new(),
            locals: HashMap::new(),
            function_returns: HashMap::new(),
            type_registry: TypeRegistry::new(),
            current_return_type: None,
        }
    }

    /// Check all snippets and return the result
    pub fn check_snippets(mut self, snippets: &[Snippet]) -> Result<CheckResult, Vec<CheckError>> {
        // First pass: register all types and function signatures
        for snippet in snippets {
            match snippet.kind {
                SnippetKind::Function => self.register_function_signature(snippet),
                SnippetKind::Struct => self.register_struct_type(snippet),
                SnippetKind::Enum => self.register_enum_type(snippet),
                _ => {}
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
        let (params_info, steps_cloned, expected_return) = {
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

            let expected_return = sig.returns.as_ref()
                .map(|r| self.resolve_return_type(r));

            (params_info, body.steps.clone(), expected_return)
        };

        // Set up local scope with parameters
        self.locals.clear();
        for (name, ty) in params_info {
            self.locals.insert(name, ty);
        }

        // Set expected return type for this function
        self.current_return_type = expected_return;

        // Check each step
        for step in &steps_cloned {
            self.check_step(step);
        }

        // Clear expected return type after checking
        self.current_return_type = None;
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
            StepKind::Query(query) => self.infer_query_step(query),
            StepKind::Insert(_) => ResolvedType::Unknown, // TODO: infer inserted type
            StepKind::Update(_) => ResolvedType::Unknown, // TODO: infer update count
            StepKind::Delete(_) => ResolvedType::None,
            StepKind::Transaction(_) => ResolvedType::Unknown,
            StepKind::Traverse(_) => ResolvedType::Unknown,
            StepKind::Construct(construct) => self.infer_construct_step(construct),
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

            // Boolean operations: require Bool inputs, return Bool
            Operation::And | Operation::Or => {
                for input_type in &input_types {
                    if !matches!(input_type, ResolvedType::Bool) {
                        self.errors.push(CheckError::TypeMismatch {
                            expected: "Bool".to_string(),
                            found: input_type.display(),
                        });
                    }
                }
                ResolvedType::Bool
            }
            Operation::Not => {
                if let Some(input_type) = input_types.first() {
                    if !matches!(input_type, ResolvedType::Bool) {
                        self.errors.push(CheckError::TypeMismatch {
                            expected: "Bool".to_string(),
                            found: input_type.display(),
                        });
                    }
                }
                ResolvedType::Bool
            }

            // Negation: same as input type
            Operation::Neg => input_types.first().cloned().unwrap_or(ResolvedType::Int),

            // String operations that return String
            Operation::Concat | Operation::Upper | Operation::Lower |
            Operation::Trim | Operation::TrimStart | Operation::TrimEnd |
            Operation::Replace | Operation::Join | Operation::Repeat |
            Operation::StrReverse | Operation::PadStart | Operation::PadEnd => ResolvedType::String,

            // String slice returns String
            Operation::Slice => ResolvedType::String,

            // String operations that return Bool
            Operation::Contains | Operation::StartsWith | Operation::EndsWith |
            Operation::IsEmpty => ResolvedType::Bool,

            // String operations that return Int
            Operation::StrLen | Operation::ByteLen => ResolvedType::Int,

            // String operations that return Optional
            Operation::IndexOf => ResolvedType::Optional(Box::new(ResolvedType::Int)),
            Operation::CharAt => ResolvedType::Optional(Box::new(ResolvedType::Char)),

            // Split returns list of strings
            Operation::Split => ResolvedType::List(Box::new(ResolvedType::String)),

            // Numeric operations: Abs, Sign preserve type; Min/Max preserve type
            Operation::Abs | Operation::Sign => input_types.first().cloned().unwrap_or(ResolvedType::Int),
            Operation::Min | Operation::Max | Operation::Clamp => {
                if input_types.iter().any(|t| matches!(t, ResolvedType::Float)) {
                    ResolvedType::Float
                } else {
                    ResolvedType::Int
                }
            }

            // Float operations always return Float
            Operation::Pow | Operation::Sqrt => ResolvedType::Float,

            // Rounding operations return Int
            Operation::Floor | Operation::Ceil | Operation::Round | Operation::Trunc => ResolvedType::Int,

            // Bitwise operations return Int
            Operation::BitAnd | Operation::BitOr | Operation::BitXor | Operation::BitNot |
            Operation::BitShl | Operation::BitShr | Operation::BitUshr => ResolvedType::Int,

            // Conversion operations
            Operation::ToInt => ResolvedType::Int,
            Operation::ToFloat => ResolvedType::Float,
            Operation::ToString => ResolvedType::String,
            Operation::ParseInt => ResolvedType::Union(vec![
                ResolvedType::Int,
                ResolvedType::Named { name: "ParseError".to_string(), id: covenant_ast::SymbolId(0), args: vec![] }
            ]),
            Operation::ParseFloat => ResolvedType::Union(vec![
                ResolvedType::Float,
                ResolvedType::Named { name: "ParseError".to_string(), id: covenant_ast::SymbolId(0), args: vec![] }
            ]),

            // List operations that return Int
            Operation::ListLen => ResolvedType::Int,

            // List operations that return Bool
            Operation::ListContains | Operation::ListIsEmpty => ResolvedType::Bool,

            // List operations that return Optional element
            Operation::ListGet | Operation::ListFirst | Operation::ListLast => {
                match input_types.first() {
                    Some(ResolvedType::List(inner)) => ResolvedType::Optional(inner.clone()),
                    _ => ResolvedType::Optional(Box::new(ResolvedType::Unknown))
                }
            }

            // List operations that return Optional Int
            Operation::ListIndexOf => ResolvedType::Optional(Box::new(ResolvedType::Int)),

            // List operations that return new list (same type)
            Operation::ListAppend | Operation::ListPrepend | Operation::ListConcat |
            Operation::ListSlice | Operation::ListReverse | Operation::ListTake |
            Operation::ListDrop | Operation::ListSort | Operation::ListDedup => {
                input_types.first().cloned().unwrap_or(ResolvedType::List(Box::new(ResolvedType::Unknown)))
            }

            // ListFlatten: List<List<T>> -> List<T>
            Operation::ListFlatten => {
                match input_types.first() {
                    Some(ResolvedType::List(inner)) => match inner.as_ref() {
                        ResolvedType::List(inner_inner) => ResolvedType::List(inner_inner.clone()),
                        _ => ResolvedType::List(Box::new(ResolvedType::Unknown))
                    },
                    _ => ResolvedType::List(Box::new(ResolvedType::Unknown))
                }
            }

            // Map operations that return Int
            Operation::MapLen => ResolvedType::Int,

            // Map operations that return Bool
            Operation::MapHas | Operation::MapIsEmpty => ResolvedType::Bool,

            // Map operations that return Optional value
            Operation::MapGet => {
                match input_types.first() {
                    Some(ResolvedType::Named { args, .. }) if args.len() >= 2 => {
                        ResolvedType::Optional(Box::new(args[1].clone()))
                    }
                    _ => ResolvedType::Optional(Box::new(ResolvedType::Unknown))
                }
            }

            // Map operations that return new map
            Operation::MapInsert | Operation::MapRemove | Operation::MapMerge => {
                input_types.first().cloned().unwrap_or(ResolvedType::Unknown)
            }

            // Map operations that return lists
            Operation::MapKeys => {
                match input_types.first() {
                    Some(ResolvedType::Named { args, .. }) if !args.is_empty() => {
                        ResolvedType::List(Box::new(args[0].clone()))
                    }
                    _ => ResolvedType::List(Box::new(ResolvedType::Unknown))
                }
            }
            Operation::MapValues => {
                match input_types.first() {
                    Some(ResolvedType::Named { args, .. }) if args.len() >= 2 => {
                        ResolvedType::List(Box::new(args[1].clone()))
                    }
                    _ => ResolvedType::List(Box::new(ResolvedType::Unknown))
                }
            }
            Operation::MapEntries => {
                match input_types.first() {
                    Some(ResolvedType::Named { args, .. }) if args.len() >= 2 => {
                        ResolvedType::List(Box::new(ResolvedType::Tuple(args.clone())))
                    }
                    _ => ResolvedType::List(Box::new(ResolvedType::Unknown))
                }
            }

            // Set operations that return Int
            Operation::SetLen => ResolvedType::Int,

            // Set operations that return Bool
            Operation::SetHas | Operation::SetIsEmpty | Operation::SetIsSubset |
            Operation::SetIsSuperset => ResolvedType::Bool,

            // Set operations that return new set
            Operation::SetAdd | Operation::SetRemove | Operation::SetUnion |
            Operation::SetIntersect | Operation::SetDiff | Operation::SetSymmetricDiff => {
                input_types.first().cloned().unwrap_or(ResolvedType::Set(Box::new(ResolvedType::Unknown)))
            }

            // Set to list
            Operation::SetToList => {
                match input_types.first() {
                    Some(ResolvedType::Set(inner)) => ResolvedType::List(inner.clone()),
                    _ => ResolvedType::List(Box::new(ResolvedType::Unknown))
                }
            }

            // DateTime operations that return Int
            Operation::DtYear | Operation::DtMonth | Operation::DtDay |
            Operation::DtHour | Operation::DtMinute | Operation::DtSecond |
            Operation::DtWeekday | Operation::DtUnix | Operation::DtDiff => ResolvedType::Int,

            // DateTime operations that return DateTime
            Operation::DtAddDays | Operation::DtAddHours |
            Operation::DtAddMinutes | Operation::DtAddSeconds => ResolvedType::DateTime,

            // DateTime format returns String
            Operation::DtFormat => ResolvedType::String,

            // Bytes operations that return Int
            Operation::BytesLen => ResolvedType::Int,

            // Bytes operations that return Bool
            Operation::BytesIsEmpty => ResolvedType::Bool,

            // Bytes operations that return Optional Int
            Operation::BytesGet => ResolvedType::Optional(Box::new(ResolvedType::Int)),

            // Bytes operations that return Bytes
            Operation::BytesSlice | Operation::BytesConcat => ResolvedType::Bytes,

            // Bytes to string can fail
            Operation::BytesToString => ResolvedType::Union(vec![
                ResolvedType::String,
                ResolvedType::Named { name: "DecodeError".to_string(), id: covenant_ast::SymbolId(0), args: vec![] }
            ]),

            // Bytes to Base64/Hex return String
            Operation::BytesToBase64 | Operation::BytesToHex => ResolvedType::String,
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
        let inferred = match &ret.value {
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
        };

        // Check against expected return type
        if let Some(ref expected) = self.current_return_type {
            if !self.types_compatible(expected, &inferred) {
                self.errors.push(CheckError::TypeMismatch {
                    expected: expected.display(),
                    found: inferred.display(),
                });
            }
        }

        inferred
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
        // Check the match target exists and get its type
        let matched_type = if let Some(ty) = self.locals.get(&match_step.on) {
            ty.clone()
        } else {
            self.errors.push(CheckError::UndefinedSymbol {
                name: match_step.on.clone(),
            });
            return ResolvedType::Error;
        };

        // Check exhaustiveness
        self.check_match_exhaustiveness(match_step, &matched_type);

        // Clone cases to avoid borrow issues
        let cases = match_step.cases.clone();

        // Collect case result types
        let mut case_types: Vec<ResolvedType> = Vec::new();

        // Check each case
        for case in &cases {
            // Add pattern bindings to scope with appropriate types
            if let MatchPattern::Variant { variant, bindings } = &case.pattern {
                // Try to get the variant's field types from the matched type
                let variant_name = extract_variant_name(variant);
                let binding_type = self.get_variant_binding_type(&matched_type, &variant_name);

                for binding in bindings {
                    self.locals.insert(binding.clone(), binding_type.clone());
                }
            }

            // Check case steps
            for step in &case.steps {
                self.check_step(step);
            }

            // Get the type of the last step (if any) as the case result
            if let Some(last_step) = case.steps.last() {
                if let Some(ty) = self.locals.get(&last_step.output_binding) {
                    case_types.push(ty.clone());
                }
            }
        }

        // Return the result type of the match
        if case_types.is_empty() {
            ResolvedType::None
        } else if case_types.iter().all(|t| t == &case_types[0]) {
            // All cases return the same type
            case_types.into_iter().next().unwrap_or(ResolvedType::None)
        } else {
            // Cases return different types - result is a union
            ResolvedType::Union(case_types)
        }
    }

    /// Infer type of a construct step
    fn infer_construct_step(&mut self, construct: &StructConstruction) -> ResolvedType {
        // The type of a construct step is the struct type being constructed
        self.resolve_type(&construct.ty)
    }

    /// Get the binding type for a variant pattern
    fn get_variant_binding_type(&self, matched_type: &ResolvedType, variant_name: &str) -> ResolvedType {
        match matched_type {
            ResolvedType::Named { name, .. } => {
                // Look up enum variant in type registry
                if let Some(enum_def) = self.type_registry.get_enum(name) {
                    for variant in &enum_def.variants {
                        if variant.name == variant_name {
                            // If variant has fields, return the first field's type
                            // (simplified - real implementation would handle multiple fields)
                            if let Some(fields) = &variant.fields {
                                if let Some((_, ty)) = fields.first() {
                                    return ty.clone();
                                }
                            }
                            return ResolvedType::None;
                        }
                    }
                }
                ResolvedType::Unknown
            }
            ResolvedType::Union(types) => {
                // Find the matching type in the union
                for ty in types {
                    if ty.display() == variant_name {
                        return ty.clone();
                    }
                }
                ResolvedType::Unknown
            }
            ResolvedType::Optional(inner) => {
                if variant_name == "some" || variant_name == inner.display() {
                    (**inner).clone()
                } else {
                    ResolvedType::None
                }
            }
            _ => ResolvedType::Unknown,
        }
    }

    /// Resolve an input's type
    fn resolve_input_type(&mut self, input: &Input) -> ResolvedType {
        match &input.source {
            InputSource::Var(name) => {
                match self.locals.get(name) {
                    Some(ty) => ty.clone(),
                    None => {
                        self.errors.push(CheckError::UndefinedSymbol {
                            name: name.clone(),
                        });
                        ResolvedType::Error
                    }
                }
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

    /// Check if two types are compatible
    fn types_compatible(&self, expected: &ResolvedType, found: &ResolvedType) -> bool {
        match (expected, found) {
            // Unknown and Error types are compatible with anything
            (ResolvedType::Unknown, _) | (_, ResolvedType::Unknown) => true,
            (ResolvedType::Error, _) | (_, ResolvedType::Error) => true,

            // Primitive types
            (ResolvedType::Int, ResolvedType::Int) => true,
            (ResolvedType::Float, ResolvedType::Float) => true,
            (ResolvedType::Float, ResolvedType::Int) => true, // Int can be used as Float
            (ResolvedType::Bool, ResolvedType::Bool) => true,
            (ResolvedType::String, ResolvedType::String) => true,
            (ResolvedType::Char, ResolvedType::Char) => true,
            (ResolvedType::Bytes, ResolvedType::Bytes) => true,
            (ResolvedType::DateTime, ResolvedType::DateTime) => true,
            (ResolvedType::None, ResolvedType::None) => true,

            // Optional types
            (ResolvedType::Optional(_), ResolvedType::None) => true,
            (ResolvedType::Optional(e), ResolvedType::Optional(f)) => self.types_compatible(e, f),
            (ResolvedType::Optional(e), f) => self.types_compatible(e, f),

            // List types
            (ResolvedType::List(e), ResolvedType::List(f)) => self.types_compatible(e, f),

            // Set types
            (ResolvedType::Set(e), ResolvedType::Set(f)) => self.types_compatible(e, f),

            // Union types - value must be compatible with at least one member
            (ResolvedType::Union(members), found) => {
                members.iter().any(|m| self.types_compatible(m, found))
            }

            // Assigning union to non-union - all members must be compatible
            (expected, ResolvedType::Union(members)) => {
                members.iter().all(|m| self.types_compatible(expected, m))
            }

            // Tuple types - all elements must match
            (ResolvedType::Tuple(e), ResolvedType::Tuple(f)) => {
                e.len() == f.len()
                    && e.iter().zip(f).all(|(a, b)| self.types_compatible(a, b))
            }

            // Named types with generic args
            (
                ResolvedType::Named { name: n1, args: a1, .. },
                ResolvedType::Named { name: n2, args: a2, .. },
            ) => {
                n1 == n2
                    && a1.len() == a2.len()
                    && a1.iter().zip(a2).all(|(t1, t2)| self.types_compatible(t1, t2))
            }

            // Struct types - all fields must match
            (ResolvedType::Struct(e), ResolvedType::Struct(f)) => {
                e.len() == f.len()
                    && e.iter().all(|(name, ty)| {
                        f.iter()
                            .find(|(n, _)| n == name)
                            .map(|(_, t)| self.types_compatible(ty, t))
                            .unwrap_or(false)
                    })
            }

            // Function types
            (
                ResolvedType::Function { params: p1, ret: r1 },
                ResolvedType::Function { params: p2, ret: r2 },
            ) => {
                p1.len() == p2.len()
                    && p1.iter().zip(p2).all(|(a, b)| self.types_compatible(a, b))
                    && self.types_compatible(r1, r2)
            }

            _ => false,
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
                    "Char" => ResolvedType::Char,
                    "Bytes" => ResolvedType::Bytes,
                    "DateTime" => ResolvedType::DateTime,
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

    /// Register a struct type (first pass)
    fn register_struct_type(&mut self, snippet: &Snippet) {
        let struct_sig = match find_struct_signature(snippet) {
            Some(s) => s,
            None => return,
        };

        let fields: Vec<(String, ResolvedType)> = struct_sig
            .fields
            .iter()
            .map(|f| (f.name.clone(), self.resolve_type(&f.ty)))
            .collect();

        // Register in type registry
        self.type_registry
            .register_struct(struct_sig.name.clone(), fields.clone());

        // Also register as a symbol
        self.symbols.define(
            struct_sig.name.clone(),
            SymbolKind::Type,
            ResolvedType::Struct(fields),
        );
    }

    /// Register an enum type (first pass)
    fn register_enum_type(&mut self, snippet: &Snippet) {
        let enum_sig = match find_enum_signature(snippet) {
            Some(e) => e,
            None => return,
        };

        let variants: Vec<VariantDef> = enum_sig
            .variants
            .iter()
            .map(|v| VariantDef {
                name: v.name.clone(),
                fields: v.fields.as_ref().map(|fields| {
                    fields
                        .iter()
                        .map(|f| (f.name.clone(), self.resolve_type(&f.ty)))
                        .collect()
                }),
            })
            .collect();

        // Register in type registry
        self.type_registry
            .register_enum(enum_sig.name.clone(), variants);

        // Also register as a symbol
        self.symbols.define(
            enum_sig.name.clone(),
            SymbolKind::Type,
            ResolvedType::Named {
                name: enum_sig.name.clone(),
                id: covenant_ast::SymbolId(0),
                args: vec![],
            },
        );
    }

    /// Check a struct snippet (second pass - validate field types)
    fn check_struct_snippet(&mut self, _snippet: &Snippet) {
        // Struct validation happens during registration
        // Future: could validate field type references exist
    }

    /// Check an enum snippet (second pass - validate variant types)
    fn check_enum_snippet(&mut self, _snippet: &Snippet) {
        // Enum validation happens during registration
        // Future: could validate variant field type references exist
    }

    /// Get variant names for a type (for match exhaustiveness)
    fn get_type_variants(&self, ty: &ResolvedType) -> Option<Vec<String>> {
        match ty {
            ResolvedType::Named { name, .. } => {
                // Look up enum in type registry
                self.type_registry.get_enum_variants(name)
            }
            ResolvedType::Union(types) => {
                // Union types: each member type is a "variant"
                Some(types.iter().map(|t| t.display()).collect())
            }
            ResolvedType::Optional(inner) => {
                // Optional is effectively a union of inner | None
                Some(vec![inner.display(), "none".to_string()])
            }
            _ => None,
        }
    }

    /// Check match exhaustiveness
    fn check_match_exhaustiveness(&mut self, match_step: &MatchStep, matched_type: &ResolvedType) {
        let all_variants = match self.get_type_variants(matched_type) {
            Some(variants) => variants,
            None => return, // Not a matchable type (enum/union/optional)
        };

        let mut covered: HashSet<String> = HashSet::new();
        let mut has_wildcard = false;

        for case in &match_step.cases {
            match &case.pattern {
                MatchPattern::Variant { variant, .. } => {
                    // Extract variant name (e.g., "Json::String" -> "String")
                    let variant_name = extract_variant_name(variant);
                    covered.insert(variant_name);
                }
                MatchPattern::Wildcard => {
                    has_wildcard = true;
                }
            }
        }

        // Wildcard covers all remaining variants
        if has_wildcard {
            return;
        }

        let missing: Vec<String> = all_variants
            .iter()
            .filter(|v| !covered.contains(*v))
            .cloned()
            .collect();

        if !missing.is_empty() {
            self.errors.push(CheckError::NonExhaustiveMatch {
                missing,
                matched_type: matched_type.display(),
            });
        }
    }

    /// Infer type of a query step
    fn infer_query_step(&mut self, query: &QueryStep) -> ResolvedType {
        match &query.content {
            QueryContent::Covenant(cov_query) => {
                // For project queries, return metadata types
                if query.target == "project" {
                    return self.infer_project_query(&cov_query.from);
                }

                // For other Covenant queries, infer from target
                // The result is typically a list of the from type
                let from_type = self.resolve_from_type(&cov_query.from);

                // Check if limit=1 (returns optional instead of list)
                if cov_query.limit == Some(1) {
                    ResolvedType::Optional(Box::new(from_type))
                } else {
                    ResolvedType::List(Box::new(from_type))
                }
            }
            QueryContent::Dialect(dialect_query) => {
                // SQL dialect queries must have explicit returns type
                self.resolve_return_type(&dialect_query.returns)
            }
        }
    }

    /// Infer type for project metadata queries
    fn infer_project_query(&self, from: &str) -> ResolvedType {
        // Project queries return lists of metadata structs
        let element_type = match from {
            "functions" => ResolvedType::Struct(vec![
                ("id".to_string(), ResolvedType::String),
                ("name".to_string(), ResolvedType::String),
                (
                    "effects".to_string(),
                    ResolvedType::List(Box::new(ResolvedType::String)),
                ),
            ]),
            "structs" | "types" => ResolvedType::Struct(vec![
                ("id".to_string(), ResolvedType::String),
                ("name".to_string(), ResolvedType::String),
            ]),
            "requirements" => ResolvedType::Struct(vec![
                ("id".to_string(), ResolvedType::String),
                ("text".to_string(), ResolvedType::Optional(Box::new(ResolvedType::String))),
                ("priority".to_string(), ResolvedType::String),
            ]),
            "tests" => ResolvedType::Struct(vec![
                ("id".to_string(), ResolvedType::String),
                ("kind".to_string(), ResolvedType::String),
                (
                    "covers".to_string(),
                    ResolvedType::List(Box::new(ResolvedType::String)),
                ),
            ]),
            _ => ResolvedType::Unknown,
        };

        ResolvedType::List(Box::new(element_type))
    }

    /// Resolve a "from" clause type
    fn resolve_from_type(&self, from: &str) -> ResolvedType {
        // Check if it's a registered type
        if let Some(struct_def) = self.type_registry.get_struct(from) {
            return ResolvedType::Struct(struct_def.fields.clone());
        }

        // Check symbol table
        if let Some(symbol) = self.symbols.lookup(from) {
            return symbol.ty.clone();
        }

        // Unknown type
        ResolvedType::Named {
            name: from.to_string(),
            id: covenant_ast::SymbolId(0),
            args: vec![],
        }
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

/// Find the struct signature in a snippet
fn find_struct_signature(snippet: &Snippet) -> Option<&StructSignature> {
    for section in &snippet.sections {
        if let Section::Signature(sig) = section {
            if let SignatureKind::Struct(struct_sig) = &sig.kind {
                return Some(struct_sig);
            }
        }
    }
    None
}

/// Find the enum signature in a snippet
fn find_enum_signature(snippet: &Snippet) -> Option<&EnumSignature> {
    for section in &snippet.sections {
        if let Section::Signature(sig) = section {
            if let SignatureKind::Enum(enum_sig) = &sig.kind {
                return Some(enum_sig);
            }
        }
    }
    None
}

/// Extract variant name from full path (e.g., "Json::String" -> "String")
fn extract_variant_name(full_name: &str) -> String {
    full_name
        .split("::")
        .last()
        .unwrap_or(full_name)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checker_creation() {
        let checker = SnippetChecker::new();
        assert!(checker.errors.is_empty());
    }

    #[test]
    fn test_type_registry_struct() {
        let mut registry = TypeRegistry::new();
        registry.register_struct(
            "User".to_string(),
            vec![
                ("id".to_string(), ResolvedType::Int),
                ("name".to_string(), ResolvedType::String),
            ],
        );

        let user = registry.get_struct("User");
        assert!(user.is_some());
        assert_eq!(user.unwrap().fields.len(), 2);

        let id_type = registry.get_struct_field("User", "id");
        assert!(matches!(id_type, Some(ResolvedType::Int)));

        let unknown = registry.get_struct("Unknown");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_type_registry_enum() {
        let mut registry = TypeRegistry::new();
        registry.register_enum(
            "Result".to_string(),
            vec![
                VariantDef {
                    name: "Ok".to_string(),
                    fields: Some(vec![("value".to_string(), ResolvedType::Int)]),
                },
                VariantDef {
                    name: "Err".to_string(),
                    fields: Some(vec![("error".to_string(), ResolvedType::String)]),
                },
            ],
        );

        let result_enum = registry.get_enum("Result");
        assert!(result_enum.is_some());
        assert_eq!(result_enum.unwrap().variants.len(), 2);

        let variants = registry.get_enum_variants("Result");
        assert!(variants.is_some());
        let v = variants.unwrap();
        assert!(v.contains(&"Ok".to_string()));
        assert!(v.contains(&"Err".to_string()));
    }

    #[test]
    fn test_extract_variant_name() {
        assert_eq!(extract_variant_name("Json::String"), "String");
        assert_eq!(extract_variant_name("Option::Some"), "Some");
        assert_eq!(extract_variant_name("Simple"), "Simple");
        assert_eq!(extract_variant_name("a::b::c"), "c");
    }

    #[test]
    fn test_get_type_variants_for_union() {
        let checker = SnippetChecker::new();
        let union_type = ResolvedType::Union(vec![
            ResolvedType::Int,
            ResolvedType::String,
        ]);

        let variants = checker.get_type_variants(&union_type);
        assert!(variants.is_some());
        let v = variants.unwrap();
        assert_eq!(v.len(), 2);
        assert!(v.contains(&"Int".to_string()));
        assert!(v.contains(&"String".to_string()));
    }

    #[test]
    fn test_get_type_variants_for_optional() {
        let checker = SnippetChecker::new();
        let optional_type = ResolvedType::Optional(Box::new(ResolvedType::Int));

        let variants = checker.get_type_variants(&optional_type);
        assert!(variants.is_some());
        let v = variants.unwrap();
        assert_eq!(v.len(), 2);
        assert!(v.contains(&"Int".to_string()));
        assert!(v.contains(&"none".to_string()));
    }

    #[test]
    fn test_get_type_variants_for_non_matchable() {
        let checker = SnippetChecker::new();

        // Primitives are not matchable (no variants)
        assert!(checker.get_type_variants(&ResolvedType::Int).is_none());
        assert!(checker.get_type_variants(&ResolvedType::String).is_none());
        assert!(checker.get_type_variants(&ResolvedType::Bool).is_none());
    }

    #[test]
    fn test_infer_project_query_functions() {
        let checker = SnippetChecker::new();
        let result = checker.infer_project_query("functions");

        match result {
            ResolvedType::List(inner) => {
                match *inner {
                    ResolvedType::Struct(fields) => {
                        assert!(fields.iter().any(|(name, _)| name == "id"));
                        assert!(fields.iter().any(|(name, _)| name == "name"));
                        assert!(fields.iter().any(|(name, _)| name == "effects"));
                    }
                    _ => panic!("Expected struct type for function info"),
                }
            }
            _ => panic!("Expected list type for project query"),
        }
    }

    #[test]
    fn test_infer_project_query_unknown() {
        let checker = SnippetChecker::new();
        let result = checker.infer_project_query("unknown_thing");

        match result {
            ResolvedType::List(inner) => {
                assert!(matches!(*inner, ResolvedType::Unknown));
            }
            _ => panic!("Expected list type for project query"),
        }
    }
}
