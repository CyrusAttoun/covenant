//! Diagnostic context and fix suggestions
//!
//! This module provides rich error context with fix suggestions
//! and effect violation explanations for the Covenant compiler.

use covenant_ast::Span;
use crate::CheckError;

/// A diagnostic with context, suggestions, and explanations
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// The underlying error
    pub error: DiagnosticKind,
    /// Suggested fixes
    pub suggestions: Vec<FixSuggestion>,
    /// Related source locations
    pub related: Vec<RelatedLocation>,
    /// Human-readable explanation of why this error occurred
    pub explanation: String,
    /// Primary source span
    pub span: Span,
    /// Severity level
    pub severity: Severity,
    /// Error code (e.g., "E-EFFECT-001")
    pub code: String,
}

/// Kind of diagnostic
#[derive(Debug, Clone)]
pub enum DiagnosticKind {
    /// Effect-related error
    Effect(EffectDiagnostic),
    /// Type-related error
    Type(TypeDiagnostic),
    /// Symbol-related error
    Symbol(SymbolDiagnostic),
}

/// Effect-specific diagnostic information
#[derive(Debug, Clone)]
pub struct EffectDiagnostic {
    /// The function with the effect violation
    pub function: String,
    /// For pure-calls-effectful: the effectful callee
    pub callee: Option<String>,
    /// Effects that are missing or violating
    pub effects: Vec<String>,
    /// The call chain that introduces the effects (for explanation)
    pub call_chain: Vec<CallChainEntry>,
}

/// An entry in the call chain for effect explanations
#[derive(Debug, Clone)]
pub struct CallChainEntry {
    /// Name of the function/snippet
    pub name: String,
    /// Effects introduced at this point
    pub effects: Vec<String>,
    /// Source span of the call
    pub span: Span,
}

/// Type-specific diagnostic information
#[derive(Debug, Clone)]
pub struct TypeDiagnostic {
    pub expected: String,
    pub found: String,
}

/// Symbol-specific diagnostic information
#[derive(Debug, Clone)]
pub struct SymbolDiagnostic {
    pub name: String,
    pub context: String,
}

/// A suggested fix for an error
#[derive(Debug, Clone)]
pub enum FixSuggestion {
    /// Add an effect declaration to a snippet
    AddEffect {
        /// The effect to add
        effect: String,
        /// The snippet ID to add it to
        snippet_id: String,
        /// Where to insert the effect (approximate)
        location: Span,
        /// The full code to insert
        code_snippet: String,
    },
    /// Remove a call to an effectful function
    RemoveCall {
        /// The callee to remove
        callee: String,
        /// Location of the call
        location: Span,
    },
    /// Wrap code in an effectful function
    WrapInEffectfulFunction {
        /// Effects that need to be declared
        effects: Vec<String>,
        /// Suggested function name
        suggested_name: Option<String>,
    },
    /// Declare effects section if missing
    DeclareEffectsSection {
        /// Effects to declare
        effects: Vec<String>,
        /// Snippet ID
        snippet_id: String,
    },
}

/// A related source location with context
#[derive(Debug, Clone)]
pub struct RelatedLocation {
    /// Message explaining the relation
    pub message: String,
    /// Source span
    pub span: Span,
    /// File path (if different from main error)
    pub file: Option<String>,
    /// Label for the location (e.g., "defined here", "called from here")
    pub label: String,
}

