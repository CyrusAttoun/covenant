# Covenant: Design Philosophy

> **Natural language is for requesting.**
> **IR is for meaning.** *(machine-readable, canonical)*
> **Symbol graph is for querying.** *(derived, bidirectional)*
> **Bytecode is for execution.** *(WASM, sandboxed)*

---

## 1. Why Machine-First

AI coding agents struggle with traditional codebases:

- They parse **text**, not meaning
- They **search and guess** instead of query
- Generation is **probabilistic** — syntax variations increase entropy
- Each task starts from scratch — no compounding benefit

The root cause: **existing languages optimize for human authorship, not machine comprehension or generation.**

Covenant is a machine-first intermediate representation. The value is not in the syntax. The value is in:

1. **Deterministic generation** — one canonical way to write everything
2. **Queryable structure** — every symbol has an ID, relationships are explicit
3. **Explicit effects** — capabilities and side effects declared per-snippet
4. **Requirements linkage** — specs and tests are first-class nodes
5. **Token stability** — small grammar, predictable sequences

Human-readable views can be derived. The IR is the source of truth.

---

## 2. The Four-Layer Model

### 2.1 Natural Language — *Requesting*
Ephemeral. Ambiguous by nature. Human-to-AI communication only. Discarded after translation.

### 2.2 IR (Intermediate Representation) — *The Artifact*
Machine-readable source of truth. Deterministic, tree-shaped, keyword-heavy. Every construct has an ID. Canonical formatting — one way to write everything.

### 2.3 Symbol Graph — *Queryable Structure*
Derived from IR by the compiler. Bidirectional references (`called_by`, `calls`, `references`, `referenced_by`). Effect closure computed transitively. Requirements and tests linked to implementations.

### 2.4 Bytecode — *Execution*
WASM target. Deterministic. Sandboxed and capability-constrained. Metered execution.

---

## 3. Core Design Principles

| Principle | Implementation |
|-----------|----------------|
| **Deterministic structure** | Everything is a block with `snippet`, `id`, and `end` |
| **No operators** | Keywords only: `add`, `equals`, `and`, `or`, `not` |
| **No expression nesting** | One operation per step, named outputs (SSA form) |
| **Canonical ordering** | Fields appear in fixed order within each block type |
| **Small grammar** | ~50 keywords, no synonyms, no optional punctuation |
| **Every node has an ID** | Enables precise queries and references |
| **Effects explicit** | Declared in `effects` section, propagated transitively |
| **Requirements first-class** | `requires` section links specs to code |
| **Tests linked** | `tests` section declares coverage |

---

## 4. Syntax Overview

### 4.1 Snippet Structure

Every code unit is a `snippet` with explicit sections:

```
snippet id="module.function_name" kind="fn"

effects
  effect database
  effect network
end

requires
  req id="R-001"
    text "Users must be retrievable by ID"
    priority high
  end
end

signature
  fn name="get_user"
    param name="id" type="Int"
    returns union
      type="User" optional
      type="DbError"
    end
  end
end

body
  // implementation steps
end

tests
  test id="T-001" kind="unit" covers="R-001"
    // test steps
  end
end

end
```

### 4.2 Operations (No Operators)

All operations use keywords, not symbols:

| Instead of | Use |
|------------|-----|
| `x + y` | `op=add input var="x" input var="y"` |
| `x == y` | `op=equals input var="x" input var="y"` |
| `x && y` | `op=and input var="x" input var="y"` |
| `!x` | `op=not input var="x"` |
| `-x` | `op=neg input var="x"` |

### 4.3 SSA Form (Named Outputs)

Every step produces a named output. No expression nesting:

```
body
  step id="s1" kind="compute"
    op=mul
    input var="x"
    input lit=2
    as="doubled"
  end

  step id="s2" kind="compute"
    op=add
    input var="doubled"
    input lit=1
    as="result"
  end

  step id="s3" kind="return"
    from="result"
    as="_"
  end
end
```

### 4.4 Function Calls

```
step id="s1" kind="call"
  fn="validate_email"
  arg name="email" from="user.email"
  as="is_valid"
end
```

### 4.5 Tool Calls (External Effects)

```
tools
  tool id="t1" contract="payments.charge@1"
    idempotent=key
    timeout=30s
    retry=(max=3 backoff=exponential)
  end
end

body
  step id="s1" kind="call"
    tool="t1"
    arg name="amount" from="total"
    arg name="key" from="idempotency_key"
    as="charge_result"
  end
end
```

---

## 5. Query System

Covenant uses a **dialect-based query system** with two paths:

1. **Covenant Dialect** (default) — Strongly-typed, minimal syntax for querying Covenant types
2. **SQL Dialects** — Opaque body blocks for full SQL power on external databases

### 5.1 Covenant Queries

When `dialect` is omitted, queries use Covenant's simple, typed syntax. This is optimized for LLM generation with minimal keywords:

```
// Query Covenant types (project AST, structs, collections)
step id="s1" kind="query"
  target="project"
  select all
  from="functions"
  where
    contains field="effects" lit="database"
  end
  order by="name" dir="asc"
  limit=10
  as="db_functions"
end

// Query with field selection
step id="s2" kind="query"
  target="project"
  select field="id" field="name" field="effects"
  from="functions"
  where
    equals field="kind" lit="fn"
  end
  as="function_list"
end
```

**Supported operations:**
- `select all` or `select field="f1" field="f2" ...`
- `from="type_name"`
- `where` with: `equals`, `not_equals`, `less`, `greater`, `less_eq`, `greater_eq`, `contains`
- `and`, `or`, `not` for compound conditions
- `join to="type" on ... end` for explicit field joins
- `follow rel="relation_name"` for relation traversal
- `order by="field" dir="asc|desc"`
- `limit=N offset=M`

