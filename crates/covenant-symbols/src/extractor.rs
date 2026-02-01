//! Pass 1: Symbol extraction from AST
//!
//! Extracts symbols and forward references from snippets.

use crate::{RelationRef, SymbolError, SymbolGraph, SymbolInfo, SymbolKind};
use covenant_ast::{
    BodySection, EffectDecl, EffectsSection, RelationsSection, RequiresSection, ReturnType, ReturnValue,
    Section, SignatureKind, SignatureSection, Snippet, SnippetKind, Step, StepKind, TestsSection,
    Type, TypeKind,
};
use std::collections::HashSet;

/// Extracts symbols and forward references from snippets (Pass 1)
pub struct SymbolExtractor {
    errors: Vec<SymbolError>,
}

impl SymbolExtractor {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Extract all symbols from snippets into a symbol graph
    pub fn extract(mut self, snippets: &[Snippet]) -> (SymbolGraph, Vec<SymbolError>) {
        let mut graph = SymbolGraph::new();

        for snippet in snippets {
            let symbol = self.extract_snippet(snippet);
            if let Err(e) = graph.insert(symbol) {
                self.errors.push(e);
            }
        }

        (graph, self.errors)
    }

    /// Extract a single snippet into a SymbolInfo
    fn extract_snippet(&self, snippet: &Snippet) -> SymbolInfo {
        let mut symbol = SymbolInfo::new(
            snippet.id.clone(),
            SymbolKind::from(snippet.kind),
            snippet.span,
        );

        // Handle extern-impl specific fields
        if snippet.kind == SnippetKind::ExternImpl {
            symbol.implements = snippet.implements.clone();
            symbol.target_platform = snippet.platform.clone();

            // Add the abstract snippet as a reference
            if let Some(ref implements) = snippet.implements {
                symbol.references.insert(implements.clone());
            }
        }

        // Process each section
        for section in &snippet.sections {
            match section {
                Section::Effects(effects) => {
                    symbol.declared_effects = self.extract_effects(effects);
                }
                Section::Signature(sig) => {
                    let type_refs = self.extract_signature_types(sig);
                    symbol.references.extend(type_refs);
                }
                Section::Body(body) => {
                    let (body_calls, body_refs) = self.extract_body_refs(body);
                    symbol.calls.extend(body_calls);
                    symbol.references.extend(body_refs);
                }
                Section::Relations(rels) => {
                    symbol.relations_to = self.extract_relations(rels);
                }
                Section::Requires(reqs) => {
                    symbol.requirements = self.extract_requirements(reqs);
                }
                Section::Tests(tests) => {
                    let (test_ids, covers, test_calls, test_refs) = self.extract_tests_full(tests);
                    symbol.tests = test_ids;
                    symbol.covers = covers;
                    symbol.calls.extend(test_calls);
                    symbol.references.extend(test_refs);
                }
                _ => {} // Other sections handled in later passes
            }
        }

        symbol
    }

    /// Extract effect declarations from effects section
    fn extract_effects(&self, effects: &EffectsSection) -> Vec<EffectDecl> {
        effects.effects.clone()
    }

    /// Extract requirement IDs from requires section
    fn extract_requirements(&self, reqs: &RequiresSection) -> Vec<String> {
        reqs.requirements.iter().map(|r| r.id.clone()).collect()
    }

    /// Extract test IDs, covers, and references from tests section
    fn extract_tests_full(
        &self,
        tests: &TestsSection,
    ) -> (Vec<String>, Vec<String>, HashSet<String>, HashSet<String>) {
        let mut test_ids = Vec::new();
        let mut covers = Vec::new();
        let mut calls = HashSet::new();
        let mut refs = HashSet::new();

        for test in &tests.tests {
            test_ids.push(test.id.clone());
            covers.extend(test.covers.iter().cloned());

            let (test_calls, test_refs) = self.extract_steps_refs(&test.steps);
            calls.extend(test_calls);
            refs.extend(test_refs);
        }

        (test_ids, covers, calls, refs)
    }

