//! Effect tracking

use std::collections::{HashMap, HashSet};
use covenant_ast::{SymbolId, EffectId};

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