### 5.2 SQL Dialect Queries

For external databases, use dialect blocks with full SQL power:

```
step id="s1" kind="query"
  dialect="postgres"
  target="app_db"
  body
    SELECT u.id, u.name, COUNT(o.id) as order_count
    FROM users u
    LEFT JOIN orders o ON o.user_id = u.id
    WHERE u.created_at > :cutoff
    GROUP BY u.id, u.name
    HAVING COUNT(o.id) > 5
    ORDER BY order_count DESC
  end
  params
    param name="cutoff" from="cutoff_date"
  end
  returns collection of="UserOrderStats"
  as="high_volume_users"
end
```

**Key points:**
- `dialect` determines the SQL syntax (postgres, sqlserver, mysql, sqlite, etc.)
- `body ... end` contains raw SQL — not parsed by Covenant compiler
- `params` section declares parameter bindings
- `returns` type annotation is required
- Compiler validates parameter placeholders match `params` declarations

**Placeholder syntax by dialect:**
| Dialect | Placeholder | Example |
|---------|-------------|---------|
| postgres | `:name` | `:user_id` |
| sqlserver | `@name` | `@user_id` |
| mysql | `?` | Positional |
| sqlite | `:name` or `?` | Either form |

### 5.3 CRUD Operations (Covenant Types)

For Covenant-managed types, use structured CRUD syntax:

```
// Insert
step id="s1" kind="insert"
  into="project.data_nodes"
  set field="name" from="name"
  set field="content" from="content"
  as="new_node"
end

// Update
step id="s2" kind="update"
  target="project.data_nodes"
  set field="content" from="updated_content"
  where
    equals field="id" var="node_id"
  end
  as="updated"
end

// Delete
step id="s3" kind="delete"
  from="project.data_nodes"
  where
    equals field="id" var="node_id"
  end
  as="deleted"
end
```

For external databases, use SQL dialect blocks for CRUD:

```
step id="s4" kind="query"
  dialect="postgres"
  target="app_db"
  body
    INSERT INTO users (name, email)
    VALUES (:name, :email)
    RETURNING id, name, email
  end
  params
    param name="name" from="user_name"
    param name="email" from="user_email"
  end
  returns type="User"
  as="new_user"
end
```

### 5.4 Joins and Relations

**Explicit field join:**
```
step id="s1" kind="query"
  target="project"
  select all
  from="functions"
  join to="tests" on
    equals field="functions.id" field="tests.covers"
  end
  as="functions_with_tests"
end
```

**Relation traversal** (when relations are pre-declared):
```
step id="s2" kind="query"
  target="project"
  select all
  from="functions"
  follow rel="covered_by"
  as="functions_with_coverage"
end
```

### 5.5 Null Handling

`none` represents absence:

```
where
  equals field="deleted_at" lit=none    // Check for null
end

where
  not_equals field="deleted_at" lit=none  // Check for non-null
end
```

### 5.6 Database Bindings

External databases are declared as `kind="database"` snippets:

```
snippet id="db.app_db" kind="database"

metadata
  type="database"
  dialect="postgres"
  connection="env:APP_DB_URL"
  version="14.0"
end

schema
  table name="users"
    field name="id" type="Int" primary_key=true
    field name="email" type="String"
    field name="created_at" type="DateTime"
  end

  table name="orders"
    field name="id" type="Int" primary_key=true
    field name="user_id" type="Int"
    field name="total" type="Decimal"
  end
end

end
```

See [QUERY_SEMANTICS.md](QUERY_SEMANTICS.md) for formal operational semantics.

---

## 6. Effects System

### 6.1 Declaration

Effects are declared per-snippet:

```
effects
  effect database
  effect network
  effect filesystem(path="/data")
end
```

### 6.2 Propagation

Effects propagate transitively. If snippet A calls snippet B, A inherits B's effects. The compiler computes the full effect closure.

### 6.3 Pure Functions

A snippet with no `effects` section (or empty effects) is pure:

```
snippet id="math.double" kind="fn"

signature
  fn name="double"
    param name="x" type="Int"
    returns type="Int"
  end
end

body
  step id="s1" kind="compute"
    op=mul
    input var="x"
    input lit=2
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
```

The compiler verifies that pure snippets call nothing with effects.

---

## 7. Requirements and Tests

### 7.1 Requirements as Nodes

Requirements are first-class, queryable nodes:

```
requires
  req id="R-AUTH-001"
    text "Users must authenticate before accessing protected resources"
    priority critical
    status approved
  end

  req id="R-AUTH-002"
    text "Failed login attempts must be rate-limited"
    priority high
    status implemented
  end
end
```

### 7.2 Test Coverage Linkage

Tests declare which requirements they cover:

```
tests
  test id="T-AUTH-001" kind="integration" covers="R-AUTH-001"
    // test implementation
  end

  test id="T-AUTH-002" kind="property" covers="R-AUTH-002"
    property="rate limit enforced after 5 failures"
  end
end
```

### 7.3 Queryable

The symbol graph enables queries like:

```
// Find requirements without tests
query target="project"
  select all
  from="requirements"
  where
    equals field="covered_by" lit=[]
  end
end

// Find tests affected by changing a symbol
query target="project"
  select all
  from="tests"
  where
    contains field="depends_on" var="symbol_id"
  end
end
```

---

## 8. Bidirectional References

The compiler computes metadata on every symbol:

```
symbol_metadata = {
    id: symbol_id,
    called_by: [symbol_id],      // functions that call this
    calls: [symbol_id],          // functions this calls
    references: [symbol_id],     // types/symbols this references
    referenced_by: [symbol_id],  // what references this
    effects: [effect_id],        // computed effect closure
    tests: [test_id],            // tests that cover this
    requirements: [req_id],      // requirements this implements
}
```

Query it like any other data:

```
// Find all callers of authenticate
query target="project"
  select field="called_by"
  from="functions"
  where
    equals field="name" lit="authenticate"
  end
end

// Find unused code
query target="project"
  select all
  from="functions"
  where
    and
      equals field="called_by" lit=[]
      equals field="is_exported" lit=false
      equals field="is_entry_point" lit=false
    end
  end
end
```

---

## 9. Relations and Graph Structure

### 9.1 Data/Code Unification

Covenant unifies data and code through a common relation system. Documentation can link to functions, functions can reference their specifications, and knowledge bases can cross-reference arbitrarily.

**Key insight:** If documentation and code use the same relation primitives, an LLM can navigate from "what does this do?" directly to the implementation, and from code back to its explanation.

### 9.2 Data Nodes

The `kind="data"` snippet stores structured or unstructured content:

```
snippet id="docs.auth_flow" kind="data"

  note "Explains the authentication flow in the system"

  content
    """
    The authentication flow validates user credentials against the
    database and issues JWT tokens on success.

    Key steps:
    1. Validate email/password against bcrypt hash
    2. Generate JWT with user claims
    3. Return token or AuthError
    """
  end

  relations
    rel to="auth.login" type=describes
    rel to="auth.validate_token" type=describes
    rel to="docs.security_model" type=elaborates_on
  end

  metadata
    author="system"
    created="2024-01-15"
    tags=["auth", "security"]
  end

end
```

**With optional schema validation:**

```
snippet id="kb.user_profile" kind="data"

  schema
    field name="display_name" type="String"
    field name="email" type="String"
    field name="preferences" type="Map<String, String>" optional
  end

  content
    display_name "Alice Smith"
    email "alice@example.com"
    preferences {
      "theme": "dark",
      "language": "en"
    }
  end

end
```

### 9.3 Relation Types

Relations use a fixed vocabulary with automatic inverse maintenance:

| Category | Relations |
|----------|-----------|
| **Structural** | `contains` ↔ `contained_by`, `next` ↔ `previous` |
| **Semantic** | `describes` ↔ `described_by`, `elaborates_on`, `contrasts_with`, `example_of` |
| **Temporal** | `supersedes` ↔ `precedes`, `version_of` |
| **Causal** | `causes` ↔ `caused_by`, `motivates` ↔ `enables` |
| **Reference** | `related_to`, `depends_on`, `implements` ↔ `implemented_by` |

**Inverse maintenance (Invariant I5):** If A declares `rel to="B" type=describes`, the compiler automatically maintains `B.relations` to include `rel from="A" type=described_by`.

### 9.4 Cross-Domain Relations

Code can link to documentation, and vice versa:

```
// Function linking to its documentation
snippet id="auth.login" kind="fn"

  relations
    rel from="docs.auth_flow" type=described_by
    rel to="reqs.R-AUTH-001" type=implements
  end

  // ... signature, body, etc.
end
```

### 9.5 Graph Traversal

The `traverse` step follows relations transitively:

```
// Find all ancestors (transitive contained_by)
step id="s1" kind="traverse"
  target="project"
  from="docs.section_3_2"
  follow type=contained_by
  depth=unbounded
  as="all_ancestors"
end

// Find everything described by docs about auth
step id="s2" kind="traverse"
  target="project"
  from="docs.auth_overview"
  follow type=contains
  depth=3
  direction=outgoing
  as="auth_docs_tree"
end
```

**Traversal semantics:**
- `depth=unbounded` — follow edges until no more matches
- `depth=N` — follow at most N hops
- `direction=outgoing` — follow `rel to=...` edges (default)
- `direction=incoming` — follow `rel from=...` edges
- `direction=both` — follow edges in both directions

**Note:** Transitivity is semantic, not materialized. The compiler does NOT pre-compute transitive closures (expensive). Traversal discovers relationships at query time.

### 9.6 Querying by Relations

Where clauses can filter by relation:

```
// Find all snippets that describe auth.login
step id="s1" kind="query"
  target="project"
  select all
  from="snippets"
  where
    rel_to target="auth.login" type=describes
  end
  as="auth_docs"
end

// Find snippets with any 'contains' relation
step id="s2" kind="query"
  target="project"
  select all
  from="snippets"
  where
    has_rel type=contains
  end
  as="container_nodes"
end
```

---

## 10. Notes and Annotations

### 10.1 Comments vs Notes

Covenant distinguishes between ignored comments and queryable notes:

- **`// comments`** — Ignored by parser, not part of AST, for implementation notes
- **`note` keyword** — Part of AST, queryable, for semantic annotations

```
snippet id="auth.login" kind="fn"

  // This is an ignored comment (parser skips it)

  note "Authenticates user via bcrypt, returns JWT on success"

  note lang="pseudo"
    """
    1. Find user by email
    2. Verify password hash
    3. Generate JWT with user.id
    4. Return token or AuthError
    """
  end

  note lang="es" "Autentica usuario, devuelve token JWT"

  // ... rest of snippet
end
```

### 10.2 Querying Notes

Notes are queryable like any other metadata:

```
step id="s1" kind="query"
  target="project"
  select field="notes"
  from="functions"
  where equals field="id" lit="auth.login" end
  as="fn_notes"
end

// Find all snippets with Spanish translations
step id="s2" kind="query"
  target="project"
  select all
  from="snippets"
  where
    contains field="notes.lang" lit="es"
  end
  as="spanish_documented"
end
```

