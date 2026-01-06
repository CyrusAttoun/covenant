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

### 5.1 Unified Query Syntax

Same syntax for database queries and AST/project queries. The `target` determines compilation:

```
// Database query (compiles to SQL)
step id="s1" kind="query"
  target="app_db"
  select all
  from="users"
  where
    equals field="id" var="user_id"
  end
  limit=1
  as="user"
end

// Project query (compiles to AST traversal)
step id="s1" kind="query"
  target="project"
  select all
  from="functions"
  where
    contains field="effects" lit="database"
  end
  as="db_functions"
end
```

### 5.2 CRUD Operations

```
// Insert
step id="s1" kind="insert"
  into="app_db.users"
  set field="name" from="name"
  set field="email" from="email"
  as="new_user"
end

// Update
step id="s2" kind="update"
  target="app_db.users"
  set field="is_active" from="false"
  where
    less field="last_login" var="cutoff_date"
  end
  as="updated_count"
end

// Delete
step id="s3" kind="delete"
  from="app_db.users"
  where
    equals field="id" var="user_id"
  end
  as="deleted"
end
```

### 5.3 Null Handling

`none` represents absence. In queries:

```
where
  equals field="deleted_at" lit=none    // → IS NULL
end

where
  not_equals field="deleted_at" lit=none  // → IS NOT NULL
end
```

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

## 9. Tool Contracts

### 9.1 External Bindings

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

### 9.2 Operational Metadata

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

## 10. WASM Compilation

### Target Runtime
- **Sandboxed execution** — memory-safe by default
- **Capability-constrained** — WASI for controlled host access
- **Metered execution** — fuel-based limits
- **Deterministic** — reproducible execution
- **Portable** — runs anywhere WASM runs

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
  .wasm Module
```

---

## 11. Human-Readable Views (Future)

The IR is the source of truth. Human-readable views are derived:

| View | Purpose |
|------|---------|
| **Pretty print** | Compact syntax for code review |
| **Diff view** | Semantic diff, not textual |
| **Graph view** | Visual call graph, dependency graph |
| **Summary view** | Natural language description |

These are display transformations, not source formats. The IR remains canonical.

---

## 12. What This Is and Isn't

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

## 13. North Star

> **AI generates canonical IR.**
> **Compilers derive queryable graphs.**
> **Tools execute with explicit capabilities.**
> **Every node is addressable, every relationship explicit.**

---

## Related Documents

- [grammar.ebnf](grammar.ebnf) — Formal syntax definition
- [prior-art.md](prior-art.md) — Lessons from Austral, Koka, and LLM-native design
- [examples/](examples/) — Example programs in IR syntax