    /// Extract type references from signature
    fn extract_signature_types(&self, sig: &SignatureSection) -> HashSet<String> {
        let mut refs = HashSet::new();

        match &sig.kind {
            SignatureKind::Function(fn_sig) => {
                // Parameter types
                for param in &fn_sig.params {
                    self.collect_type_refs(&param.ty, &mut refs);
                }
                // Return type
                if let Some(ret) = &fn_sig.returns {
                    self.collect_return_type_refs(ret, &mut refs);
                }
            }
            SignatureKind::Struct(struct_sig) => {
                for field in &struct_sig.fields {
                    self.collect_type_refs(&field.ty, &mut refs);
                }
            }
            SignatureKind::Enum(enum_sig) => {
                for variant in &enum_sig.variants {
                    if let Some(fields) = &variant.fields {
                        for field in fields {
                            self.collect_type_refs(&field.ty, &mut refs);
                        }
                    }
                }
            }
        }

        refs
    }

    /// Recursively collect type references from a type
    fn collect_type_refs(&self, ty: &Type, refs: &mut HashSet<String>) {
        match &ty.kind {
            TypeKind::Named(path) => {
                // Only add non-primitive types
                let name = path.name();
                if !is_primitive_type(name) {
                    refs.insert(name.to_string());
                }
                // Also check generic arguments
                for generic in &path.generics {
                    self.collect_type_refs(generic, refs);
                }
            }
            TypeKind::Optional(inner) => self.collect_type_refs(inner, refs),
            TypeKind::List(inner) => self.collect_type_refs(inner, refs),
            TypeKind::Union(types) => {
                for t in types {
                    self.collect_type_refs(t, refs);
                }
            }
            TypeKind::Tuple(types) => {
                for t in types {
                    self.collect_type_refs(t, refs);
                }
            }
            TypeKind::Function { params, ret } => {
                for p in params {
                    self.collect_type_refs(p, refs);
                }
                self.collect_type_refs(ret, refs);
            }
            TypeKind::Struct(fields) => {
                for f in fields {
                    self.collect_type_refs(&f.ty, refs);
                }
            }
        }
    }

    /// Collect type refs from return type
    fn collect_return_type_refs(&self, ret: &ReturnType, refs: &mut HashSet<String>) {
        match ret {
            ReturnType::Single { ty, .. } => self.collect_type_refs(ty, refs),
            ReturnType::Collection { of } => self.collect_type_refs(of, refs),
            ReturnType::Union { types } => {
                for member in types {
                    self.collect_type_refs(&member.ty, refs);
                }
            }
        }
    }

    /// Extract call and type references from body
    fn extract_body_refs(&self, body: &BodySection) -> (HashSet<String>, HashSet<String>) {
        self.extract_steps_refs(&body.steps)
    }

    /// Extract references from a list of steps
    fn extract_steps_refs(&self, steps: &[Step]) -> (HashSet<String>, HashSet<String>) {
        let mut calls = HashSet::new();
        let mut refs = HashSet::new();

        for step in steps {
            self.extract_step_refs(step, &mut calls, &mut refs);
        }

        (calls, refs)
    }