### 10.3 Use Cases

- **LLM-generated explanations:** Notes with `lang="en"` for human-readable descriptions
- **Pseudo-code:** Notes with `lang="pseudo"` for algorithm summaries
- **Translations:** Notes with `lang="es"`, `lang="fr"`, etc.
- **Provenance tracking:** Metadata fields `generated_by` and `human_reviewed`

---

## 11. Storage Architecture

### 11.1 Hybrid Model

Covenant uses a **hybrid storage model**:

- **Source of truth:** `.cov` text files (human-readable, git-friendly)
- **Derived index:** Pluggable key-value store for fast queries

```
project/
  src/
    auth.cov
    users.cov
    docs/
      overview.cov
  .covenant/
    index.db          # Derived index (LMDB, redb, etc.)
    symbol_graph.json # Optional: cached JSON export
```

### 11.2 Storage Provider Interface

The storage backend is pluggable. The spec defines **what** guarantees are needed, not **how** to implement:

**Layer 1: Core Operations (Required)**
```
get(id: String) -> Option<Node>
put(id: String, node: Node) -> Result<()>
delete(id: String) -> Result<()>
list(prefix: String) -> Iterator<Node>
```

**Layer 2: Index Queries (Required)**
```
query_by_kind(kind: String) -> Iterator<Node>
query_by_effect(effect: String) -> Iterator<Node>
query_by_relation(target: String, rel_type: RelationType) -> Iterator<Node>
query(pattern: QueryAST) -> Iterator<Node>
```

**Layer 3: Transactions (Required)**
```
transaction(ops: List<Op>) -> Result<()>
get_version(id: String) -> Int
```

**Guarantees:**
- All operations atomic
- Read-your-writes within transaction
- Serializable isolation (no dirty reads)
- Optimistic locking with version field for conflict detection

### 11.3 Recommended Backends

For ultrafast, local, in-process storage:

| Backend | Characteristics |
|---------|-----------------|
| **LMDB** | Memory-mapped, extremely fast reads, used by LDAP |
| **redb** | Pure Rust, simple API, embedded |
| **sled** | Modern, lock-free, still maturing |

The choice is **not embedded in the language spec** — implementations can swap backends as needed.

### 11.4 Index Structure

The derived index maintains:

```
snippets:
  id → AST node + source location

symbol_refs:
  (from_id, to_id, ref_type) → metadata

effects_index:
  effect_name → [snippet_id, ...]

relations_index:
  (target_id, rel_type) → [source_id, ...]

kind_index:
  kind → [snippet_id, ...]
```

**Benefits:**
- O(1) lookup by ID
- O(1) lookup by effect, kind, or relation
- Incremental updates without full recompilation
- Concurrent access from multiple tools

---

## 12. Tool Contracts

### 12.1 External Bindings

```
snippet id="http.get" kind="extern"

effects
  effect network
end

signature
  fn name="get"
    param name="url" type="String"
    returns union
      type="Response"
      type="HttpError"
    end
  end
end

metadata
  contract="axios.get@1"
  cost_hint=moderate
  latency_hint=slow
end

end
```

### 12.2 Operational Metadata

Tools can declare execution hints:

```
tools
  tool id="t1" contract="payments.charge@1"
    idempotent=idempotency_key   // which arg provides idempotency
    timeout=30s
    retry=(max=3 backoff=exponential)
    auth="payments:write"
  end
end
```

This enables AI planning — knowing a call is idempotent, slow, or requires specific permissions.

---

## 13. WASM Compilation

### Target: WASI 0.2 Component Model

Covenant compiles to **WASI 0.2 Component Model** modules for maximum portability:

- **Sandboxed execution** — memory-safe by default (WASM guarantee)
- **Capability-constrained** — effects map to WIT interface imports
- **Metered execution** — runtime fuel systems (Wasmtime, etc.)
- **Deterministic** — reproducible execution
- **Portable** — runs on any WASI 0.2 runtime (Wasmtime, Wasmer, wazero)

### Effect to Interface Mapping

| Covenant Effect | WIT Interface |
|-----------------|---------------|
| `effect network` | `wasi:http/outgoing-handler` |
| `effect filesystem` | `wasi:filesystem/types` |
| `effect storage` | `wasi:keyvalue/store` |
| `effect random` | `wasi:random/random` |
| `effect database` | `covenant:database/sql` (custom) |
| `effect std.concurrent` | `future<T>`, subtasks (WASI 0.3, Nov 2025) |

**Custom interfaces:** Database and project query capabilities use Covenant-defined WIT interfaces since WASI equivalents are immature or nonexistent.

### Compilation Targets

```bash
# WASI 0.2 Component Model (recommended for portability)
covenant compile --target=wasi app.cov

# JavaScript targets (for browser/Node.js environments)
covenant compile --target=browser app.cov
covenant compile --target=node app.cov
```

### Compilation Pipeline

```
IR (source)
    ↓
  Parser
    ↓
  Symbol Graph (queryable, bidirectional refs)
    ↓
  Type Checker (effects, capabilities)
    ↓
  IR Optimizer
    ↓
  WASM Emitter
    ↓
  ┌─────────────────────────────────────┐
  │ --target=wasi                        │
  │   → WASM Component (.wasm)           │
  │   → WIT interface imports            │
  ├─────────────────────────────────────┤
  │ --target=browser/node                │
  │   → Core WASM module (.wasm)         │
  │   → Generated JS glue (runtime.js)   │
  └─────────────────────────────────────┘
```

