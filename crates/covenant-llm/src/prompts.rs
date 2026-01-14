//! Prompt templates for LLM interactions

use crate::types::{Pattern, SnippetMetadata, Verbosity};

/// System prompt for code generation
pub const CODE_GENERATION_PROMPT: &str = r#"You are a code generator for the Covenant programming language. Covenant is a machine-first IR language designed for LLM generation.

## Critical Rules

1. **No operators** - Use keywords: `add`, `sub`, `mul`, `div`, `mod`, `equals`, `and`, `or`, `not`, `less`, `greater`, `less_eq`, `greater_eq`
2. **SSA form** - One operation per step, each step has a named output (`as="result"`)
3. **Canonical ordering** - Sections must be in order: effects, requires, signature, body, tests
4. **Every node has an ID** - All steps need `id="..."` attributes

## Snippet Structure

```
snippet id="module.function_name" kind="fn"

effects
  effect database
  effect network
end

signature
  fn name="function_name"
    param name="x" type="Int"
    returns type="Int"
  end
end

body
  step id="s1" kind="compute"
    op=add
    input var="x"
    input lit=1
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
```

## Step Types

- `compute`: Mathematical/logical operations (`op=add`, `op=equals`, etc.)
- `call`: Function calls (`fn="name"`, `arg name="x" from="var"`)
- `return`: Return a value (`from="var"` or `lit=value`)
- `if`: Conditional (`condition="var"`, `then ... end`, `else ... end`)
- `bind`: Bind a variable (`from var="x"` or `lit=value`)

## Types

- Primitives: `Int`, `Float`, `Bool`, `String`, `None`
- Collections: `List[T]`, `Map[K, V]`
- Optional: `type="User" optional`
- Union returns: `returns union type="Success" type="Error" end`

Output ONLY valid Covenant code. No explanations, no markdown formatting.
"#;

/// System prompt for explanation generation
pub const EXPLAIN_SYSTEM_PROMPT: &str = r#"You are an expert at explaining Covenant code to developers.

Covenant is a machine-first IR (intermediate representation) language with:
- Explicit effects (database, network, filesystem, etc.)
- SSA (single static assignment) form - one operation per step
- Keyword-based operations instead of operators (add, equals, etc.)
- Explicit requirements and tests as first-class constructs

Your job is to generate clear, accurate explanations of what Covenant code does.

## Output Format

You MUST output valid JSON matching this structure:
{
  "summary": "One sentence describing what this code does",
  "detailed_description": "2-3 paragraphs explaining the purpose and behavior",
  "parameters": [
    {"name": "param_name", "type": "Type", "description": "what it represents"}
  ],
  "return_value": {
    "type": "ReturnType",
    "description": "what is returned",
    "success_cases": ["when success is returned"],
    "error_cases": ["when error is returned"]
  },
  "effects_summary": "Human-readable description of side effects",
  "effects": [
    {"effect": "effect_name", "description": "how it's used"}
  ],
  "step_explanations": [
    {"step_id": "s1", "what": "what this step does", "why": "why it's needed"}
  ],
  "algorithm_summary": "High-level algorithm description (for complex functions)",
  "data_flow_summary": "How data flows through the function",
  "warnings": ["any important caveats"]
}

## Guidelines

1. Focus on WHAT the code does and WHY, not HOW (the syntax)
2. Use domain-appropriate language based on the snippet's module/effects
3. Be concise but complete
4. For parameters, explain what valid values are if relevant
5. For effects, explain what external resources are accessed
6. For steps, explain the business logic, not the syntax

Output ONLY valid JSON. No markdown, no explanations outside the JSON."#;

/// Build the user prompt for explanation generation
pub fn build_explain_prompt(
    meta: &SnippetMetadata,
    patterns: &[Pattern],
    code: &str,
    verbosity: Verbosity,
) -> String {
    let effects_str = if meta.effects.is_empty() {
        "None (pure function)".to_string()
    } else {
        meta.effects.join(", ")
    };

    let params_str = if meta.params.is_empty() {
        "None".to_string()
    } else {
        meta.params
            .iter()
            .map(|(name, ty)| format!("{}: {}", name, ty))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let return_str = meta.return_type.as_deref().unwrap_or("None");

    let patterns_str = if patterns.is_empty() {
        "None detected".to_string()
    } else {
        patterns
            .iter()
            .map(|p| p.description())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let verbosity_instruction = match verbosity {
        Verbosity::Minimal => {
            "Provide minimal output: summary only, parameter names without descriptions, no step explanations."
        }
        Verbosity::Standard => {
            "Provide standard output: summary, parameter descriptions, key step explanations."
        }
        Verbosity::Detailed => {
            "Provide detailed output: full descriptions, all step explanations, data flow analysis."
        }
    };

    format!(
        r#"Explain the following Covenant code.

## Snippet Info
- ID: {id}
- Kind: {kind}
- Effects: {effects}
- Parameters: {params}
- Returns: {returns}
- Step count: {step_count}
- Patterns detected: {patterns}

## Verbosity
{verbosity_instruction}

## Code
```covenant
{code}
```

Generate a JSON explanation following the schema in your instructions."#,
        id = meta.id,
        kind = meta.kind,
        effects = effects_str,
        params = params_str,
        returns = return_str,
        step_count = meta.step_count,
        patterns = patterns_str,
        code = code
    )
}

/// Step kind to default verb phrase mapping
#[allow(dead_code)]
pub fn step_kind_phrase(kind: &str) -> &'static str {
    match kind {
        "compute" => "Computes",
        "call" => "Calls",
        "query" => "Queries",
        "bind" => "Binds",
        "return" => "Returns",
        "if" => "Conditionally executes",
        "match" => "Matches against",
        "for" => "Iterates over",
        "insert" => "Inserts into",
        "update" => "Updates",
        "delete" => "Deletes from",
        "transaction" => "Executes atomically",
        "traverse" => "Traverses",
        _ => "Performs",
    }
}

/// Detect domain from snippet metadata
#[allow(dead_code)]
pub fn detect_domain(meta: &SnippetMetadata) -> &'static str {
    let id_lower = meta.id.to_lowercase();

    // Check module name patterns
    if id_lower.starts_with("auth.") || id_lower.contains("login") || id_lower.contains("password")
    {
        return "authentication";
    }
    if id_lower.starts_with("user.") || id_lower.starts_with("users.") {
        return "user management";
    }
    if id_lower.starts_with("payment.") || id_lower.contains("checkout") {
        return "payments";
    }
    if id_lower.starts_with("http.") || id_lower.starts_with("api.") {
        return "HTTP/API";
    }

    // Check effects
    for effect in &meta.effects {
        match effect.as_str() {
            "database" => return "database operations",
            "network" => return "network operations",
            "filesystem" => return "file operations",
            "console" => return "console I/O",
            _ => {}
        }
    }

    // Default
    "general"
}