    /// Extract references from a single step
    fn extract_step_refs(
        &self,
        step: &Step,
        calls: &mut HashSet<String>,
        refs: &mut HashSet<String>,
    ) {
        match &step.kind {
            StepKind::Call(call) => {
                calls.insert(call.fn_name.clone());
                // Check handle block for nested calls
                if let Some(handle) = &call.handle {
                    for case in &handle.cases {
                        // Handle case error_type may be a type reference
                        if !is_primitive_type(&case.error_type) {
                            refs.insert(case.error_type.clone());
                        }
                        let (nested_calls, nested_refs) = self.extract_steps_refs(&case.steps);
                        calls.extend(nested_calls);
                        refs.extend(nested_refs);
                    }
                }
            }
            StepKind::If(if_step) => {
                let (then_calls, then_refs) = self.extract_steps_refs(&if_step.then_steps);
                calls.extend(then_calls);
                refs.extend(then_refs);

                if let Some(else_steps) = &if_step.else_steps {
                    let (else_calls, else_refs) = self.extract_steps_refs(else_steps);
                    calls.extend(else_calls);
                    refs.extend(else_refs);
                }
            }
            StepKind::Match(match_step) => {
                for case in &match_step.cases {
                    let (case_calls, case_refs) = self.extract_steps_refs(&case.steps);
                    calls.extend(case_calls);
                    refs.extend(case_refs);
                }
            }
            StepKind::For(for_step) => {
                let (body_calls, body_refs) = self.extract_steps_refs(&for_step.steps);
                calls.extend(body_calls);
                refs.extend(body_refs);
            }
            StepKind::Transaction(tx) => {
                let (tx_calls, tx_refs) = self.extract_steps_refs(&tx.steps);
                calls.extend(tx_calls);
                refs.extend(tx_refs);
            }
            StepKind::Return(ret) => {
                // Check for struct/variant type references in return
                match &ret.value {
                    ReturnValue::Struct(s) => {
                        self.collect_type_refs(&s.ty, refs);
                    }
                    ReturnValue::Variant(v) => {
                        // Extract type from variant name (e.g., "ParseError::MissingField")
                        // or just use the full type name
                        if !is_primitive_type(&v.ty) {
                            // Try to extract the enum name from variant path
                            if let Some(type_name) = v.ty.split("::").next() {
                                if !is_primitive_type(type_name) {
                                    refs.insert(type_name.to_string());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            StepKind::Query(query) => {
                // Query target might be a database binding reference
                if !query.target.is_empty() && query.target != "project" {
                    refs.insert(query.target.clone());
                }
            }
            StepKind::Insert(insert) => {
                // Insert target might be a type reference
                if !insert.target.is_empty() {
                    refs.insert(insert.target.clone());
                }
            }
            StepKind::Update(update) => {
                if !update.target.is_empty() {
                    refs.insert(update.target.clone());
                }
            }
            StepKind::Delete(delete) => {
                if !delete.target.is_empty() {
                    refs.insert(delete.target.clone());
                }
            }
            StepKind::Traverse(traverse) => {
                if !traverse.target.is_empty() {
                    refs.insert(traverse.target.clone());
                }
            }
            StepKind::Construct(construct) => {
                // Struct construction references the struct type
                self.collect_type_refs(&construct.ty, refs);
            }
            // Compute and Bind don't introduce new calls or type refs
            StepKind::Compute(_) | StepKind::Bind(_) => {}
            StepKind::Parallel(parallel) => {
                // Recurse into branches
                for branch in &parallel.branches {
                    let (branch_calls, branch_refs) = self.extract_steps_refs(&branch.steps);
                    calls.extend(branch_calls);
                    refs.extend(branch_refs);
                }
            }
            StepKind::Race(race) => {
                // Recurse into branches
                for branch in &race.branches {
                    let (branch_calls, branch_refs) = self.extract_steps_refs(&branch.steps);
                    calls.extend(branch_calls);
                    refs.extend(branch_refs);
                }
            }
        }
    }

    /// Extract relations
    fn extract_relations(&self, rels: &RelationsSection) -> Vec<RelationRef> {
        rels.relations
            .iter()
            .map(|rel| RelationRef {
                target: rel.target.clone(),
                relation_type: rel.rel_type.clone().unwrap_or_else(|| {
                    format!("{:?}", rel.kind).to_lowercase()
                }),
                direction: rel.kind,
            })
            .collect()
    }
}

impl Default for SymbolExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a type name is a primitive
fn is_primitive_type(name: &str) -> bool {
    matches!(name, "Int" | "Float" | "Bool" | "String" | "None" | "Void")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_primitive_type() {
        assert!(is_primitive_type("Int"));
        assert!(is_primitive_type("String"));
        assert!(is_primitive_type("Bool"));
        assert!(is_primitive_type("Float"));
        assert!(is_primitive_type("None"));
        assert!(!is_primitive_type("User"));
        assert!(!is_primitive_type("DbError"));
    }
}
