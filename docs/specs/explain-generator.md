# Explain Generator Specification

Specification for the tool that generates natural language explanations of Covenant code.

---

## Overview

The Explain Generator takes a parsed Covenant snippet and produces a structured explanation (per [ai-explain-schema.json](ai-explain-schema.json)) that describes what the code does in human-readable terms.

---

## Input

**Required:**
- Parsed AST of a Covenant snippet
- Snippet ID

**Optional:**
- Symbol graph context (for understanding calls, references)
- Previous explanations (for incremental updates)
- User-specified verbosity level

---

## Output

JSON object conforming to the AI Explain Schema, containing:
- Natural language summary
- Parameter descriptions
- Return value explanation
- Step-by-step explanations
- Effects summary
- Data flow description

---

## Generation Strategy

### Phase 1: AST Analysis

Extract structural information from the AST:

1. **Signature Analysis**
   - Extract parameter names, types, and positions
   - Determine return type (simple, union, collection)
   - Identify generic type parameters

2. **Effects Analysis**
   - List declared effects
   - Note any effect parameters
   - Compute effect closure from called functions

3. **Body Analysis**
   - Count and categorize steps by kind
   - Identify control flow patterns (linear, branching, looping)
   - Map data flow between steps

### Phase 2: Pattern Recognition

Identify common patterns to inform explanation:

| Pattern | Indicators | Explanation Hint |
|---------|------------|------------------|
| Query-then-return | query step followed by return | "Retrieves X from Y" |
| Validate-and-transform | compute checks, if branching | "Validates X before processing" |
| Error propagation | match on union, return error case | "Handles errors by propagating" |
| CRUD operation | insert/update/delete step | "Creates/updates/deletes records" |
| Iteration | for loop | "Processes each item in collection" |

### Phase 3: LLM Generation

Use Claude API to generate natural language:

**Prompt Template:**
```
You are explaining Covenant code to a developer.

Snippet ID: {snippet_id}
Kind: {kind}
Effects: {effects_list}
Parameters: {params_with_types}
Returns: {return_type}
Step count: {step_count}
Patterns detected: {patterns}

Generate:
1. A one-sentence summary (max 100 chars)
2. A detailed description (2-3 paragraphs)
3. For each parameter, a description of what it represents
4. For each step, a brief explanation

Focus on WHAT the code does and WHY, not HOW (the syntax).
Use domain language appropriate for {detected_domain}.
```

### Phase 4: Validation

Verify generated explanation:

1. **Accuracy check** - Does summary match detected patterns?
2. **Completeness check** - Are all parameters explained?
3. **Type consistency** - Do type references match AST?
4. **Length limits** - Truncate if exceeding schema limits

---

## Caching Strategy

### Cache Key
```
key = hash(snippet_id + snippet_content_hash + generator_version)
```

### Cache Invalidation

Regenerate when:
- Snippet content changes (different hash)
- Generator version changes
- Explicit invalidation requested

### Cache Storage

Store in `.covenant/explanations/` directory:
```
.covenant/
  explanations/
    auth.login.json
    users.create.json
    ...
```

---

## Step Kind Mappings

Default verb phrases for each step kind:

| Step Kind | Default Phrase Template |
|-----------|------------------------|
| `compute` | "{operation} {inputs} to produce {output}" |
| `call` | "Call {function} with {args}" |
| `query` | "Query {target} for {selection}" |
| `bind` | "Bind {source} to {name}" |
| `return` | "Return {value}" |
| `if` | "If {condition}, then {then_summary}, else {else_summary}" |
| `match` | "Match {value} against {case_count} cases" |
| `for` | "For each {var} in {collection}, {body_summary}" |
| `insert` | "Insert into {target} with {fields}" |
| `update` | "Update {target} where {condition}" |
| `delete` | "Delete from {target} where {condition}" |
| `transaction` | "Execute {step_count} steps atomically" |
| `traverse` | "Traverse {relation} from {start}" |

---

## Configuration

### Verbosity Levels

| Level | Summary | Parameters | Steps | Data Flow |
|-------|---------|------------|-------|-----------|
| minimal | Yes | Names only | None | None |
| standard | Yes | With types | Key steps | Summary |
| detailed | Yes | Full descriptions | All steps | Complete |

### Domain Detection

Detect domain from:
- Module name patterns (`auth.*`, `users.*`, `payments.*`)
- Effect combinations
- Type names used

Adjust vocabulary accordingly:
- `auth` domain: "authenticates", "validates credentials", "issues token"
- `db` domain: "queries", "retrieves", "persists"
- `http` domain: "handles request", "returns response"

---

## Error Handling

If generation fails:

1. Return partial explanation with available data
2. Set `confidence` to 0.0
3. Add warning explaining what couldn't be generated
4. Log error for debugging

---

## Integration Points

### CLI Command
```
covenant explain <snippet_id>
covenant explain --file <path>
covenant explain --all
```

### Language Server
- Hover tooltips
- Documentation generation
- Code lens annotations

### Build Pipeline
- Pre-commit hook for documentation
- CI/CD documentation generation
- API documentation export

---

## Performance Targets

| Operation | Target |
|-----------|--------|
| Cache hit | < 10ms |
| Simple snippet (no LLM) | < 100ms |
| Complex snippet (with LLM) | < 2s |
| Batch generation (100 snippets) | < 60s |
