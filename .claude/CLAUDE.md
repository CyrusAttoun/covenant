# Covenant

A machine-first programming language designed for LLM generation and navigation. Compiles to WASM.

---

## Documents

| Doc | Purpose |
|-----|---------|
| [DESIGN.md](../DESIGN.md) | Philosophy, four-layer model, core design, compiler semantics |
| [grammar.ebnf](../grammar.ebnf) | Formal syntax definition (machine-parseable) |
| [ERROR_CODES.md](../ERROR_CODES.md) | Comprehensive error catalog with auto-fix strategies |
| [COMPILER.md](../COMPILER.md) | Detailed compilation phase specifications |
| [QUERY_SEMANTICS.md](../QUERY_SEMANTICS.md) | Formal operational semantics for queries |
| [examples/](../examples/) | Example `.cov` programs |
| [prior-art.md](../prior-art.md) | Lessons from Austral, Koka, and LLM-native design |

---

## Specifications

| Spec | Purpose |
|------|---------|
| [LLM Code Generation](../docs/specs/llm-code-generation.md) | LLM-based code generation system with compiler validation and self-correction |

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
    target="project"
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

### Query System

Two paths: **Covenant dialect** (default) for Covenant types, **SQL dialects** for external databases.

#### Covenant Queries (Default)

Simple, typed syntax for querying Covenant types (project AST, structs, collections):

```
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
```

Supported: `select all/field`, `from`, `where`, `join`, `follow rel`, `order`, `limit`

#### SQL Dialect Queries

For external databases, use opaque `body ... end` blocks with full SQL power:

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
  end
  params
    param name="cutoff" from="cutoff_date"
  end
  returns collection of="UserOrderStats"
  as="high_volume_users"
end
```

**Key points:**
- `dialect` required (postgres, sqlserver, mysql, sqlite)
- `body ... end` contains raw SQL (not parsed by Covenant)
- `params` declares parameter bindings
- `returns` type annotation required

### CRUD Operations (Covenant Types)

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
  as="_"
end
```

For external databases, use SQL dialect blocks.

### Database Bindings

```
snippet id="db.app_db" kind="database"

metadata
  type="database"
  dialect="postgres"
  connection="env:APP_DB_URL"
end

schema
  table name="users"
    field name="id" type="Int" primary_key=true
    field name="email" type="String"
  end
end

end
```

### Null Handling

`none` represents absence. In queries:
```
where
  equals field="deleted_at" lit=none    // Check for null
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
