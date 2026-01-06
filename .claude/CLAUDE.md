# Covenant

A machine-first programming language designed for LLM generation and navigation. Compiles to WASM.

---

## Documents

| Doc | Purpose |
|-----|---------|
| [DESIGN.md](../DESIGN.md) | Philosophy, four-layer model, core design |
| [grammar.ebnf](../grammar.ebnf) | Formal syntax definition (machine-parseable) |
| [examples/](../examples/) | Example `.cov` programs |
| [prior-art.md](../prior-art.md) | Lessons from Austral, Koka, and LLM-native design |

---

## Quick Reference

### Core Principles
- **Machine-first IR** — deterministic, tree-shaped, keyword-heavy syntax
- **No operators** — use keywords: `add`, `equals`, `and`, `or`, `not`
- **SSA form** — one operation per step, named outputs (`as="result"`)
- **Canonical ordering** — one valid way to write everything
- **Every node has an ID** — enables precise queries and references
- **Effects explicit** — declared in `effects` section, propagated transitively
- **Requirements first-class** — specs and tests are queryable nodes
- **WASM target** — sandboxed, capability-constrained, metered

### Snippet Structure

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
  step id="s1" kind="query"
    target="app_db"
    select all
    from="users"
    where
      equals field="id" var="id"
    end
    limit=1
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

tests
  test id="T-001" kind="unit" covers="R-001"
    // test steps
  end
end

end
```

### Operations (No Operators)

| Instead of | Use |
|------------|-----|
| `x + y` | `op=add input var="x" input var="y"` |
| `x == y` | `op=equals input var="x" input var="y"` |
| `x && y` | `op=and input var="x" input var="y"` |
| `!x` | `op=not input var="x"` |

### Query Syntax

Same syntax for database and AST queries. Target determines compilation:

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

### CRUD Operations

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
  set field="is_active" lit=false
  where
    less field="last_login" var="cutoff"
  end
  as="updated"
end

// Delete
step id="s3" kind="delete"
  from="app_db.users"
  where
    equals field="id" var="user_id"
  end
  as="_"
end
```

### Null Handling

`none` represents absence. In queries:
```
where
  equals field="deleted_at" lit=none    // → IS NULL
end
```

### External Bindings

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

### Bidirectional References

Compiler computes metadata on every symbol:
```
symbol_metadata = {
    called_by: [symbol_id],
    calls: [symbol_id],
    references: [symbol_id],
    referenced_by: [symbol_id],
    effects: [effect_id],
    tests: [test_id],
    requirements: [req_id],
}
```

---

## Status

**Design phase.** No compiler exists yet.

Current focus: finalize IR syntax, define AST schema, build parser.
