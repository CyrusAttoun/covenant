//! Effect tracking and validation (Phase 3)
//!
//! This module handles:
//! - Effect registration and lookup (EffectTable)
//! - Transitive effect closure computation
//! - Effect declaration validation (I2 invariant)
//! - Parameterized effect validation (effect subsumption)
//! - Rich diagnostic generation for effect violations

use std::collections::{HashMap, HashSet};
use covenant_ast::{EffectDecl, Literal, SymbolId, EffectId, Span};
use covenant_symbols::{SymbolGraph, SymbolInfo};

use crate::diagnostics::{
    Diagnostic, EffectDiagnosticBuilder, FixSuggestion, RelatedLocation, CallChainEntry,
};

/// Table of effects and their relationships
#[derive(Debug, Default)]
pub struct EffectTable {
    effects: Vec<Effect>,
    by_name: HashMap<String, EffectId>,
    /// Which symbols have which effects
    symbol_effects: HashMap<SymbolId, HashSet<EffectId>>,
}

/// An effect definition
#[derive(Debug, Clone)]
pub struct Effect {
    pub id: EffectId,
    pub name: String,
    pub source: String,
}

impl EffectTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an effect
    pub fn register(&mut self, name: String, source: String) -> EffectId {
        if let Some(&id) = self.by_name.get(&name) {
            return id;
        }

        let id = EffectId(self.effects.len() as u32);
        let effect = Effect {
            id,
            name: name.clone(),
            source,
        };
        self.effects.push(effect);
        self.by_name.insert(name, id);
        id
    }

    /// Get an effect by name
    pub fn get_by_name(&self, name: &str) -> Option<&Effect> {
        self.by_name.get(name).and_then(|&id| self.get(id))
    }

    /// Get an effect by ID
    pub fn get(&self, id: EffectId) -> Option<&Effect> {
        self.effects.get(id.0 as usize)
    }

    /// Add an effect to a symbol
    pub fn add_effect(&mut self, symbol: SymbolId, effect: EffectId) {
        self.symbol_effects
            .entry(symbol)
            .or_default()
            .insert(effect);
    }

    /// Get all effects for a symbol
    pub fn effects_of(&self, symbol: SymbolId) -> HashSet<EffectId> {
        self.symbol_effects
            .get(&symbol)
            .cloned()
            .unwrap_or_default()
    }

    /// Check if a symbol is pure (has no effects)
    pub fn is_pure(&self, symbol: SymbolId) -> bool {
        self.effects_of(symbol).is_empty()
    }

    /// Compute transitive closure of effects for a function
    /// (includes effects from all called functions)
    pub fn transitive_effects(
        &self,
        symbol: SymbolId,
        calls: &HashMap<SymbolId, HashSet<SymbolId>>,
    ) -> HashSet<EffectId> {
        let mut visited = HashSet::new();
        let mut effects = HashSet::new();
        self.collect_effects(symbol, calls, &mut visited, &mut effects);
        effects
    }

    fn collect_effects(
        &self,
        symbol: SymbolId,
        calls: &HashMap<SymbolId, HashSet<SymbolId>>,
        visited: &mut HashSet<SymbolId>,
        effects: &mut HashSet<EffectId>,
    ) {
        if !visited.insert(symbol) {
            return;
        }

        // Add direct effects
        effects.extend(self.effects_of(symbol));

        // Add effects from called functions
        if let Some(called) = calls.get(&symbol) {
            for &callee in called {
                self.collect_effects(callee, calls, visited, effects);
            }
        }
    }
}

// =============================================================================
// Phase 3: Effect Checker (I2 Invariant Validation)
// =============================================================================

/// Effect closure for a single symbol
#[derive(Debug, Clone)]
pub struct EffectClosure {
    /// Effects declared in the effects section (by name, for backwards compatibility)
    pub declared: HashSet<String>,
    /// Full effect declarations with parameters
    pub declared_full: Vec<EffectDecl>,
    /// Computed transitive closure (includes effects from callees) - names only
    pub computed: HashSet<String>,
    /// Full computed effects with parameters
    pub computed_full: Vec<EffectDecl>,
    /// True if declared is empty (pure function)
    pub is_pure: bool,
}

/// Result of effect checking phase
#[derive(Debug)]
pub struct EffectCheckResult {
    /// Effect closures indexed by symbol name
    pub closures: HashMap<String, EffectClosure>,
    /// Effect violations found
    pub violations: Vec<EffectError>,
}