See [WASI_INTEGRATION.md](WASI_INTEGRATION.md) for detailed WIT definitions and host requirements.

---

## 14. Error Diagnostics

### 14.1 Error Message Structure

All compiler errors follow a structured JSON schema for machine readability and LLM self-correction:

```json
{
  "code": "E-TYPE-001",
  "severity": "error" | "warning" | "info",
  "message": "Cannot call effectful function from pure context",
  "source_location": {
    "file": "app.cov",
    "snippet_id": "module.function_name",
    "step_id": "s5",
    "line": 42,
    "column": 8
  },
  "context": {
    "callee_effects": ["database", "network"],
    "caller_effects": []
  },
  "suggestions": [
    {
      "type": "auto_fix",
      "description": "Add effects to caller signature",
      "confidence": 0.95,
      "edits": [
        {
          "operation": "insert_after",
          "target": "snippet[@id='foo'].signature",
          "content": "effects\n  effect database\n  effect network\nend"
        }
      ]
    },
    {
      "type": "query",
      "description": "Find all functions with database effect",
      "query": "target=project select all from=functions where contains field=effects lit=database"
    }
  ]
}
```

### 14.2 Error Code Categories

| Prefix | Category | Description |
|--------|----------|-------------|
| **E-PARSE-xxx** | Syntax errors | Invalid grammar, unexpected tokens, malformed blocks |
| **E-TYPE-xxx** | Type errors | Type mismatches, undefined types, incompatible unions |
| **E-EFFECT-xxx** | Effect violations | Pure functions calling effectful code, missing effect declarations |
| **E-REQ-xxx** | Requirement errors | Uncovered requirements, broken test linkage |
| **E-SYMBOL-xxx** | Symbol errors | Undefined references, duplicate IDs, circular imports |
| **W-DEAD-xxx** | Dead code warnings | Unused bindings, unreachable steps, uncalled functions |
| **W-PERF-xxx** | Performance warnings | Expensive queries, missing indexes, inefficient patterns |

### 14.3 Error Recovery Strategy

The compiler collects all errors rather than stopping on first failure:

1. **Parse errors** — reported immediately, parser attempts recovery at next block boundary
2. **Symbol errors** — collected during symbol graph construction
3. **Effect errors** — collected during effect closure computation
4. **Type errors** — collected during type checking
5. **Requirement errors** — collected during coverage validation

**Phase boundaries:** Parse errors must be resolved before type checking begins. Within each phase, all errors are reported together.

### 14.4 Suggestion Ranking

Auto-fix suggestions are ranked by confidence:

- **Confidence 1.0 (canonical)** — Single deterministic fix, always correct
- **Confidence 0.8-0.99 (ranked)** — Multiple valid fixes, ordered by likelihood
- **Confidence <0.8 (interactive)** — Requires user/LLM choice, ambiguous context

Suggestions may include:
- **auto_fix** — Direct code edits with AST transformations
- **query** — Project queries to find related code or patterns
- **refactor** — Multi-step transformations that preserve semantics

---

## 15. Auto-Fix Protocol

### 15.1 Fixable Error Declaration

Errors declare fixability via `auto_fix_kind`:

- **canonical** — Single deterministic fix (e.g., missing `end` keyword)
- **ranked** — Multiple valid fixes with confidence scores (e.g., ambiguous type inference)
- **interactive** — Requires user/LLM choice (e.g., ambiguous imports, design decisions)

### 15.2 Fix Application Semantics

Auto-fix application follows a strict validation protocol:

1. **Parse fix edits** into AST transformations
2. **Apply transformations** to in-memory AST
3. **Re-validate** — run all compiler phases on modified AST
4. **Check invariants** — verify symbol graph consistency (I1-I4, see section 14.2)
5. If validation fails → **rollback** to previous state
6. Return result: `success | failure(error_code)`

**Transaction guarantee:** All edits in a fix are applied atomically. Partial application is never committed.

### 15.3 Example: Effect Propagation Fix

**Error:** `E-EFFECT-002` — function calls effectful code but doesn't declare effects

**Auto-fix:**
```json
{
  "kind": "canonical",
  "confidence": 1.0,
  "description": "Add missing effect declarations",
  "edit": {
    "operation": "insert_after",
    "target": "snippet[@id='foo'].signature",
    "content": "effects\n  effect database\nend"
  }
}
```

### 15.4 Multi-Step Fixes

Complex fixes may require multiple coordinated edits:

```json
{
  "kind": "ranked",
  "confidence": 0.9,
  "description": "Extract common logic into new function",
  "edits": [
    {
      "operation": "insert_before",
      "target": "snippet[@id='main']",
      "content": "snippet id=\"utils.helper\" kind=\"fn\"...\nend"
    },
    {
      "operation": "replace",
      "target": "snippet[@id='main']/body/step[@id='s1']",
      "content": "step id=\"s1\" kind=\"call\" fn=\"helper\"..."
    }
  ]
}
```

All edits are validated together. If any edit fails, the entire fix is rejected.

---

## 16. Compilation Phases

The compiler is a multi-phase pipeline with explicit error boundaries:

```
IR Source (.cov files)
    ↓
┌───────────────────────────────────────┐
│ Phase 1: Parser                       │
│ Output: Raw AST (JSON)                │
│ Errors: E-PARSE-xxx                   │
└───────────────────────────────────────┘
    ↓
┌───────────────────────────────────────┐
│ Phase 2: Symbol Graph Builder         │
│ Output: Symbol table + forward refs   │
│ Errors: E-SYMBOL-xxx                  │
│ Validates: I1, I3, I4                 │
└───────────────────────────────────────┘
    ↓
┌───────────────────────────────────────┐
│ Phase 3: Effect Checker                │
│ Output: AST with effect closures      │
│ Errors: E-EFFECT-xxx                  │
│ Validates: I2 (effect transitivity)   │
└───────────────────────────────────────┘
    ↓
┌───────────────────────────────────────┐
│ Phase 4: Type Checker                  │
│ Output: Fully typed AST               │
│ Errors: E-TYPE-xxx                    │
└───────────────────────────────────────┘
    ↓
┌───────────────────────────────────────┐
│ Phase 5: Requirement Validator         │
│ Output: Coverage report               │
│ Errors: E-REQ-xxx                     │
└───────────────────────────────────────┘
    ↓
┌───────────────────────────────────────┐
│ Phase 6: IR Optimizer                  │
│ Output: Optimized IR                  │
│ Warnings: W-DEAD-xxx, W-PERF-xxx      │
└───────────────────────────────────────┘
    ↓
┌───────────────────────────────────────┐
│ Phase 7: WASM Emitter                  │
│ Output: .wasm binary                  │
│ Errors: Backend errors (rare)         │
└───────────────────────────────────────┘
```

### 16.1 Phase Outputs

Each phase produces structured output for the next:

**Phase 1 → Phase 2:** JSON AST with source locations
```json
{
  "snippets": [
    {
      "id": "module.func",
      "kind": "fn",
      "location": {"file": "app.cov", "line": 1, "col": 0},
      "sections": {...}
    }
  ]
}
```

**Phase 2 → Phase 3:** Symbol table with untyped references
```json
{
  "symbols": {
    "module.func": {
      "calls": ["other.func"],
      "references": ["User", "DbError"],
      "effects": null  // computed in phase 3
    }
  }
}
```

**Phase 3 → Phase 4:** Symbol table with effect closures
```json
{
  "symbols": {
    "module.func": {
      "effects": ["database", "network"],
      "effect_closure": ["database", "network", "filesystem"]
    }
  }
}
```

**Phase 4 → Phase 5:** Fully typed AST with type annotations on every expression

**Phase 5 → Phase 6:** Coverage report + validated AST

**Phase 6 → Phase 7:** Optimized IR (dead code eliminated, constants folded)

### 16.2 Error Accumulation

- **Within each phase:** Collect all errors before proceeding
- **Between phases:** Hard boundary — phase N+1 only runs if phase N succeeds
- **Exception:** Optimizer warnings do not block WASM emission

---

## 17. Query Semantics

### 17.1 Purity & Determinism

**Project queries are pure and deterministic:**
- Same query on same symbol graph → identical results
- Same ordering (lexicographic by ID if no `order_by` specified)
- No side effects — queries **cannot** mutate the symbol graph
- Safe to memoize and cache

**Database queries inherit database semantics:**
- Determinism depends on isolation level (see section 14.6)
- Can mutate database state via `insert`, `update`, `delete`
- Not memoized (side effects prevent safe caching)

### 17.2 Symbol Graph Consistency

**Immediate consistency guarantee:**
- Symbol graph updates are **atomic**
- All metadata recomputed **synchronously** on snippet modification
- Queries always see fully consistent state (no eventual consistency)

**Invariants (always maintained):**

- **I1 (Bidirectionality):** `A ∈ B.calls ⟺ B ∈ A.called_by`
  - If function A calls B, then B's `called_by` includes A

- **I2 (Effect Transitivity):** If A calls B and B has effect E, then A must declare E
  - Effect closure is computed transitively
  - Pure functions cannot call effectful functions

- **I3 (Coverage Linkage):** `T ∈ R.covered_by ⟺ R ∈ T.covers`
  - If test T covers requirement R, then R is in T's `covers` set

- **I4 (Acyclicity):** No circular imports
  - The import graph is a directed acyclic graph (DAG)
  - Detected during symbol graph construction

- **I5 (Relation Bidirectionality):** `A.rel_to(B, R) ⟺ B.rel_from(A, inverse(R))`
  - If snippet A declares `rel to="B" type=describes`, B automatically has `rel from="A" type=described_by`
  - Inverse mappings: `contains`↔`contained_by`, `describes`↔`described_by`, `next`↔`previous`, etc.
  - Orphan relations (referencing non-existent IDs) produce error E-REL-001

The compiler validates all five invariants before committing any symbol graph update.

### 17.3 Caching Strategy

**Project queries are memoized:**
- Cache key: `hash(query_AST, symbol_graph_version)`
- Cache invalidation: on any snippet modification (version bump)
- Cache eviction: LRU with configurable size limit (default 1000 entries)

**Database queries are NOT cached:**
- Database may change outside compiler's control
- Violates correctness unless read-only snapshot isolation is guaranteed

**Cache hit optimization:**
Project queries dominate compilation time in large codebases. Memoization provides:
- 100x speedup for repeated queries
- Incremental compilation support
- Fast IDE responsiveness (hover, go-to-definition)

### 17.4 Cost Model

**Project query complexity:**

| Operation | Complexity | Description |
|-----------|------------|-------------|
| `select all` | O(\|nodes in symbol graph\|) | Linear scan |
| `where` clause | O(\|nodes\| × predicate_cost) | Filter overhead |
| `join` | O(\|left\| × \|right\|) | Nested loop join (no indexes yet) |
| `limit/offset` | O(\|results\|) | Applied after filtering |
| `order_by` | O(\|results\| × log \|results\|) | Sort results |

