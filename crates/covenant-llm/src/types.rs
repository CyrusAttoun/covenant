//! Types for AI explanation generation
//!
//! These types match the schema defined in docs/specs/ai-explain-schema.json

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Complete explanation for a Covenant snippet
/// Matches the AI Explain Schema specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explanation {
    /// The full snippet ID (e.g., 'auth.login')
    pub snippet_id: String,

    /// The snippet kind (fn, struct, enum, etc.)
    pub kind: String,

    /// One-sentence natural language summary
    pub summary: String,

    /// Multi-paragraph explanation for documentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detailed_description: Option<String>,

    /// Explanations for each parameter (for fn kind)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<ParamExplanation>,

    /// Explanation of the return value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_value: Option<ReturnExplanation>,

    /// Human-readable summary of effects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects_summary: Option<String>,

    /// List of effects with explanations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<EffectExplanation>,

    /// Per-step explanations for body steps
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub step_explanations: Vec<StepExplanation>,

    /// High-level algorithm description for complex functions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm_summary: Option<String>,

    /// Summary of how data flows through the function
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_flow_summary: Option<String>,

    /// Requirements this snippet implements
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements_covered: Vec<RequirementRef>,

    /// Summary of test coverage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tests_summary: Option<String>,

    /// Related snippets for context
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_snippets: Vec<RelatedSnippet>,

    /// Example of how to call/use this snippet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_example: Option<String>,

    /// Important caveats or warnings
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,

    /// When this explanation was generated
    pub generated_at: DateTime<Utc>,

    /// Version of the explanation generator
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator_version: Option<String>,

    /// Hash of the snippet content for cache invalidation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet_hash: Option<String>,

    /// Confidence score for this explanation (0.0 to 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// Explanation for a function parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamExplanation {
    /// Parameter name
    pub name: String,

    /// Parameter type
    #[serde(rename = "type")]
    pub ty: String,

    /// What this parameter represents
    pub description: String,

    /// Any constraints or valid values
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<String>,
}

/// Explanation of a return value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnExplanation {
    /// Return type
    #[serde(rename = "type")]
    pub ty: String,

    /// What the return value represents
    pub description: String,

    /// When success values are returned
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub success_cases: Vec<String>,

    /// When error values are returned
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub error_cases: Vec<String>,
}

/// Explanation for an effect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectExplanation {
    /// Effect name
    pub effect: String,

    /// How this effect is used
    pub description: String,

    /// Effect parameters if any
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Explanation for a single step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExplanation {
    /// The step ID (e.g., 's1', 's2a')
    pub step_id: String,

    /// What this step does
    pub what: String,

    /// Why this step is needed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,

    /// What type/value this step produces
    #[serde(skip_serializing_if = "Option::is_none")]
    pub produces: Option<String>,

    /// Where inputs come from and outputs go
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_flow: Option<String>,
}

/// Reference to a requirement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementRef {
    pub req_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Reference to a related snippet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedSnippet {
    pub snippet_id: String,
    pub relationship: RelationshipKind,
}

/// Type of relationship between snippets
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKind {
    Calls,
    CalledBy,
    UsesType,
    SimilarTo,
    DocumentedBy,
}

/// Verbosity level for explanations
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Verbosity {
    /// Names only, minimal output
    Minimal,
    /// Default: summaries with types
    #[default]
    Standard,
    /// Full descriptions, all steps
    Detailed,
}

impl std::str::FromStr for Verbosity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "minimal" | "min" => Ok(Verbosity::Minimal),
            "standard" | "std" => Ok(Verbosity::Standard),
            "detailed" | "full" => Ok(Verbosity::Detailed),
            _ => Err(format!("Unknown verbosity level: {}", s)),
        }
    }
}

/// Detected code patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    /// Query followed by return
    QueryThenReturn,
    /// Validation before processing
    ValidateAndTransform,
    /// Error handling with match/propagation
    ErrorPropagation,
    /// Create/Read/Update/Delete operation
    Crud,
    /// Loop iteration over collection
    Iteration,
    /// Pure computation (no effects)
    PureComputation,
}

impl Pattern {
    pub fn description(&self) -> &'static str {
        match self {
            Pattern::QueryThenReturn => "Retrieves data and returns it",
            Pattern::ValidateAndTransform => "Validates input before processing",
            Pattern::ErrorPropagation => "Handles errors by propagating them",
            Pattern::Crud => "Creates, reads, updates, or deletes records",
            Pattern::Iteration => "Processes each item in a collection",
            Pattern::PureComputation => "Performs computation without side effects",
        }
    }
}

/// Output format for explanations
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExplainFormat {
    /// JSON output (default)
    #[default]
    Json,
    /// Human-readable text
    Text,
    /// Markdown documentation
    Markdown,
    /// Compact single-line
    Compact,
}

impl std::str::FromStr for ExplainFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(ExplainFormat::Json),
            "text" | "txt" => Ok(ExplainFormat::Text),
            "markdown" | "md" => Ok(ExplainFormat::Markdown),
            "compact" => Ok(ExplainFormat::Compact),
            _ => Err(format!("Unknown format: {}", s)),
        }
    }
}

/// Metadata extracted from AST for prompt building
#[derive(Debug, Clone)]
pub struct SnippetMetadata {
    pub id: String,
    pub kind: String,
    pub effects: Vec<String>,
    pub params: Vec<(String, String)>, // (name, type)
    pub return_type: Option<String>,
    pub step_count: usize,
    pub step_kinds: Vec<String>,
    pub requirements: Vec<(String, Option<String>)>, // (id, text)
    pub test_count: usize,
}
