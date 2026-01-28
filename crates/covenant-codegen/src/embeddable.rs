//! Embeddable symbol metadata for WASM embedding
//!
//! This module provides structures and functions for serializing symbol
//! metadata into a format suitable for embedding in WASM modules.

use covenant_checker::EffectCheckResult;
use covenant_symbols::SymbolGraph;
use serde::{Deserialize, Serialize};

/// Symbol metadata optimized for embedding in WASM
///
/// This structure combines information from the SymbolGraph (forward and backward
/// references) with computed data from the effect checker (transitive effect closures).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddableSymbol {
    /// Symbol ID (e.g., "auth.login", "math.factorial")
    pub id: String,

    /// Symbol kind (e.g., "function", "struct", "extern")
    pub kind: String,

    /// Source line number
    pub line: u32,

    /// Functions this symbol calls (forward references)
    pub calls: Vec<String>,

    /// Types this symbol references (forward references)
    pub references: Vec<String>,

    /// Functions that call this symbol (backward references)
    pub called_by: Vec<String>,

    /// Symbols that reference this type (backward references)
    pub referenced_by: Vec<String>,

    /// Declared effects (from effects section)
    pub effects: Vec<String>,

    /// Computed transitive effect closure (includes effects from callees)
    pub effect_closure: Vec<String>,

    /// Requirements declared in this snippet
    pub requirements: Vec<String>,

    /// Tests declared in this snippet
    pub tests: Vec<String>,

    /// For test snippets: requirements this test covers
    pub covers: Vec<String>,
}

/// Build embeddable symbols from a SymbolGraph and EffectCheckResult
///
/// This combines compile-time symbol information with computed effect closures
/// into a serializable format suitable for WASM embedding.
pub fn build_embeddable_symbols(
    graph: &SymbolGraph,
    effect_result: &EffectCheckResult,
) -> Vec<EmbeddableSymbol> {
    graph
        .iter()
        .map(|sym| {
            let closure = effect_result.closures.get(&sym.name);

            EmbeddableSymbol {
                id: sym.name.clone(),
                kind: format!("{:?}", sym.kind).to_lowercase(),
                line: sym.span.start as u32,
                calls: sym.calls.iter().cloned().collect(),
                references: sym.references.iter().cloned().collect(),
                called_by: sym
                    .called_by
                    .iter()
                    .filter_map(|id| graph.get(*id).map(|s| s.name.clone()))
                    .collect(),
                referenced_by: sym
                    .referenced_by
                    .iter()
                    .filter_map(|id| graph.get(*id).map(|s| s.name.clone()))
                    .collect(),
                effects: sym.declared_effects.clone(),
                effect_closure: closure
                    .map(|c| c.computed.iter().cloned().collect())
                    .unwrap_or_default(),
                requirements: sym.requirements.clone(),
                tests: sym.tests.clone(),
                covers: sym.covers.clone(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embeddable_symbol_serialization() {
        let symbol = EmbeddableSymbol {
            id: "test.func".to_string(),
            kind: "function".to_string(),
            line: 42,
            calls: vec!["console.println".to_string()],
            references: vec![],
            called_by: vec![],
            referenced_by: vec![],
            effects: vec!["console".to_string()],
            effect_closure: vec!["console".to_string()],
            requirements: vec!["R-001".to_string()],
            tests: vec!["T-001".to_string()],
            covers: vec![],
        };

        let json = serde_json::to_string(&symbol).unwrap();
        let parsed: EmbeddableSymbol = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, "test.func");
        assert_eq!(parsed.requirements, vec!["R-001"]);
        assert_eq!(parsed.tests, vec!["T-001"]);
    }
}