**Cost hints become constraints:**
- `cost_hint=cheap` → query must complete in <10ms
- `cost_hint=moderate` → query must complete in <100ms
- `cost_hint=expensive` → query may take >1s but <10s
- Use `timeout=30s` metadata to override (fails if exceeded)

**Compiler rejection:** If static analysis determines a query will exceed its cost hint, compilation fails with `E-QUERY-001: Query exceeds cost budget`.

### 17.5 Null Handling

**Three-valued logic in `where` clauses:**
- `equals field="x" lit=none` → TRUE if x is null
- `not_equals field="x" lit=none` → TRUE if x is not null
- Comparisons with null (`less`, `greater`) → UNKNOWN → row excluded from results

**Null propagation in operations:**
- Arithmetic: `add(null, 5)` → `null`
- Logic: `and(null, true)` → `null` (use `and_then` for short-circuit)
- String: `concat(null, "x")` → `null`

**Optional types compile to unions:**
```
type="User" optional
↓ (desugars to)
union
  type="User"
  type="None"
end
```

### 17.6 Transactionality

**Database transactions:**
- Explicit `transaction` block (see section 15.3)
- ACID guarantees inherited from target database
- Default isolation: `READ COMMITTED`
- Override with: `isolation=serializable`

**Symbol graph transactions:**
- Snippet modifications are atomic
- Multi-snippet updates: wrap in `refactor` block (see section 15.4)
- Rollback on validation failure (invariants I1-I4)

### 17.7 Query Optimization

**Project queries (current):**
- No user-visible indexes
- Compiler may reorder filters for efficiency
- No join optimization (nested loop only)
- Future: `index on="effects"` pragma for frequently queried fields

**Database queries:**
- Compiled to SQL — database optimizer handles it
- Future: `hint="use_index(users_email_idx)"` for manual optimization

---

## 18. Incremental Compilation

### 18.1 Symbol Graph Updates

When a snippet is modified, the compiler performs incremental recomputation:

**Step 1: Parse modified snippet**
```
new_snippet = parse(source)
```

**Step 2: Invalidate dependent metadata**
```
for func_id in old_snippet.calls:
    symbol_table[func_id].called_by.remove(old_snippet.id)

for symbol_id in old_snippet.references:
    symbol_table[symbol_id].referenced_by.remove(old_snippet.id)

for req_id in old_snippet.tests.covers:
    symbol_table[req_id].covered_by.remove(old_snippet.id)
```

**Step 3: Recompute local metadata**
```
new_snippet.calls = extract_calls(new_snippet.body)
new_snippet.references = extract_references(new_snippet)
new_snippet.effects = new_snippet.effects_section
```

**Step 4: Propagate updates**
```
for func_id in new_snippet.calls:
    symbol_table[func_id].called_by.add(new_snippet.id)

# Recompute effect closure transitively
new_snippet.effect_closure = compute_effect_closure(new_snippet)
```

**Step 5: Validate invariants**
```
assert validate_I1(symbol_table)  # Bidirectionality
assert validate_I2(symbol_table)  # Effect transitivity
assert validate_I3(symbol_table)  # Coverage linkage
assert validate_I4(symbol_table)  # Acyclicity

if any validation fails:
    rollback to old_snippet
    return error
```

**Step 6: Bump symbol graph version**
```
symbol_graph.version += 1
query_cache.clear()  # Invalidate all cached queries
```

### 18.2 Snippet Deletion

When a snippet is deleted:

**Step 1: Check for references**
```
if snippet.called_by is not empty:
    return error("Cannot delete, still referenced by: {snippet.called_by}")

if snippet.referenced_by is not empty:
    return error("Cannot delete, still referenced by: {snippet.referenced_by}")
```

**Step 2: Clear bidirectional links**
```
for callee_id in snippet.calls:
    symbol_table[callee_id].called_by.remove(snippet.id)

for ref_id in snippet.references:
    symbol_table[ref_id].referenced_by.remove(snippet.id)
```

**Step 3: Remove from symbol table**
```
del symbol_table[snippet.id]
symbol_graph.version += 1
```

### 18.3 Transaction Block Syntax

For database operations requiring atomicity:

```
step id="s1" kind="transaction"
  step id="s1a" kind="insert"
    into="app_db.orders"
    set field="user_id" from="user_id"
    set field="total" from="total"
    as="order"
  end

  step id="s1b" kind="update"
    target="app_db.inventory"
    set field="quantity" op=sub from="quantity"
    where
      equals field="product_id" var="product_id"
    end
    as="updated"
  end

  as="transaction_result"
end
```

**Semantics:**
- All steps execute in single database transaction
- If any step fails, **rollback all** (ACID atomicity)
- Isolation level: configurable via `isolation=serializable`

### 18.4 Refactor Block Syntax

For multi-snippet transformations:

```
refactor id="rename_function"
  step id="r1" kind="update_snippet"
    target="module.old_name"
    set field="signature.fn.name" lit="new_name"
  end

  step id="r2" kind="query"
    target="project"
    select all
    from="steps"
    where
      and
        equals field="kind" lit="call"
        equals field="fn" lit="old_name"
      end
    end
    as="call_sites"
  end

  step id="r3" kind="update_all"
    target="call_sites"
    set field="fn" lit="new_name"
  end
end
```

**Semantics:**
- All steps execute in a transaction
- If any step fails, **rollback all**
- Symbol graph only updated after final validation
- Atomically bump version once at the end

**Validation:**
- After all edits, run full compilation pipeline
- Check invariants I1-I4
- If validation fails, restore original state

---

## 19. Structured Concurrency

### 19.1 Design Philosophy