/// Effect validation errors
#[derive(Debug, Clone)]
pub enum EffectError {
    /// Pure function calls effectful code (E-EFFECT-001)
    PureCallsEffectful {
        /// Name of the pure function
        function: String,
        /// Name of the effectful callee
        callee: String,
        /// Effects introduced by the callee
        effects: Vec<String>,
        /// Source span of the function
        span: Span,
    },
    /// Missing effect declaration (E-EFFECT-002)
    MissingEffect {
        /// Name of the function
        function: String,
        /// Effects that are missing from declaration
        missing: Vec<String>,
        /// Callee that introduced these effects
        source_callee: String,
        /// Source span of the function
        span: Span,
    },
    /// Effect parameter not covered (E-EFFECT-003)
    ParameterNotCovered {
        /// Name of the function
        function: String,
        /// The effect name
        effect_name: String,
        /// The parameter name that's not covered
        param_name: String,
        /// The required parameter value
        required_value: String,
        /// The declared parameter value (if any)
        declared_value: Option<String>,
        /// Callee that requires this effect
        source_callee: String,
        /// Source span of the function
        span: Span,
    },
}

/// Compute effect closures for all symbols in the graph and validate I2 invariant.
///
/// The I2 invariant states that for every function:
/// - `declared_effects ⊇ computed_effects`
/// - If a function declares no effects (pure), it cannot call effectful code
pub fn check_effects(graph: &SymbolGraph) -> EffectCheckResult {
    let mut closures = HashMap::new();
    let mut violations = Vec::new();

    // Process all callable symbols (functions and externs)
    for symbol in graph.iter().filter(|s| s.is_callable()) {
        let closure = compute_closure_for_symbol(symbol, graph);

        // Validate: declared must cover computed
        if let Some(error) = validate_closure(symbol, &closure, graph) {
            violations.push(error);
        }

        closures.insert(symbol.name.clone(), closure);
    }

    EffectCheckResult { closures, violations }
}

/// Compute transitive effect closure for a single symbol
fn compute_closure_for_symbol(symbol: &SymbolInfo, graph: &SymbolGraph) -> EffectClosure {
    let declared: HashSet<String> = symbol.declared_effects.iter().map(|e| e.name.clone()).collect();
    let declared_full = symbol.declared_effects.clone();
    let is_pure = declared.is_empty();

    let mut computed = HashSet::new();
    let mut computed_full = Vec::new();
    let mut visited = HashSet::new();

    collect_transitive_effects(symbol, graph, &mut visited, &mut computed, &mut computed_full);

    EffectClosure { declared, declared_full, computed, computed_full, is_pure }
}

/// Recursively collect effects from callees
fn collect_transitive_effects(
    symbol: &SymbolInfo,
    graph: &SymbolGraph,
    visited: &mut HashSet<String>,
    effects: &mut HashSet<String>,
    effects_full: &mut Vec<EffectDecl>,
) {
    if !visited.insert(symbol.name.clone()) {
        return; // Already visited (handles cycles)
    }

    // Add this symbol's declared effects
    for effect in &symbol.declared_effects {
        effects.insert(effect.name.clone());
        // For effects with parameters, we need to keep all of them so we can
        // check if any callee requires stricter parameters than what's declared.
        // We'll deduplicate by (name, params) combination, but keep effects with
        // different parameters so the validation can check all of them.
        if effect.has_params() {
            // Only add if this exact (name, params) combo isn't already there
            let already_exists = effects_full.iter().any(|e| {
                e.name == effect.name && e.params.len() == effect.params.len() &&
                e.params.iter().zip(&effect.params).all(|(a, b)| {
                    a.name == b.name && a.value == b.value
                })
            });
            if !already_exists {
                effects_full.push(effect.clone());
            }
        } else if !effects_full.iter().any(|e| e.name == effect.name) {
            effects_full.push(effect.clone());
        }
    }

    // Recurse into callees
    for callee_name in &symbol.calls {
        if let Some(callee) = graph.get_by_name(callee_name) {
            collect_transitive_effects(callee, graph, visited, effects, effects_full);
        }
        // Note: unresolved calls are ignored here (handled in Phase 4)
    }
}

