//! Pass 2: Backward reference resolution
//!
//! Computes called_by, referenced_by, and relations_from from forward references.

use crate::{RelationRef, SymbolError, SymbolGraph, SymbolId};
use std::collections::HashSet;

/// Inverse relation type mapping
const RELATION_INVERSES: &[(&str, &str)] = &[
    ("to", "from"),
    ("from", "to"),
    ("contains", "contained_by"),
    ("contained_by", "contains"),
    ("describes", "described_by"),
    ("described_by", "describes"),
    ("next", "previous"),
    ("previous", "next"),
    ("supersedes", "precedes"),
    ("precedes", "supersedes"),
    ("causes", "caused_by"),
    ("caused_by", "causes"),
    ("motivates", "enables"),
    ("enables", "motivates"),
    ("implements", "implemented_by"),
    ("implemented_by", "implements"),
    // Symmetric relations (inverse is same type)
    ("elaborates_on", "elaborates_on"),
    ("contrasts_with", "contrasts_with"),
    ("example_of", "example_of"),
    ("related_to", "related_to"),
    ("depends_on", "depends_on"),
    ("version_of", "version_of"),
];

/// Resolves backward references (Pass 2)
pub struct BackwardResolver;

impl BackwardResolver {
    /// Compute backward references for all symbols
    pub fn resolve(graph: &mut SymbolGraph) -> Vec<SymbolError> {
        let mut errors = Vec::new();

        // Collect all symbol IDs and their forward references
        // We need to collect first to avoid borrow issues
        let forwards: Vec<(
            SymbolId,
            String,
            HashSet<String>,
            HashSet<String>,
            Vec<RelationRef>,
            covenant_ast::Span,
        )> = graph
            .iter()
            .map(|s| {
                (
                    s.id,
                    s.name.clone(),
                    s.calls.clone(),
                    s.references.clone(),
                    s.relations_to.clone(),
                    s.span,
                )
            })
            .collect();

        // Process each symbol's forward references
        for (caller_id, caller_name, calls, references, relations, span) in forwards {
            // Resolve calls -> called_by
            for callee_name in &calls {
                if let Some(callee_id) = graph.id_of(callee_name) {
                    if let Some(callee_mut) = graph.get_mut(callee_id) {
                        callee_mut.called_by.insert(caller_id);
                    }
                } else {
                    // Mark as unresolved (soft error)
                    if let Some(caller_mut) = graph.get_mut(caller_id) {
                        caller_mut.unresolved_calls.insert(callee_name.clone());
                    }
                    errors.push(SymbolError::UndefinedReference {
                        name: callee_name.clone(),
                        span,
                        referrer: caller_name.clone(),
                    });
                }
            }

            // Resolve references -> referenced_by
            for ref_name in &references {
                if let Some(ref_id) = graph.id_of(ref_name) {
                    if let Some(ref_mut) = graph.get_mut(ref_id) {
                        ref_mut.referenced_by.insert(caller_id);
                    }
                } else {
                    // Mark as unresolved (soft error)
                    if let Some(caller_mut) = graph.get_mut(caller_id) {
                        caller_mut.unresolved_references.insert(ref_name.clone());
                    }
                    errors.push(SymbolError::UndefinedReference {
                        name: ref_name.clone(),
                        span,
                        referrer: caller_name.clone(),
                    });
                }
            }

            // Resolve relations -> relations_from (with inverse types)
            for rel in &relations {
                if let Some(target_id) = graph.id_of(&rel.target) {
                    let inverse_type = get_inverse_relation(&rel.relation_type);

                    if let Some(target_mut) = graph.get_mut(target_id) {
                        target_mut.relations_from.push(RelationRef {
                            target: caller_name.clone(),
                            relation_type: inverse_type,
                        });
                    }
                } else {
                    errors.push(SymbolError::RelationTargetNotFound {
                        target: rel.target.clone(),
                        span,
                        from_symbol: caller_name.clone(),
                    });
                }
            }
        }

        errors
    }
}

/// Get the inverse relation type
fn get_inverse_relation(rel_type: &str) -> String {
    RELATION_INVERSES
        .iter()
        .find(|(from, _)| *from == rel_type)
        .map(|(_, to)| to.to_string())
        .unwrap_or_else(|| rel_type.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_inverse_relation() {
        assert_eq!(get_inverse_relation("to"), "from");
        assert_eq!(get_inverse_relation("from"), "to");
        assert_eq!(get_inverse_relation("contains"), "contained_by");
        assert_eq!(get_inverse_relation("related_to"), "related_to");
        assert_eq!(get_inverse_relation("unknown"), "unknown");
    }
}
