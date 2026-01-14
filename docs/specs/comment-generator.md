# Comment Generator Specification

Algorithm and implementation details for auto-generating comments.

---

## Overview

The Comment Generator produces human-readable inline comments for Covenant code. It uses a combination of template-based generation and LLM enhancement.

---

## Input

- Parsed AST of a Covenant snippet
- Verbosity level (minimal, standard, detailed)
- Optional: User-provided description hint

---

## Output

- Modified AST with comment nodes inserted
- Formatted `.cov` file with comments

---

## Generation Algorithm

### Phase 1: Structure Analysis

Analyze the snippet structure:

```
struct SnippetAnalysis {
    kind: SnippetKind,
    has_effects: bool,
    effect_count: int,
    has_requirements: bool,
    param_count: int,
    return_type: ReturnTypeKind,  // simple, union, collection
    step_count: int,
    has_branches: bool,
    has_loops: bool,
    complexity_score: int,
}
```

### Phase 2: Template Selection

Select comment templates based on analysis:

**Snippet Summary Templates:**

| Condition | Template |
|-----------|----------|
| Pure function | "{Name} computes {return_description}" |
| Query function | "{Name} retrieves {return_type} from {target}" |
| CRUD function | "{Name} {operation}s {entity}" |
| Effectful | "{Name} {action} using {effects}" |

**Section Templates:**

| Section | Template |
|---------|----------|
| effects | "EFFECTS: {effect_summary}" |
| requires | "REQUIREMENTS: Implements {req_ids}" |
| signature | "SIGNATURE: {param_summary} -> {return_summary}" |
| body | "IMPLEMENTATION: {algorithm_hint}" |
| tests | "TESTS: {coverage_summary}" |

**Step Templates:**

| Step Kind | Template |
|-----------|----------|
| compute | "{Operation} {operands} to produce {output}" |
| call (fn) | "Call {function} with {arg_summary}" |
| call (tool) | "Invoke external tool {tool}" |
| query | "Query {target} for {selection}" |
| bind | "Bind {source} to {name}" |
| return | "Return {value_description}" |
| if | "If {condition}, {then_summary}; else {else_summary}" |
| match | "Match {value} against {variant_count} cases" |
| for | "For each {var} in {collection}, {body_summary}" |
| insert | "Insert into {target}: {fields}" |
| update | "Update {target} where {condition}" |
| delete | "Delete from {target} where {condition}" |
| transaction | "Atomically: {step_summaries}" |
| traverse | "Traverse {relation} from {start}" |

### Phase 3: Content Generation

For each template, fill in placeholders:

**Snippet Level:**
1. Extract function name from signature
2. Summarize effects as comma-separated list
3. Describe return type (handle unions specially)
4. Generate one-sentence purpose

**Step Level:**
1. Map operation to verb phrase
2. Summarize inputs (first 2-3 args)
3. Note output binding and type

### Phase 4: LLM Enhancement (Optional)

For `detailed` verbosity, use LLM to enhance:

**Prompt:**
```
Given this Covenant step:

{step_ast}

Generate a comment that:
1. Explains what this step does in plain English
2. Notes any important implications
3. Describes the output type/value

Keep it under 100 characters per line.
```

### Phase 5: Formatting

Apply formatting rules:

1. Wrap at configured line length (default 80)
2. Align multi-line comments
3. Add blank lines before section comments
4. Indent step comments to match step indentation

---

## Complexity Scoring

Determine comment depth based on complexity:

```
score = (
    step_count * 1 +
    branch_count * 2 +
    loop_count * 3 +
    match_count * 2 +
    nested_depth * 2 +
    effect_count * 1
)

if score <= 5:
    complexity = "simple"
elif score <= 15:
    complexity = "moderate"
else:
    complexity = "complex"
```

Adjust verbosity:
- Simple: minimal comments unless requested otherwise
- Moderate: standard comments
- Complex: encourage detailed comments

---

## Verb Phrase Mappings

### Operations

| Op | Verb Phrase |
|----|-------------|
| add | "Add" |
| sub | "Subtract" |
| mul | "Multiply" |
| div | "Divide" |
| mod | "Get remainder of" |
| equals | "Check if equal:" |
| not_equals | "Check if not equal:" |
| less | "Check if less than:" |
| greater | "Check if greater than:" |
| and | "Logical AND:" |
| or | "Logical OR:" |
| not | "Negate:" |
| concat | "Concatenate:" |
| contains | "Check if contains:" |

### Step Actions

| Kind | Action Verb |
|------|-------------|
| query | "Query", "Retrieve", "Fetch", "Look up" |
| insert | "Insert", "Create", "Add" |
| update | "Update", "Modify", "Change" |
| delete | "Delete", "Remove" |
| call | "Call", "Invoke", "Execute" |
| return | "Return" |
| bind | "Bind", "Assign", "Set" |
| if | "If", "When", "Check" |
| match | "Match", "Handle", "Switch on" |
| for | "For each", "Iterate over" |

---

## Special Cases

### Union Return Types

```
// Returns: AuthToken on success, AuthError on failure
returns union
  type="AuthToken"
  type="AuthError"
end
```

Generate: "Returns AuthToken on success, AuthError on failure"

### Optional Types

```
returns type="User" optional
```

Generate: "Returns User if found, none otherwise"

### Collection Types

```
returns collection of="User"
```

Generate: "Returns a list of User records"

### Error Handlers

```
handle
  case type="ParseError"
    // ...
  end
end
```

Generate: "Handle ParseError: {case_summary}"

---

## Comment Quality Rules

### Do

- Use active voice ("Query the database" not "The database is queried")
- Start with a verb for action comments
- Include type information where helpful
- Explain WHY for non-obvious steps
- Use domain terminology

### Don't

- State the obvious (`// Increment x by 1` for `op=add ... lit=1`)
- Include implementation details in summary
- Use jargon without explanation
- Repeat the code in words

### Skip Comments When

- Step is trivially obvious from context
- Previous comment explains this step
- Step is a simple bind from parameter
- Step is final return of computed result

---

## Implementation Notes

### AST Modification

Insert comment nodes into AST:

```rust
struct CommentNode {
    kind: CommentKind,  // line, block
    content: String,
    position: Position,  // before, inline
    marker: Option<String>,  // "[AUTO]", "[MANUAL]"
}
```

### Rendering

When serializing AST to text:

1. Emit comment nodes before their target
2. Apply indentation matching target
3. Wrap long comments across lines
4. Preserve blank line spacing

### Idempotency

Running the generator twice should produce identical output:
- Same input → same comments
- Regenerating doesn't add duplicate comments
- Hash-based caching ensures stability