/// Validate that declared effects cover computed effects
fn validate_closure(
    symbol: &SymbolInfo,
    closure: &EffectClosure,
    graph: &SymbolGraph,
) -> Option<EffectError> {
    // First check: find effects by name that are in computed but not in declared
    let missing: Vec<String> = closure.computed
        .difference(&closure.declared)
        .cloned()
        .collect();

    if !missing.is_empty() {
        // Find which callee introduced these effects (for error message)
        let source_callee = find_effect_source(symbol, &missing, graph)
            .unwrap_or_else(|| "unknown".to_string());

        if closure.is_pure {
            return Some(EffectError::PureCallsEffectful {
                function: symbol.name.clone(),
                callee: source_callee,
                effects: missing,
                span: symbol.span,
            });
        } else {
            return Some(EffectError::MissingEffect {
                function: symbol.name.clone(),
                missing,
                source_callee,
                span: symbol.span,
            });
        }
    }

    // Second check: for effects with the same name, validate parameters
    // The declared effect must subsume (cover) the required effect
    // For each required effect with params, find if any declared effect covers it
    for required in &closure.computed_full {
        if !required.has_params() {
            continue; // No parameters to check
        }

        // Skip if this is the symbol's own effect declaration (don't validate self against self)
        let is_own_effect = symbol.declared_effects.iter().any(|own| {
            own.name == required.name && own.params.len() == required.params.len() &&
            own.params.iter().zip(&required.params).all(|(a, b)| {
                a.name == b.name && a.value == b.value
            })
        });
        if is_own_effect {
            continue;
        }

        // Find the declared effect with matching name
        if let Some(declared) = closure.declared_full.iter().find(|d| d.name == required.name) {
            // Check if declared subsumes required
            if let Some(error) = check_effect_subsumption(symbol, declared, required, graph) {
                return Some(error);
            }
        }
    }

    None
}

/// Check if declared effect subsumes required effect (parameter-aware matching)
///
/// Rules:
/// - If required has no parameters, any declaration with same name is fine
/// - If declared has no parameters, it covers any parameterized version (wildcard)
/// - If both have parameters, declared must subsume required for each param:
///   - For path parameters: declared path must be a prefix of required path
///   - For other parameters: declared must equal required (exact match)
fn check_effect_subsumption(
    symbol: &SymbolInfo,
    declared: &EffectDecl,
    required: &EffectDecl,
    graph: &SymbolGraph,
) -> Option<EffectError> {
    // If declared has no parameters, it's a wildcard - covers everything
    if !declared.has_params() {
        return None;
    }

    // If required has no parameters but declared does, that's fine too
    if !required.has_params() {
        return None;
    }

    // Both have parameters - check each required parameter
    for req_param in &required.params {
        match declared.get_param(&req_param.name) {
            Some(decl_param) => {
                // Check if declared value subsumes required value
                if !param_value_subsumes(&decl_param.value, &req_param.value, &req_param.name) {
                    let source_callee = find_effect_source_for_param(
                        symbol, &required.name, &req_param.name, graph
                    ).unwrap_or_else(|| "unknown".to_string());

                    return Some(EffectError::ParameterNotCovered {
                        function: symbol.name.clone(),
                        effect_name: required.name.clone(),
                        param_name: req_param.name.clone(),
                        required_value: literal_to_string(&req_param.value),
                        declared_value: Some(literal_to_string(&decl_param.value)),
                        source_callee,
                        span: symbol.span,
                    });
                }
            }
            None => {
                // Declared doesn't have this parameter - that's OK, it means unrestricted
            }
        }
    }

    None
}

/// Check if a declared parameter value subsumes a required parameter value
///
/// For "path" parameters: declared must be a prefix of required (path="/data" subsumes path="/data/users")
/// For other parameters: exact match required
fn param_value_subsumes(declared: &Literal, required: &Literal, param_name: &str) -> bool {
    // Special handling for path parameters
    if param_name == "path" {
        if let (Literal::String(decl_path), Literal::String(req_path)) = (declared, required) {
            // Declared path must be a prefix of required path
            // "/data" subsumes "/data/users" and "/data" but not "/other"
            return req_path.starts_with(decl_path.as_str());
        }
    }

    // For all other parameters, require exact match
    match (declared, required) {
        (Literal::Int(d), Literal::Int(r)) => d == r,
        (Literal::Float(d), Literal::Float(r)) => (d - r).abs() < f64::EPSILON,
        (Literal::String(d), Literal::String(r)) => d == r,
        (Literal::Bool(d), Literal::Bool(r)) => d == r,
        (Literal::None, Literal::None) => true,
        _ => false,
    }
}

/// Convert a Literal to a displayable string
fn literal_to_string(lit: &Literal) -> String {
    match lit {
        Literal::Int(i) => i.to_string(),
        Literal::Float(f) => f.to_string(),
        Literal::String(s) => format!("\"{}\"", s),
        Literal::Bool(b) => b.to_string(),
        Literal::None => "none".to_string(),
    }
}