/// Severity level of a diagnostic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl Diagnostic {
    /// Create a new diagnostic
    pub fn new(
        error: DiagnosticKind,
        span: Span,
        code: impl Into<String>,
        explanation: impl Into<String>,
    ) -> Self {
        Self {
            error,
            suggestions: Vec::new(),
            related: Vec::new(),
            explanation: explanation.into(),
            span,
            severity: Severity::Error,
            code: code.into(),
        }
    }

    /// Add a fix suggestion
    pub fn with_suggestion(mut self, suggestion: FixSuggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Add suggestions from an iterator
    pub fn with_suggestions(mut self, suggestions: impl IntoIterator<Item = FixSuggestion>) -> Self {
        self.suggestions.extend(suggestions);
        self
    }

    /// Add a related location
    pub fn with_related(mut self, related: RelatedLocation) -> Self {
        self.related.push(related);
        self
    }

    /// Set severity
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Get the primary message for this diagnostic
    pub fn message(&self) -> String {
        match &self.error {
            DiagnosticKind::Effect(e) => {
                if let Some(ref callee) = e.callee {
                    format!(
                        "pure function `{}` calls effectful function `{}` (effects: {})",
                        e.function,
                        callee,
                        e.effects.join(", ")
                    )
                } else {
                    format!(
                        "function `{}` is missing effect declarations: {}",
                        e.function,
                        e.effects.join(", ")
                    )
                }
            }
            DiagnosticKind::Type(t) => {
                format!("type mismatch: expected `{}`, found `{}`", t.expected, t.found)
            }
            DiagnosticKind::Symbol(s) => {
                format!("undefined symbol: `{}` ({})", s.name, s.context)
            }
        }
    }

    /// Format the diagnostic as a simple string
    pub fn format_simple(&self) -> String {
        let mut output = format!("{}: {}\n", self.code, self.message());

        if !self.explanation.is_empty() {
            output.push_str(&format!("\nExplanation:\n  {}\n", self.explanation));
        }

        if !self.suggestions.is_empty() {
            output.push_str("\nSuggested fixes:\n");
            for (i, suggestion) in self.suggestions.iter().enumerate() {
                output.push_str(&format!("  {}. {}\n", i + 1, suggestion.description()));
            }
        }

        if !self.related.is_empty() {
            output.push_str("\nRelated locations:\n");
            for related in &self.related {
                output.push_str(&format!(
                    "  - {} (at {}..{})\n",
                    related.message, related.span.start, related.span.end
                ));
            }
        }

        output
    }
}

impl FixSuggestion {
    /// Get a human-readable description of this suggestion
    pub fn description(&self) -> String {
        match self {
            FixSuggestion::AddEffect { effect, snippet_id, .. } => {
                format!("Add `effect {}` to snippet `{}`", effect, snippet_id)
            }
            FixSuggestion::RemoveCall { callee, .. } => {
                format!("Remove call to `{}`", callee)
            }
            FixSuggestion::WrapInEffectfulFunction { effects, suggested_name } => {
                let name = suggested_name.as_deref().unwrap_or("new_function");
                format!(
                    "Extract code into a new function `{}` with effects: {}",
                    name,
                    effects.join(", ")
                )
            }
            FixSuggestion::DeclareEffectsSection { effects, snippet_id } => {
                format!(
                    "Add effects section to `{}` declaring: {}",
                    snippet_id,
                    effects.join(", ")
                )
            }
        }
    }

    /// Get the code snippet for this fix (if applicable)
    pub fn code_snippet(&self) -> Option<String> {
        match self {
            FixSuggestion::AddEffect { effect, .. } => {
                Some(format!("effect {}", effect))
            }
            FixSuggestion::DeclareEffectsSection { effects, .. } => {
                let mut code = String::from("effects\n");
                for effect in effects {
                    code.push_str(&format!("  effect {}\n", effect));
                }
                code.push_str("end");
                Some(code)
            }
            _ => None,
        }
    }
}

