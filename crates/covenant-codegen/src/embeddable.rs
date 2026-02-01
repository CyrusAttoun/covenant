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

    /// Required runtime capabilities derived from effect_closure.
    /// Maps effects to concrete WASM imports needed at runtime.
    /// Used by the host to gate imports based on declared effects.
    #[serde(default)]
    pub required_capabilities: Vec<String>,
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
            let effect_closure: Vec<String> = closure
                .map(|c| c.computed.iter().cloned().collect())
                .unwrap_or_default();

            // Derive required capabilities from effect closure
            let required_capabilities = effects_to_capabilities(&effect_closure);

            // Extract effect names from EffectDecl structs
            let effects: Vec<String> = sym.declared_effects
                .iter()
                .map(|e| e.name.clone())
                .collect();

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
                effects,
                effect_closure,
                requirements: sym.requirements.clone(),
                tests: sym.tests.clone(),
                covers: sym.covers.clone(),
                required_capabilities,
            }
        })
        .collect()
}

/// Map effect names to required WASM import capabilities.
///
/// This defines the canonical mapping from Covenant effects to the
/// concrete WASM imports that must be provided by the runtime host.
fn effects_to_capabilities(effects: &[String]) -> Vec<String> {
    let mut capabilities = Vec::new();

    for effect in effects {
        match effect.as_str() {
            // Core effects
            "database" => {
                capabilities.push("db.execute_query".to_string());
            }
            "network" => {
                capabilities.push("http.fetch".to_string());
            }
            "filesystem" => {
                capabilities.push("fs.read".to_string());
                capabilities.push("fs.write".to_string());
                capabilities.push("fs.delete".to_string());
                capabilities.push("fs.exists".to_string());
                capabilities.push("fs.read_dir".to_string());
                capabilities.push("fs.create_dir".to_string());
                capabilities.push("fs.remove_dir".to_string());
            }
            "console" => {
                capabilities.push("console.println".to_string());
                capabilities.push("console.print".to_string());
                capabilities.push("console.eprintln".to_string());
                capabilities.push("console.eprint".to_string());
            }

            // Standard library effects
            "std.storage" => {
                capabilities.push("std.storage.kv.get".to_string());
                capabilities.push("std.storage.kv.set".to_string());
                capabilities.push("std.storage.kv.delete".to_string());
                capabilities.push("std.storage.kv.has".to_string());
                capabilities.push("std.storage.kv.list".to_string());
                capabilities.push("std.storage.kv.clear".to_string());
                capabilities.push("std.storage.doc.put".to_string());
                capabilities.push("std.storage.doc.get".to_string());
                capabilities.push("std.storage.doc.delete".to_string());
                capabilities.push("std.storage.doc.query".to_string());
                capabilities.push("std.storage.doc.count".to_string());
                capabilities.push("std.storage.doc.create_index".to_string());
            }
            "std.time" => {
                capabilities.push("std.time.now".to_string());
                capabilities.push("std.time.sleep".to_string());
            }
            "std.random" => {
                capabilities.push("std.random.int".to_string());
                capabilities.push("std.random.float".to_string());
                capabilities.push("std.random.bytes".to_string());
            }
            "std.crypto" => {
                capabilities.push("std.crypto.hash".to_string());
                capabilities.push("std.crypto.sign".to_string());
                capabilities.push("std.crypto.verify".to_string());
            }

            // Unknown effects - include as-is for forward compatibility
            _ => {
                // Custom or future effects map to themselves
                capabilities.push(effect.clone());
            }
        }
    }

    // Remove duplicates and sort for deterministic output
    capabilities.sort();
    capabilities.dedup();
    capabilities
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
            required_capabilities: vec![
                "console.eprint".to_string(),
                "console.eprintln".to_string(),
                "console.print".to_string(),
                "console.println".to_string(),
            ],
        };

        let json = serde_json::to_string(&symbol).unwrap();
        let parsed: EmbeddableSymbol = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, "test.func");
        assert_eq!(parsed.requirements, vec!["R-001"]);
        assert_eq!(parsed.tests, vec!["T-001"]);
        assert_eq!(parsed.required_capabilities.len(), 4);
        assert!(parsed.required_capabilities.contains(&"console.println".to_string()));
    }

    #[test]
    fn test_effects_to_capabilities() {
        // Database effect
        let caps = effects_to_capabilities(&["database".to_string()]);
        assert_eq!(caps, vec!["db.execute_query"]);

        // Multiple effects
        let caps = effects_to_capabilities(&["database".to_string(), "network".to_string()]);
        assert_eq!(caps, vec!["db.execute_query", "http.fetch"]);

        // Console effect expands to multiple capabilities
        let caps = effects_to_capabilities(&["console".to_string()]);
        assert_eq!(caps.len(), 4);
        assert!(caps.contains(&"console.println".to_string()));
        assert!(caps.contains(&"console.print".to_string()));

        // Unknown effect maps to itself
        let caps = effects_to_capabilities(&["custom.effect".to_string()]);
        assert_eq!(caps, vec!["custom.effect"]);

        // Empty effects produce empty capabilities
        let caps = effects_to_capabilities(&[]);
        assert!(caps.is_empty());
    }
}