/// Find which callee introduced a missing effect (for diagnostics)
fn find_effect_source(symbol: &SymbolInfo, missing: &[String], graph: &SymbolGraph) -> Option<String> {
    for callee_name in &symbol.calls {
        if let Some(callee) = graph.get_by_name(callee_name) {
            for effect in &callee.declared_effects {
                if missing.contains(&effect.name) {
                    return Some(callee_name.clone());
                }
            }
        }
    }
    // If not found in direct callees, it might be transitive
    // In that case, we could do a deeper search, but for now return None
    None
}

/// Find which callee introduced an effect with a specific parameter
fn find_effect_source_for_param(
    symbol: &SymbolInfo,
    effect_name: &str,
    _param_name: &str,
    graph: &SymbolGraph
) -> Option<String> {
    for callee_name in &symbol.calls {
        if let Some(callee) = graph.get_by_name(callee_name) {
            for effect in &callee.declared_effects {
                if effect.name == effect_name {
                    return Some(callee_name.clone());
                }
            }
        }
    }
    None
}

// =============================================================================
// Rich Diagnostic Generation
// =============================================================================

/// Check effects and return rich diagnostics (used with --explain flag)
pub fn check_effects_with_diagnostics(graph: &SymbolGraph) -> (EffectCheckResult, Vec<Diagnostic>) {
    let result = check_effects(graph);
    let diagnostics = result.violations.iter()
        .map(|err| explain_effect_violation(err, graph))
        .collect();
    (result, diagnostics)
}

/// Generate a rich diagnostic from an effect error, including call chain explanation
pub fn explain_effect_violation(error: &EffectError, graph: &SymbolGraph) -> Diagnostic {
    match error {
        EffectError::PureCallsEffectful { function, callee, effects, span } => {
            let call_chain = build_call_chain(function, callee, effects, graph);
            let related = build_related_locations(&call_chain);
            let suggestions = build_pure_calls_effectful_suggestions(function, effects, *span);

            EffectDiagnosticBuilder::pure_calls_effectful(
                function.clone(),
                callee.clone(),
                effects.clone(),
                *span,
            )
            .with_call_chain(call_chain)
            .with_suggestions(suggestions)
            .with_related(related)
            .build()
        }
        EffectError::MissingEffect { function, missing, source_callee, span } => {
            let call_chain = build_call_chain(function, source_callee, missing, graph);
            let related = build_related_locations(&call_chain);
            let suggestions = build_missing_effect_suggestions(function, missing, *span);

            EffectDiagnosticBuilder::missing_effect(
                function.clone(),
                missing.clone(),
                source_callee.clone(),
                *span,
            )
            .with_call_chain(call_chain)
            .with_suggestions(suggestions)
            .with_related(related)
            .build()
        }
        EffectError::ParameterNotCovered {
            function, effect_name, param_name, required_value,
            declared_value, source_callee, span
        } => {
            let explanation = format!(
                "Function `{}` declares effect `{}` but the parameter `{}` has value {:?}, \
                 while callee `{}` requires value `{}`.",
                function, effect_name, param_name, declared_value, source_callee, required_value
            );

            let suggestion = FixSuggestion::AddEffect {
                effect: format!("{}({}={})", effect_name, param_name, required_value),
                snippet_id: function.clone(),
                location: *span,
                code_snippet: format!("effect {}({}={})", effect_name, param_name, required_value),
            };

            Diagnostic::new(
                crate::diagnostics::DiagnosticKind::Effect(crate::diagnostics::EffectDiagnostic {
                    function: function.clone(),
                    callee: Some(source_callee.clone()),
                    effects: vec![effect_name.clone()],
                    call_chain: Vec::new(),
                }),
                *span,
                "E-EFFECT-003",
                explanation,
            )
            .with_suggestion(suggestion)
        }
    }
}

/// Build the call chain that introduces the effect violation
fn build_call_chain(
    function: &str,
    callee: &str,
    effects: &[String],
    graph: &SymbolGraph,
) -> Vec<CallChainEntry> {
    let mut chain = Vec::new();
    let mut visited = HashSet::new();

    // Start with the calling function
    if let Some(sym) = graph.get_by_name(function) {
        chain.push(CallChainEntry {
            name: function.to_string(),
            effects: sym.declared_effects.iter().map(|e| e.name.clone()).collect(),
            span: sym.span,
        });
    }

    // Find the path to the effect source
    let mut current = callee.to_string();
    while !visited.contains(&current) {
        visited.insert(current.clone());

        if let Some(sym) = graph.get_by_name(&current) {
            let sym_effects: Vec<String> = sym.declared_effects.iter().map(|e| e.name.clone()).collect();

            chain.push(CallChainEntry {
                name: current.clone(),
                effects: sym_effects.clone(),
                span: sym.span,
            });

            // Check if this symbol has any of the missing effects
            if sym_effects.iter().any(|e| effects.contains(e)) {
                break;
            }

            // Follow the first callee that might have the effect
            let mut found_next = false;
            for next_callee in &sym.calls {
                if let Some(next_sym) = graph.get_by_name(next_callee) {
                    let next_effects: HashSet<String> = next_sym.declared_effects
                        .iter()
                        .map(|e| e.name.clone())
                        .collect();
                    if effects.iter().any(|e| next_effects.contains(e)) || !next_effects.is_empty() {
                        current = next_callee.clone();
                        found_next = true;
                        break;
                    }
                }
            }
            if !found_next {
                break;
            }
        } else {
            break;
        }
    }

    chain
}