impl From<CheckError> for Diagnostic {
    fn from(error: CheckError) -> Self {
        match error {
            CheckError::UndefinedSymbol { name } => {
                Diagnostic::new(
                    DiagnosticKind::Symbol(SymbolDiagnostic {
                        name: name.clone(),
                        context: "not found in scope".to_string(),
                    }),
                    Span::dummy(),
                    "E-SYMBOL-001",
                    format!("The symbol `{}` was used but not defined. Check for typos or missing imports.", name),
                )
            }
            CheckError::TypeMismatch { expected, found } => {
                Diagnostic::new(
                    DiagnosticKind::Type(TypeDiagnostic {
                        expected: expected.clone(),
                        found: found.clone(),
                    }),
                    Span::dummy(),
                    "E-TYPE-001",
                    format!(
                        "Expected type `{}` but found `{}`. Ensure the value matches the expected type.",
                        expected, found
                    ),
                )
            }
            CheckError::EffectNotAllowed { effect } => {
                Diagnostic::new(
                    DiagnosticKind::Effect(EffectDiagnostic {
                        function: "unknown".to_string(),
                        callee: None,
                        effects: vec![effect.clone()],
                        call_chain: Vec::new(),
                    }),
                    Span::dummy(),
                    "E-EFFECT-001",
                    format!(
                        "The effect `{}` is not allowed in this context. Either declare it in the effects section or extract effectful code.",
                        effect
                    ),
                )
            }
            CheckError::DuplicateDefinition { name } => {
                Diagnostic::new(
                    DiagnosticKind::Symbol(SymbolDiagnostic {
                        name: name.clone(),
                        context: "already defined".to_string(),
                    }),
                    Span::dummy(),
                    "E-SYMBOL-002",
                    format!("The name `{}` is defined multiple times. Each name must be unique in its scope.", name),
                )
            }
            CheckError::IncompatibleUnion { value_type, union_type } => {
                Diagnostic::new(
                    DiagnosticKind::Type(TypeDiagnostic {
                        expected: union_type.clone(),
                        found: value_type.clone(),
                    }),
                    Span::dummy(),
                    "E-TYPE-002",
                    format!(
                        "Type `{}` is not a member of union `{}`. Check that the value matches one of the union variants.",
                        value_type, union_type
                    ),
                )
            }
            CheckError::NonExhaustiveMatch { missing, matched_type } => {
                Diagnostic::new(
                    DiagnosticKind::Type(TypeDiagnostic {
                        expected: format!("all variants of {}", matched_type),
                        found: format!("missing: {}", missing.join(", ")),
                    }),
                    Span::dummy(),
                    "E-TYPE-003",
                    format!(
                        "Match on `{}` is not exhaustive. Missing variants: {}. Add arms for all cases or use a wildcard pattern.",
                        matched_type,
                        missing.join(", ")
                    ),
                )
            }
            CheckError::UnknownQueryTarget { target } => {
                Diagnostic::new(
                    DiagnosticKind::Symbol(SymbolDiagnostic {
                        name: target.clone(),
                        context: "unknown query target".to_string(),
                    }),
                    Span::dummy(),
                    "E-QUERY-001",
                    format!(
                        "Query target `{}` is not recognized. Valid targets include: project, collections, or database bindings.",
                        target
                    ),
                )
            }
            CheckError::UnknownField { field, type_name } => {
                Diagnostic::new(
                    DiagnosticKind::Type(TypeDiagnostic {
                        expected: format!("field of {}", type_name),
                        found: field.clone(),
                    }),
                    Span::dummy(),
                    "E-TYPE-004",
                    format!(
                        "Field `{}` does not exist on type `{}`. Check the field name for typos.",
                        field, type_name
                    ),
                )
            }
            CheckError::UnknownExternAbstract { impl_id, abstract_id } => {
                Diagnostic::new(
                    DiagnosticKind::Symbol(SymbolDiagnostic {
                        name: abstract_id.clone(),
                        context: format!("referenced by extern-impl `{}`", impl_id),
                    }),
                    Span::dummy(),
                    "E-EXTERN-001",
                    format!(
                        "Extern implementation `{}` references unknown abstract `{}`. Define the extern-abstract first.",
                        impl_id, abstract_id
                    ),
                )
            }
            CheckError::NoBindingForTarget { extern_id, target } => {
                Diagnostic::new(
                    DiagnosticKind::Symbol(SymbolDiagnostic {
                        name: extern_id.clone(),
                        context: format!("no binding for target `{}`", target),
                    }),
                    Span::dummy(),
                    "E-EXTERN-002",
                    format!(
                        "No extern-impl binding for `{}` targeting platform `{}`. Create an extern-impl snippet for this target.",
                        extern_id, target
                    ),
                )
            }
            CheckError::InvalidExternId { id } => {
                Diagnostic::new(
                    DiagnosticKind::Symbol(SymbolDiagnostic {
                        name: id.clone(),
                        context: "invalid extern ID format".to_string(),
                    }),
                    Span::dummy(),
                    "E-EXTERN-003",
                    format!(
                        "Extern snippet ID `{}` must be namespaced (e.g., 'module.function'). Use a dot to separate namespace from name.",
                        id
                    ),
                )
            }
        }
    }
}

/// Builder for creating diagnostics from effect errors
pub struct EffectDiagnosticBuilder {
    diagnostic: Diagnostic,
}

