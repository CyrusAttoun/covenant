//! Data Graph - Compile-time graph structure for embedding into WASM
//!
//! Collects `kind="data"` snippets, computes inverse relations, and generates
//! adjacency indexes for the Graph Access Interface (GAI).

use std::collections::HashMap;

use covenant_ast::{RelationKind, Section, Snippet, SnippetKind};

/// A node in the data graph (extracted from a kind="data" snippet)
#[derive(Debug, Clone)]
pub struct DataNode {
    pub id: String,
    pub kind: String, // "data", "fn", "struct", etc. - for query filtering
    pub content: String,
    pub notes: Vec<String>,
    pub metadata: Vec<(String, String)>,
}

/// A directed edge in the data graph
#[derive(Debug, Clone)]
pub struct DataRelation {
    pub from_idx: usize,
    pub to_idx: usize,
    pub rel_type: String,
}

/// Per-node adjacency information (offsets into the sorted relations tables)
#[derive(Debug, Clone, Default)]
pub struct AdjacencyEntry {
    pub out_start: usize,
    pub out_count: usize,
    pub in_start: usize,
    pub in_count: usize,
}

/// The complete data graph ready for embedding into WASM
#[derive(Debug)]
pub struct DataGraph {
    pub nodes: Vec<DataNode>,
    pub id_to_index: HashMap<String, usize>,
    /// Outgoing relations sorted by from_idx
    pub outgoing: Vec<DataRelation>,
    /// Incoming relations sorted by to_idx
    pub incoming: Vec<DataRelation>,
    /// Per-node adjacency index
    pub adjacency: Vec<AdjacencyEntry>,
    /// All distinct relation type strings
    pub relation_types: Vec<String>,
    /// Mapping from relation type string to its enum index
    pub rel_type_to_idx: HashMap<String, u8>,
}

/// Inverse relation type mapping
const RELATION_INVERSES: &[(&str, &str)] = &[
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
    // Symmetric relations
    ("elaborates_on", "elaborates_on"),
    ("contrasts_with", "contrasts_with"),
    ("example_of", "example_of"),
    ("related_to", "related_to"),
    ("depends_on", "depends_on"),
    ("version_of", "version_of"),
];

fn get_inverse_relation(rel_type: &str) -> String {
    RELATION_INVERSES
        .iter()
        .find(|(from, _)| *from == rel_type)
        .map(|(_, to)| to.to_string())
        .unwrap_or_else(|| rel_type.to_string())
}