/// Build related locations from a call chain
fn build_related_locations(call_chain: &[CallChainEntry]) -> Vec<RelatedLocation> {
    call_chain.iter().enumerate().map(|(i, entry)| {
        let label = if i == 0 {
            "function is defined here".to_string()
        } else if !entry.effects.is_empty() {
            format!("declares effects: {}", entry.effects.join(", "))
        } else {
            "called from here".to_string()
        };

        let message = if i == 0 {
            format!("`{}` is pure (declares no effects)", entry.name)
        } else if !entry.effects.is_empty() {
            format!(
                "`{}` requires effects: {}",
                entry.name,
                entry.effects.join(", ")
            )
        } else {
            format!("via `{}`", entry.name)
        };

        RelatedLocation {
            message,
            span: entry.span,
            file: None,
            label,
        }
    }).collect()
}

/// Build fix suggestions for pure-calls-effectful error
fn build_pure_calls_effectful_suggestions(
    function: &str,
    effects: &[String],
    span: Span,
) -> Vec<FixSuggestion> {
    let mut suggestions = Vec::new();

    // Primary suggestion: add the missing effects
    suggestions.push(FixSuggestion::DeclareEffectsSection {
        effects: effects.to_vec(),
        snippet_id: function.to_string(),
    });

    // Alternative: add each effect individually
    for effect in effects {
        suggestions.push(FixSuggestion::AddEffect {
            effect: effect.clone(),
            snippet_id: function.to_string(),
            location: span,
            code_snippet: format!("effect {}", effect),
        });
    }

    // If there's a single direct callee causing the issue, suggest wrapping
    if effects.len() == 1 {
        suggestions.push(FixSuggestion::WrapInEffectfulFunction {
            effects: effects.to_vec(),
            suggested_name: Some(format!("{}_with_effects", function)),
        });
    }

    suggestions
}

/// Build fix suggestions for missing-effect error
fn build_missing_effect_suggestions(
    function: &str,
    missing: &[String],
    span: Span,
) -> Vec<FixSuggestion> {
    let mut suggestions = Vec::new();

    // Primary suggestion: add the missing effects
    for effect in missing {
        suggestions.push(FixSuggestion::AddEffect {
            effect: effect.clone(),
            snippet_id: function.to_string(),
            location: span,
            code_snippet: format!("effect {}", effect),
        });
    }

    suggestions
}

/// Format an effect error as a human-readable explanation string
pub fn format_effect_explanation(error: &EffectError, graph: &SymbolGraph) -> String {
    let diagnostic = explain_effect_violation(error, graph);

    let mut output = String::new();

    // Header with error code
    output.push_str(&format!("{}: {}\n", diagnostic.code, diagnostic.message()));
    output.push('\n');

    // Explanation
    output.push_str("Why this error occurs:\n");
    output.push_str(&format!("  {}\n", diagnostic.explanation));
    output.push('\n');

    // Call chain
    if let crate::diagnostics::DiagnosticKind::Effect(ref effect_diag) = diagnostic.error {
        if !effect_diag.call_chain.is_empty() {
            output.push_str("Call chain:\n");
            for (i, entry) in effect_diag.call_chain.iter().enumerate() {
                let arrow = if i > 0 { "  -> " } else { "     " };
                let effects_str = if entry.effects.is_empty() {
                    "(pure)".to_string()
                } else {
                    format!("[effects: {}]", entry.effects.join(", "))
                };
                output.push_str(&format!("{}  {} {}\n", arrow, entry.name, effects_str));
            }
            output.push('\n');
        }
    }

    // Suggestions
    if !diagnostic.suggestions.is_empty() {
        output.push_str("How to fix:\n");
        for (i, suggestion) in diagnostic.suggestions.iter().enumerate() {
            output.push_str(&format!("  {}. {}\n", i + 1, suggestion.description()));
            if let Some(code) = suggestion.code_snippet() {
                output.push_str(&format!("     ```\n     {}\n     ```\n", code));
            }
        }
    }

    output
}
