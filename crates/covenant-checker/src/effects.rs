//! Effect tracking and validation (Phase 3)
//!
//! This module handles:
//! - Effect registration and lookup (EffectTable)
//! - Transitive effect closure computation
//! - Effect declaration validation (I2 invariant)

use std::collections::{HashMap, HashSet};
use covenant_ast::{SymbolId, EffectId, Span};
use covenant_symbols::{SymbolGraph, SymbolInfo};

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
    /// Effects declared in the effects section
    pub declared: HashSet<String>,
    /// Computed transitive closure (includes effects from callees)
    pub computed: HashSet<String>,
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
    let declared: HashSet<String> = symbol.declared_effects.iter().cloned().collect();
    let is_pure = declared.is_empty();

    let mut computed = HashSet::new();
    let mut visited = HashSet::new();

    collect_transitive_effects(symbol, graph, &mut visited, &mut computed);

    EffectClosure { declared, computed, is_pure }
}

/// Recursively collect effects from callees
fn collect_transitive_effects(
    symbol: &SymbolInfo,
    graph: &SymbolGraph,
    visited: &mut HashSet<String>,
    effects: &mut HashSet<String>,
) {
    if !visited.insert(symbol.name.clone()) {
        return; // Already visited (handles cycles)
    }

    // Add this symbol's declared effects
    effects.extend(symbol.declared_effects.iter().cloned());

    // Recurse into callees
    for callee_name in &symbol.calls {
        if let Some(callee) = graph.get_by_name(callee_name) {
            collect_transitive_effects(callee, graph, visited, effects);
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
    // Find effects in computed but not in declared
    let missing: Vec<String> = closure.computed
        .difference(&closure.declared)
        .cloned()
        .collect();

    if missing.is_empty() {
        return None;
    }

    // Find which callee introduced these effects (for error message)
    let source_callee = find_effect_source(symbol, &missing, graph)
        .unwrap_or_else(|| "unknown".to_string());

    if closure.is_pure {
        Some(EffectError::PureCallsEffectful {
            function: symbol.name.clone(),
            callee: source_callee,
            effects: missing,
            span: symbol.span,
        })
    } else {
        Some(EffectError::MissingEffect {
            function: symbol.name.clone(),
            missing,
            source_callee,
            span: symbol.span,
        })
    }
}

/// Find which callee introduced a missing effect (for diagnostics)
fn find_effect_source(symbol: &SymbolInfo, missing: &[String], graph: &SymbolGraph) -> Option<String> {
    for callee_name in &symbol.calls {
        if let Some(callee) = graph.get_by_name(callee_name) {
            for effect in &callee.declared_effects {
                if missing.contains(effect) {
                    return Some(callee_name.clone());
                }
            }
        }
    }
    // If not found in direct callees, it might be transitive
    // In that case, we could do a deeper search, but for now return None
    None
}
