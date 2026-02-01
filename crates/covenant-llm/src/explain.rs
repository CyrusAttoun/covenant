//! Explanation generator for Covenant snippets

use chrono::Utc;
use sha2::{Digest, Sha256};

use covenant_ast::{
    BodySection, EffectsSection, FunctionSignature, RequiresSection, ReturnType, Section,
    SignatureKind, SignatureSection, Snippet, SnippetKind, Step, StepKind, TestsSection,
    Type, TypeKind,
};

use crate::cache::ExplanationCache;
use crate::prompts::{build_explain_prompt, EXPLAIN_SYSTEM_PROMPT};
use crate::types::{Explanation, ExplainFormat, Pattern, SnippetMetadata, Verbosity};
use crate::{LlmClient, LlmError};

/// Generator for AI explanations of Covenant code
pub struct ExplainGenerator {
    llm: LlmClient,
    cache: Option<ExplanationCache>,
}

/// Error type for explanation generation
#[derive(Debug, thiserror::Error)]
pub enum ExplainError {
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Cache error: {0}")]
    Cache(String),
    #[error("Invalid snippet: {0}")]
    InvalidSnippet(String),
}

impl ExplainGenerator {
    /// Create a new generator with an LLM client
    pub fn new(llm: LlmClient) -> Self {
        Self { llm, cache: None }
    }

    /// Create a generator with caching enabled
    pub fn with_cache(llm: LlmClient, cache: ExplanationCache) -> Self {
        Self {
            llm,
            cache: Some(cache),
        }
    }

    /// Generate an explanation for a snippet
    pub async fn explain(
        &self,
        snippet: &Snippet,
        code: &str,
        verbosity: Verbosity,
    ) -> Result<Explanation, ExplainError> {
        let content_hash = Self::hash_content(code);

        // Check cache first
        if let Some(ref cache) = self.cache {
            if let Some(cached) = cache.get(&snippet.id, &content_hash) {
                return Ok(cached);
            }
        }

        // Extract metadata from AST
        let meta = self.extract_metadata(snippet);

        // Detect patterns
        let patterns = self.detect_patterns(snippet);

        // Build prompt
        let user_prompt = build_explain_prompt(&meta, &patterns, code, verbosity);

        // Call LLM
        let response = self
            .llm
            .generate_explanation(EXPLAIN_SYSTEM_PROMPT, &user_prompt)
            .await?;

        // Parse response as JSON
        let mut explanation: Explanation = self.parse_response(&response, &meta)?;

        // Fill in metadata fields
        explanation.snippet_id = snippet.id.clone();
        explanation.kind = snippet_kind_str(snippet.kind);
        explanation.generated_at = Utc::now();
        explanation.generator_version = Some("0.1.0".to_string());
        explanation.snippet_hash = Some(content_hash.clone());

        // Cache the result
        if let Some(ref cache) = self.cache {
            cache.put(&snippet.id, &content_hash, &explanation);
        }

        Ok(explanation)
    }

    /// Extract metadata from a snippet for prompt building
    fn extract_metadata(&self, snippet: &Snippet) -> SnippetMetadata {
        let mut meta = SnippetMetadata {
            id: snippet.id.clone(),
            kind: snippet_kind_str(snippet.kind),
            effects: Vec::new(),
            params: Vec::new(),
            return_type: None,
            step_count: 0,
            step_kinds: Vec::new(),
            requirements: Vec::new(),
            test_count: 0,
        };

        for section in &snippet.sections {
            match section {
                Section::Effects(EffectsSection { effects, .. }) => {
                    meta.effects = effects.iter().map(|e| e.name.clone()).collect();
                }
                Section::Signature(SignatureSection { kind, .. }) => {
                    if let SignatureKind::Function(FunctionSignature {
                        params, returns, ..
                    }) = kind
                    {
                        meta.params = params
                            .iter()
                            .map(|p| (p.name.clone(), type_to_string(&p.ty)))
                            .collect();
                        meta.return_type = returns.as_ref().map(return_type_to_string);
                    }
                }
                Section::Body(BodySection { steps, .. }) => {
                    meta.step_count = steps.len();
                    meta.step_kinds = steps.iter().map(|s| step_kind_str(&s.kind)).collect();
                }
                Section::Requires(RequiresSection { requirements, .. }) => {
                    meta.requirements = requirements
                        .iter()
                        .map(|r| (r.id.clone(), r.text.clone()))
                        .collect();
                }
                Section::Tests(TestsSection { tests, .. }) => {
                    meta.test_count = tests.len();
                }
                _ => {}
            }
        }

        meta
    }