impl EffectDiagnosticBuilder {
    /// Create a builder for a pure-calls-effectful error
    pub fn pure_calls_effectful(
        function: String,
        callee: String,
        effects: Vec<String>,
        span: Span,
    ) -> Self {
        let explanation = format!(
            "Function `{}` is pure (declares no effects) but calls `{}` which requires effects: {}. \
             Pure functions can only call other pure functions.",
            function, callee, effects.join(", ")
        );

        let diagnostic = Diagnostic::new(
            DiagnosticKind::Effect(EffectDiagnostic {
                function: function.clone(),
                callee: Some(callee.clone()),
                effects: effects.clone(),
                call_chain: Vec::new(),
            }),
            span,
            "E-EFFECT-001",
            explanation,
        );

        Self { diagnostic }
    }

    /// Create a builder for a missing-effect error
    pub fn missing_effect(
        function: String,
        missing: Vec<String>,
        source_callee: String,
        span: Span,
    ) -> Self {
        let explanation = format!(
            "Function `{}` calls `{}` which requires effects that are not declared: {}. \
             Add these effects to the function's effects section.",
            function, source_callee, missing.join(", ")
        );

        let diagnostic = Diagnostic::new(
            DiagnosticKind::Effect(EffectDiagnostic {
                function: function.clone(),
                callee: Some(source_callee.clone()),
                effects: missing.clone(),
                call_chain: Vec::new(),
            }),
            span,
            "E-EFFECT-002",
            explanation,
        );

        Self { diagnostic }
    }

    /// Add the call chain for detailed explanation
    pub fn with_call_chain(mut self, chain: Vec<CallChainEntry>) -> Self {
        if let DiagnosticKind::Effect(ref mut effect_diag) = self.diagnostic.error {
            effect_diag.call_chain = chain;
        }
        self
    }

    /// Add fix suggestions
    pub fn with_suggestions(mut self, suggestions: Vec<FixSuggestion>) -> Self {
        self.diagnostic.suggestions = suggestions;
        self
    }

    /// Add related locations
    pub fn with_related(mut self, related: Vec<RelatedLocation>) -> Self {
        self.diagnostic.related = related;
        self
    }

    /// Build the final diagnostic
    pub fn build(self) -> Diagnostic {
        self.diagnostic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_creation() {
        let diag = Diagnostic::new(
            DiagnosticKind::Effect(EffectDiagnostic {
                function: "my_func".to_string(),
                callee: Some("db_query".to_string()),
                effects: vec!["database".to_string()],
                call_chain: Vec::new(),
            }),
            Span::new(10, 20),
            "E-EFFECT-001",
            "Test explanation",
        );

        assert_eq!(diag.code, "E-EFFECT-001");
        assert_eq!(diag.span.start, 10);
        assert_eq!(diag.span.end, 20);
    }

    #[test]
    fn test_fix_suggestion_description() {
        let suggestion = FixSuggestion::AddEffect {
            effect: "database".to_string(),
            snippet_id: "my.function".to_string(),
            location: Span::dummy(),
            code_snippet: "effect database".to_string(),
        };

        assert!(suggestion.description().contains("database"));
        assert!(suggestion.description().contains("my.function"));
    }

    #[test]
    fn test_diagnostic_builder() {
        let diag = EffectDiagnosticBuilder::pure_calls_effectful(
            "pure_fn".to_string(),
            "effectful_fn".to_string(),
            vec!["network".to_string()],
            Span::new(0, 100),
        )
        .with_suggestions(vec![
            FixSuggestion::AddEffect {
                effect: "network".to_string(),
                snippet_id: "pure_fn".to_string(),
                location: Span::dummy(),
                code_snippet: "effect network".to_string(),
            },
        ])
        .build();

        assert_eq!(diag.code, "E-EFFECT-001");
        assert_eq!(diag.suggestions.len(), 1);
    }

    #[test]
    fn test_from_check_error() {
        let error = CheckError::UndefinedSymbol { name: "foo".to_string() };
        let diag: Diagnostic = error.into();

        assert_eq!(diag.code, "E-SYMBOL-001");
        assert!(diag.message().contains("foo"));
    }
}