impl DataGraph {
    /// Build a DataGraph from a list of parsed snippets.
    /// Only `kind="data"` snippets are included as nodes.
    /// All snippets (including non-data) are indexed for relation resolution.
    pub fn from_snippets(snippets: &[Snippet]) -> Self {
        let mut nodes = Vec::new();
        let mut id_to_index: HashMap<String, usize> = HashMap::new();

        // Pass 1: Collect all data nodes
        for snippet in snippets {
            if snippet.kind != SnippetKind::Data {
                continue;
            }

            let id = snippet.id.clone();
            let mut content = String::new();
            let mut notes = Vec::new();
            let mut metadata = Vec::new();

            for section in &snippet.sections {
                match section {
                    Section::Content(c) => {
                        content = c.content.clone();
                    }
                    Section::Metadata(m) => {
                        for entry in &m.entries {
                            metadata.push((entry.key.clone(), entry.value.clone()));
                        }
                    }
                    _ => {}
                }
            }

            // Extract notes from snippet
            for note in &snippet.notes {
                notes.push(note.content.clone());
            }

            let idx = nodes.len();
            id_to_index.insert(id.clone(), idx);
            // Add kind to metadata for query filtering
            metadata.push(("kind".to_string(), "data".to_string()));
            nodes.push(DataNode {
                id,
                kind: "data".to_string(),
                content,
                notes,
                metadata,
            });
        }

        // Also index non-data snippets so relations can reference them
        // (e.g., a data node describing a function)
        for snippet in snippets {
            if snippet.kind == SnippetKind::Data {
                continue; // Already indexed
            }
            let id = snippet.id.clone();
            if !id_to_index.contains_key(&id) {
                // Add as a node with empty content (for relation resolution only)
                let idx = nodes.len();
                id_to_index.insert(id.clone(), idx);
                let kind_str = match snippet.kind {
                    SnippetKind::Function => "fn",
                    SnippetKind::Struct => "struct",
                    SnippetKind::Database => "database",
                    SnippetKind::Extern => "extern",
                    SnippetKind::Enum => "enum",
                    SnippetKind::Module => "module",
                    SnippetKind::ExternAbstract => "extern_abstract",
                    SnippetKind::ExternImpl => "extern_impl",
                    SnippetKind::Test => "test",
                    SnippetKind::Data => "data", // Won't happen due to continue above
                };
                let metadata = vec![("kind".to_string(), kind_str.to_string())];
                nodes.push(DataNode {
                    id,
                    kind: kind_str.to_string(),
                    content: String::new(),
                    notes: Vec::new(),
                    metadata,
                });
            }
        }

        // Pass 2: Collect declared relations and compute inverses
        let mut raw_relations: Vec<DataRelation> = Vec::new();

        for snippet in snippets {
            let from_id = &snippet.id;
            let from_idx = match id_to_index.get(from_id) {
                Some(&idx) => idx,
                None => continue,
            };

            for section in &snippet.sections {
                if let Section::Relations(rels) = section {
                    for rel in &rels.relations {
                        let target_idx = match id_to_index.get(&rel.target) {
                            Some(&idx) => idx,
                            None => continue, // Dangling reference, skip
                        };

                        let rel_type = rel.rel_type.clone().unwrap_or_else(|| {
                            match rel.kind {
                                RelationKind::To => "related_to".to_string(),
                                RelationKind::From => "related_to".to_string(),
                            }
                        });

                        match rel.kind {
                            RelationKind::To => {
                                // Forward: from_idx -> target_idx with rel_type
                                raw_relations.push(DataRelation {
                                    from_idx,
                                    to_idx: target_idx,
                                    rel_type: rel_type.clone(),
                                });
                                // Inverse: target_idx -> from_idx with inverse type
                                let inverse = get_inverse_relation(&rel_type);
                                raw_relations.push(DataRelation {
                                    from_idx: target_idx,
                                    to_idx: from_idx,
                                    rel_type: inverse,
                                });
                            }
                            RelationKind::From => {
                                // "from" means: the declaring snippet has rel_type FROM target
                                // e.g., B says `rel from="A" type=contained_by` means B is contained_by A
                                // Canonical forward edge: target -> declaring (with inverse type)
                                // e.g., A -> B with "contains"
                                let inverse = get_inverse_relation(&rel_type);
                                raw_relations.push(DataRelation {
                                    from_idx: target_idx,
                                    to_idx: from_idx,
                                    rel_type: inverse.clone(),
                                });
                                // Backward edge: declaring -> target (with stated type)
                                // e.g., B -> A with "contained_by"
                                raw_relations.push(DataRelation {
                                    from_idx,
                                    to_idx: target_idx,
                                    rel_type: rel_type.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Deduplicate relations (same from, to, type)
        raw_relations.sort_by(|a, b| {
            a.from_idx.cmp(&b.from_idx)
                .then(a.to_idx.cmp(&b.to_idx))
                .then(a.rel_type.cmp(&b.rel_type))
        });
        raw_relations.dedup_by(|a, b| {
            a.from_idx == b.from_idx && a.to_idx == b.to_idx && a.rel_type == b.rel_type
        });

        // Build relation type index
        let mut relation_types: Vec<String> = Vec::new();
        let mut rel_type_to_idx: HashMap<String, u8> = HashMap::new();
        for rel in &raw_relations {
            if !rel_type_to_idx.contains_key(&rel.rel_type) {
                let idx = relation_types.len() as u8;
                rel_type_to_idx.insert(rel.rel_type.clone(), idx);
                relation_types.push(rel.rel_type.clone());
            }
        }

        // Build outgoing list (sorted by from_idx)
        let mut outgoing = raw_relations.clone();
        outgoing.sort_by_key(|r| (r.from_idx, r.to_idx));

        // Build incoming list (sorted by to_idx)
        let mut incoming = raw_relations;
        incoming.sort_by_key(|r| (r.to_idx, r.from_idx));

        // Build adjacency index
        let mut adjacency: Vec<AdjacencyEntry> = vec![AdjacencyEntry::default(); nodes.len()];

        // Outgoing offsets
        let mut i = 0;
        for node_idx in 0..nodes.len() {
            let start = i;
            while i < outgoing.len() && outgoing[i].from_idx == node_idx {
                i += 1;
            }
            adjacency[node_idx].out_start = start;
            adjacency[node_idx].out_count = i - start;
        }

        // Incoming offsets
        let mut i = 0;
        for node_idx in 0..nodes.len() {
            let start = i;
            while i < incoming.len() && incoming[i].to_idx == node_idx {
                i += 1;
            }
            adjacency[node_idx].in_start = start;
            adjacency[node_idx].in_count = i - start;
        }

        DataGraph {
            nodes,
            id_to_index,
            outgoing,
            incoming,
            adjacency,
            relation_types,
            rel_type_to_idx,
        }
    }

    /// Number of data nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get outgoing relations for a node
    pub fn outgoing_for(&self, node_idx: usize) -> &[DataRelation] {
        let adj = &self.adjacency[node_idx];
        &self.outgoing[adj.out_start..adj.out_start + adj.out_count]
    }

    /// Get incoming relations for a node
    pub fn incoming_for(&self, node_idx: usize) -> &[DataRelation] {
        let adj = &self.adjacency[node_idx];
        &self.incoming[adj.in_start..adj.in_start + adj.in_count]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use covenant_ast::*;

    fn make_data_snippet(id: &str, content: &str, relations: Vec<RelationDecl>) -> Snippet {
        let mut sections = Vec::new();
        if !content.is_empty() {
            sections.push(Section::Content(ContentSection {
                content: content.to_string(),
                span: Span::default(),
            }));
        }
        if !relations.is_empty() {
            sections.push(Section::Relations(RelationsSection {
                relations,
                span: Span::default(),
            }));
        }
        Snippet {
            id: id.to_string(),
            kind: SnippetKind::Data,
            notes: vec![],
            sections,
            implements: None,
            platform: None,
            span: Span::default(),
        }
    }

    fn rel_to(target: &str, rel_type: &str) -> RelationDecl {
        RelationDecl {
            kind: RelationKind::To,
            target: target.to_string(),
            rel_type: Some(rel_type.to_string()),
            span: Span::default(),
        }
    }

    #[test]
    fn test_basic_graph_construction() {
        let snippets = vec![
            make_data_snippet("kb.root", "Root node", vec![
                rel_to("kb.child1", "contains"),
                rel_to("kb.child2", "contains"),
            ]),
            make_data_snippet("kb.child1", "Child 1", vec![]),
            make_data_snippet("kb.child2", "Child 2", vec![
                rel_to("kb.child1", "related_to"),
            ]),
        ];

        let graph = DataGraph::from_snippets(&snippets);

        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.id_to_index["kb.root"], 0);
        assert_eq!(graph.id_to_index["kb.child1"], 1);
        assert_eq!(graph.id_to_index["kb.child2"], 2);
    }

    #[test]
    fn test_inverse_relations_computed() {
        let snippets = vec![
            make_data_snippet("a", "Node A", vec![
                rel_to("b", "describes"),
            ]),
            make_data_snippet("b", "Node B", vec![]),
        ];

        let graph = DataGraph::from_snippets(&snippets);

        // a -> b (describes)
        let a_out = graph.outgoing_for(0);
        assert_eq!(a_out.len(), 1);
        assert_eq!(a_out[0].to_idx, 1);
        assert_eq!(a_out[0].rel_type, "describes");

        // b -> a (described_by) - auto-computed inverse
        let b_out = graph.outgoing_for(1);
        assert_eq!(b_out.len(), 1);
        assert_eq!(b_out[0].to_idx, 0);
        assert_eq!(b_out[0].rel_type, "described_by");
    }

    #[test]
    fn test_incoming_relations() {
        let snippets = vec![
            make_data_snippet("a", "Node A", vec![
                rel_to("b", "contains"),
            ]),
            make_data_snippet("b", "Node B", vec![]),
        ];

        let graph = DataGraph::from_snippets(&snippets);

        // b has incoming "contains" from a
        let b_in = graph.incoming_for(1);
        assert_eq!(b_in.len(), 1);
        assert_eq!(b_in[0].from_idx, 0);
        assert_eq!(b_in[0].rel_type, "contains");
    }

    #[test]
    fn test_deduplication() {
        // Both declare the same edge -- should deduplicate
        let snippets = vec![
            make_data_snippet("a", "Node A", vec![
                rel_to("b", "contains"),
            ]),
            make_data_snippet("b", "Node B", vec![
                RelationDecl {
                    kind: RelationKind::From,
                    target: "a".to_string(),
                    rel_type: Some("contained_by".to_string()),
                    span: Span::default(),
                },
            ]),
        ];

        let graph = DataGraph::from_snippets(&snippets);

        // Should have exactly 2 outgoing total (a->b contains, b->a contained_by)
        // not 4 (which would happen without dedup)
        let a_out = graph.outgoing_for(0);
        let b_out = graph.outgoing_for(1);
        assert_eq!(a_out.len(), 1); // a->b contains
        assert_eq!(b_out.len(), 1); // b->a contained_by
    }

    #[test]
    fn test_relation_type_index() {
        let snippets = vec![
            make_data_snippet("a", "", vec![
                rel_to("b", "contains"),
                rel_to("c", "describes"),
            ]),
            make_data_snippet("b", "", vec![]),
            make_data_snippet("c", "", vec![]),
        ];

        let graph = DataGraph::from_snippets(&snippets);

        // Should have 4 relation types: contains, contained_by, describes, described_by
        assert!(graph.relation_types.contains(&"contains".to_string()));
        assert!(graph.relation_types.contains(&"contained_by".to_string()));
        assert!(graph.relation_types.contains(&"describes".to_string()));
        assert!(graph.relation_types.contains(&"described_by".to_string()));
    }
}