    /// Detect common patterns in the snippet
    fn detect_patterns(&self, snippet: &Snippet) -> Vec<Pattern> {
        let mut patterns = Vec::new();

        // Get body steps if present
        let steps: Vec<&Step> = snippet
            .sections
            .iter()
            .filter_map(|s| match s {
                Section::Body(b) => Some(&b.steps),
                _ => None,
            })
            .flatten()
            .collect();

        if steps.is_empty() {
            return patterns;
        }

        // Check for pure computation (no effects)
        let has_effects = snippet.sections.iter().any(|s| {
            matches!(s, Section::Effects(e) if !e.effects.is_empty())
        });
        if !has_effects {
            patterns.push(Pattern::PureComputation);
        }

        // Check for query-then-return pattern
        let has_query = steps.iter().any(|s| matches!(s.kind, StepKind::Query(_)));
        let ends_with_return = steps.last().map_or(false, |s| matches!(s.kind, StepKind::Return(_)));
        if has_query && ends_with_return && steps.len() <= 3 {
            patterns.push(Pattern::QueryThenReturn);
        }

        // Check for CRUD operations
        let has_crud = steps.iter().any(|s| {
            matches!(
                s.kind,
                StepKind::Insert(_) | StepKind::Update(_) | StepKind::Delete(_)
            )
        });
        if has_crud {
            patterns.push(Pattern::Crud);
        }

        // Check for iteration
        let has_iteration = steps.iter().any(|s| matches!(s.kind, StepKind::For(_)));
        if has_iteration {
            patterns.push(Pattern::Iteration);
        }

        // Check for error propagation (match with error handling)
        let has_match = steps.iter().any(|s| matches!(s.kind, StepKind::Match(_)));
        if has_match {
            patterns.push(Pattern::ErrorPropagation);
        }

        // Check for validation pattern (if early in function)
        let has_early_if = steps
            .iter()
            .take(2)
            .any(|s| matches!(s.kind, StepKind::If(_)));
        if has_early_if {
            patterns.push(Pattern::ValidateAndTransform);
        }

        patterns
    }