Covenant does **not** support multi-threading, async/await keywords, or shared mutable state. Instead, it provides **structured concurrency** through extensible kinds imported via the effects system.

**Why no threads/async:**
- Threads introduce non-determinism (violates core principle)
- Async/await creates "colored functions" problem
- Shared mutable state requires locks (complexity)
- LLMs struggle to reason about concurrent code

**Why structured concurrency:**
- Declarative — one pattern to learn
- Deterministic — results always in declaration order
- Scoped — concurrency is contained, not viral
- Effect-tracked — explicit capability declaration

### 19.2 The std.concurrent Effect

Import concurrency primitives via the effect system:

```
effects
  effect std.concurrent
  effect network
end
```

This makes available:
- `std.concurrent.parallel` — execute branches concurrently, wait for all
- `std.concurrent.race` — execute branches concurrently, return first to complete

### 19.3 Parallel Execution

```
step id="s1" kind="std.concurrent.parallel"
  branch id="b1"
    step id="b1.1" kind="call"
      fn="http.get"
      arg name="url" lit="https://api.example.com/users"
      as="users"
    end
  end

  branch id="b2"
    step id="b2.1" kind="call"
      fn="http.get"
      arg name="url" lit="https://api.example.com/products"
      as="products"
    end
  end

  as="results"  // Struct with users, products fields
end
```

**Semantics:**
- All branches start concurrently
- Block completes when ALL branches complete
- Results collected in declaration order (deterministic)
- Branches are isolated — no shared mutable state

### 19.4 Error Handling

```
step id="s1" kind="std.concurrent.parallel"
  on_error="fail_fast"  // default
  ...
end
```

| Strategy | Behavior |
|----------|----------|
| `fail_fast` | Cancel remaining branches on first error (default) |
| `collect_all` | Wait for all branches, collect errors into result |
| `ignore_errors` | Replace failed branches with `none` |

### 19.5 Timeouts

```
step id="s1" kind="std.concurrent.parallel"
  timeout=5s
  on_timeout="cancel"
  ...
end
```

### 19.6 Race Pattern

Return the first branch to complete:

```
step id="s1" kind="std.concurrent.race"
  branch id="b1"
    step id="b1.1" kind="call"
      fn="cache.get"
      arg name="key" from="user_id"
      as="cached"
    end
  end

  branch id="b2"
    step id="b2.1" kind="call"
      fn="db.query"
      arg name="id" from="user_id"
      as="fresh"
    end
  end

  as="first_result"
end
```

### 19.7 What's NOT Supported

- **Fire-and-forget** — All concurrent work is scoped; you always wait for results
- **Inter-branch communication** — Branches cannot share state or signal each other
- **Unbounded spawning** — No "spawn and forget" pattern
- **Callbacks** — No callback-based async APIs

See [EXTENSIBLE_KINDS.md](EXTENSIBLE_KINDS.md) for how `std.concurrent` is defined as an effect-kind.

---

## 20. Extensible Kinds

### 20.1 Core Concept

The `kind` attribute in snippets and steps is **extensible**. While core kinds (`fn`, `data`, `extern`) are built-in, additional kinds can be imported via the effects system.

### 20.2 Importing Kinds

```
effects
  effect std.concurrent  // makes parallel, race kinds available
end

body
  step id="s1" kind="std.concurrent.parallel"
    ...
  end
end
```

### 20.3 Namespacing

Kinds are fully qualified by their effect: `effect.kindname`

- `std.concurrent.parallel` — standard library
- `std.testing.mock` — standard library
- `acme.workflow.approval` — organization-specific

### 20.4 Defining Custom Kinds

Use `kind="effect-kind"` to define new kinds:

```
snippet id="myorg.custom" kind="effect-kind"

kinds
  kind name="my_construct"
    structure
      section name="item" multiple=true required=true
        contains kind="step"
      end
    end
    compile_to="myorg_runtime"
  end
end

effects_required
  effect myorg.runtime
end

end
```

See [EXTENSIBLE_KINDS.md](EXTENSIBLE_KINDS.md) for full specification.

---

## 21. Human-Readable Views (Future)

The IR is the source of truth. Human-readable views are derived:

| View | Purpose |
|------|---------|
| **Pretty print** | Compact syntax for code review |
| **Diff view** | Semantic diff, not textual |
| **Graph view** | Visual call graph, dependency graph |
| **Summary view** | Natural language description |

These are display transformations, not source formats. The IR remains canonical.

---

## 22. What This Is and Isn't

**Is:**
- A machine-first intermediate representation
- A queryable project format
- An effect-tracked, capability-constrained language
- Optimized for LLM generation and navigation

**Is Not:**
- A replacement for all languages
- A natural-language programming system
- Designed for human hand-authoring (though possible)

---

## 23. North Star

> **AI generates canonical IR.**
> **Compilers derive queryable graphs.**
> **Tools execute with explicit capabilities.**
> **Every node is addressable, every relationship explicit.**

---

## Related Documents

- [grammar.ebnf](grammar.ebnf) — Formal syntax definition
- [ERROR_CODES.md](ERROR_CODES.md) — Comprehensive error catalog with auto-fix strategies
- [COMPILER.md](COMPILER.md) — Detailed compilation phase specifications
- [QUERY_SEMANTICS.md](QUERY_SEMANTICS.md) — Formal operational semantics for queries
- [STORAGE.md](STORAGE.md) — Storage provider interface specification
- [EXTENSIBLE_KINDS.md](EXTENSIBLE_KINDS.md) — Pluggable kind system specification
- [prior-art.md](prior-art.md) — Lessons from Austral, Koka, and LLM-native design
- [examples/](examples/) — Example programs in IR syntax