    /// Parse LLM response into Explanation struct
    fn parse_response(
        &self,
        response: &str,
        meta: &SnippetMetadata,
    ) -> Result<Explanation, ExplainError> {
        // Try to extract JSON from response (handle markdown code blocks)
        let json_str = extract_json(response);

        // Parse with defaults for missing fields
        match serde_json::from_str::<Explanation>(&json_str) {
            Ok(exp) => Ok(exp),
            Err(e) => {
                // Try to create a partial explanation
                if let Ok(partial) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    Ok(Explanation {
                        snippet_id: meta.id.clone(),
                        kind: meta.kind.clone(),
                        summary: partial["summary"]
                            .as_str()
                            .unwrap_or("Unable to generate summary")
                            .to_string(),
                        detailed_description: partial["detailed_description"]
                            .as_str()
                            .map(String::from),
                        parameters: Vec::new(),
                        return_value: None,
                        effects_summary: partial["effects_summary"].as_str().map(String::from),
                        effects: Vec::new(),
                        step_explanations: Vec::new(),
                        algorithm_summary: None,
                        data_flow_summary: None,
                        requirements_covered: Vec::new(),
                        tests_summary: None,
                        related_snippets: Vec::new(),
                        usage_example: None,
                        warnings: vec!["Explanation may be incomplete due to parsing errors"
                            .to_string()],
                        generated_at: Utc::now(),
                        generator_version: Some("0.1.0".to_string()),
                        snippet_hash: None,
                        confidence: Some(0.5),
                    })
                } else {
                    Err(ExplainError::Json(e))
                }
            }
        }
    }

    /// Compute content hash for caching
    fn hash_content(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Format explanation for output
pub fn format_explanation(explanation: &Explanation, format: ExplainFormat) -> String {
    match format {
        ExplainFormat::Json => serde_json::to_string_pretty(explanation).unwrap_or_default(),
        ExplainFormat::Text => format_as_text(explanation),
        ExplainFormat::Markdown => format_as_markdown(explanation),
        ExplainFormat::Compact => format_as_compact(explanation),
    }
}

fn format_as_text(exp: &Explanation) -> String {
    let mut out = String::new();

    out.push_str(&format!("{} ({})\n", exp.snippet_id, exp.kind));
    out.push_str(&format!("{}\n\n", "=".repeat(exp.snippet_id.len() + exp.kind.len() + 3)));

    out.push_str(&format!("{}\n\n", exp.summary));

    if let Some(ref desc) = exp.detailed_description {
        out.push_str(&format!("{}\n\n", desc));
    }

    if !exp.parameters.is_empty() {
        out.push_str("Parameters:\n");
        for p in &exp.parameters {
            out.push_str(&format!("  {} ({}): {}\n", p.name, p.ty, p.description));
        }
        out.push('\n');
    }

    if let Some(ref ret) = exp.return_value {
        out.push_str(&format!("Returns: {} - {}\n\n", ret.ty, ret.description));
    }

    if let Some(ref effects) = exp.effects_summary {
        out.push_str(&format!("Effects: {}\n\n", effects));
    }

    if !exp.step_explanations.is_empty() {
        out.push_str("Steps:\n");
        for s in &exp.step_explanations {
            out.push_str(&format!("  {}: {}\n", s.step_id, s.what));
        }
    }

    out
}

fn format_as_markdown(exp: &Explanation) -> String {
    let mut out = String::new();

    out.push_str(&format!("# `{}`\n\n", exp.snippet_id));
    out.push_str(&format!("**Kind:** {}\n\n", exp.kind));
    out.push_str(&format!("{}\n\n", exp.summary));

    if let Some(ref desc) = exp.detailed_description {
        out.push_str("## Description\n\n");
        out.push_str(&format!("{}\n\n", desc));
    }

    if !exp.parameters.is_empty() {
        out.push_str("## Parameters\n\n");
        out.push_str("| Name | Type | Description |\n");
        out.push_str("|------|------|-------------|\n");
        for p in &exp.parameters {
            out.push_str(&format!("| `{}` | `{}` | {} |\n", p.name, p.ty, p.description));
        }
        out.push('\n');
    }

    if let Some(ref ret) = exp.return_value {
        out.push_str("## Returns\n\n");
        out.push_str(&format!("**Type:** `{}`\n\n", ret.ty));
        out.push_str(&format!("{}\n\n", ret.description));
    }

    if let Some(ref effects) = exp.effects_summary {
        out.push_str("## Effects\n\n");
        out.push_str(&format!("{}\n\n", effects));
    }

    out
}

fn format_as_compact(exp: &Explanation) -> String {
    format!("{}: {}", exp.snippet_id, exp.summary)
}

// Helper functions

fn snippet_kind_str(kind: SnippetKind) -> String {
    match kind {
        SnippetKind::Function => "fn".to_string(),
        SnippetKind::Struct => "struct".to_string(),
        SnippetKind::Enum => "enum".to_string(),
        SnippetKind::Module => "module".to_string(),
        SnippetKind::Database => "database".to_string(),
        SnippetKind::Extern => "extern".to_string(),
        SnippetKind::ExternAbstract => "extern-abstract".to_string(),
        SnippetKind::ExternImpl => "extern-impl".to_string(),
        SnippetKind::Test => "test".to_string(),
        SnippetKind::Data => "data".to_string(),
    }
}

fn step_kind_str(kind: &StepKind) -> String {
    match kind {
        StepKind::Compute(_) => "compute".to_string(),
        StepKind::Call(_) => "call".to_string(),
        StepKind::Query(_) => "query".to_string(),
        StepKind::Bind(_) => "bind".to_string(),
        StepKind::Return(_) => "return".to_string(),
        StepKind::If(_) => "if".to_string(),
        StepKind::Match(_) => "match".to_string(),
        StepKind::For(_) => "for".to_string(),
        StepKind::Insert(_) => "insert".to_string(),
        StepKind::Update(_) => "update".to_string(),
        StepKind::Delete(_) => "delete".to_string(),
        StepKind::Transaction(_) => "transaction".to_string(),
        StepKind::Traverse(_) => "traverse".to_string(),
        StepKind::Construct(_) => "construct".to_string(),
        StepKind::Parallel(_) => "parallel".to_string(),
        StepKind::Race(_) => "race".to_string(),
    }
}

fn type_to_string(ty: &Type) -> String {
    match &ty.kind {
        TypeKind::Named(path) => {
            let name = path.segments.join("::");
            if path.generics.is_empty() {
                name
            } else {
                let params_str = path.generics.iter().map(type_to_string).collect::<Vec<_>>().join(", ");
                format!("{}[{}]", name, params_str)
            }
        }
        TypeKind::Optional(inner) => format!("{}?", type_to_string(inner)),
        TypeKind::List(inner) => format!("List[{}]", type_to_string(inner)),
        TypeKind::Union(types) => {
            types.iter().map(type_to_string).collect::<Vec<_>>().join(" | ")
        }
        TypeKind::Tuple(types) => {
            let types_str = types.iter().map(type_to_string).collect::<Vec<_>>().join(", ");
            format!("({})", types_str)
        }
        TypeKind::Function { params, ret } => {
            let params_str = params.iter().map(type_to_string).collect::<Vec<_>>().join(", ");
            let ret_str = type_to_string(ret);
            format!("({}) -> {}", params_str, ret_str)
        }
        TypeKind::Struct(fields) => {
            let fields_str = fields
                .iter()
                .map(|f| format!("{}: {}", f.name, type_to_string(&f.ty)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {} }}", fields_str)
        }
    }
}

fn return_type_to_string(ret: &ReturnType) -> String {
    match ret {
        ReturnType::Single { ty, optional } => {
            let base = type_to_string(ty);
            if *optional {
                format!("{}?", base)
            } else {
                base
            }
        }
        ReturnType::Collection { of } => format!("List[{}]", type_to_string(of)),
        ReturnType::Union { types } => {
            let types_str = types
                .iter()
                .map(|m| {
                    let base = type_to_string(&m.ty);
                    if m.optional {
                        format!("{}?", base)
                    } else {
                        base
                    }
                })
                .collect::<Vec<_>>()
                .join(" | ");
            types_str
        }
    }
}

/// Extract JSON from response (handles markdown code blocks)
fn extract_json(response: &str) -> String {
    // Try to find JSON code block
    if let Some(start) = response.find("```json") {
        let after_marker = &response[start + 7..];
        if let Some(end) = after_marker.find("```") {
            return after_marker[..end].trim().to_string();
        }
    }

    // Try generic code block
    if let Some(start) = response.find("```") {
        let after_backticks = &response[start + 3..];
        let code_start = after_backticks.find('\n').map(|i| i + 1).unwrap_or(0);
        let code = &after_backticks[code_start..];
        if let Some(end) = code.find("```") {
            return code[..end].trim().to_string();
        }
    }

    // Try to find raw JSON object
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            return response[start..=end].to_string();
        }
    }

    response.to_string()
}
