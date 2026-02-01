# Covenant

A machine-first programming language designed for LLM generation and navigation. Compiles to WASM. Default runtime target is Deno.

---

## Documentation

### Quick Start by Goal

**"I want to learn Covenant"**
1. [Tutorial](../docs/guide/tutorial.md) - Hello World and basics
2. [Reading Guide](../docs/guide/reading-guide.md) - How to read Covenant code
3. [Syntax Examples](../docs/guide/syntax-examples.md) - Cheat sheet
4. [Patterns](../docs/guide/patterns.md) - Common idioms

**"I want to understand the language design"**
1. [Design](../docs/design/DESIGN.md) - Philosophy and four-layer model
2. [Query Semantics](../docs/design/QUERY_SEMANTICS.md) - Query system spec
3. [Compiler](../docs/design/COMPILER.md) - Compilation phases
4. [Grammar](../docs/design/grammar.ebnf) - Formal syntax

**"I want to build LLM integrations"**
1. [LLM Code Generation](../docs/specs/llm-code-generation.md) - Generation system
2. [Explain Generator](../docs/specs/explain-generator.md) - Explanation algorithm
3. [Comment Generator](../docs/specs/comment-generator.md) - Auto-documentation

### Document Index

| Directory | Purpose |
|-----------|---------|
| [docs/guide/](../docs/guide/) | Learning materials for language users |
| [docs/design/](../docs/design/) | Language design and compiler specifications |
| [docs/specs/](../docs/specs/) | LLM and tooling integration specifications |
| [examples/](../examples/) | Example `.cov` programs |

---

## Quick Reference

### Core Principles
- **Machine-first IR** — deterministic, tree-shaped, keyword-heavy syntax
- **No operators** — use keywords: `add`, `equals`, `and`, `or`, `not`
- **SSA form** — one operation per step, named outputs (`as="result"`)
- **Canonical ordering** — one valid way to write everything
- **Every node has an ID** — enables precise queries and references
- **Effects explicit** — declared in `effects` section, propagated transitively
- **Parameterized effects** — capability narrowing via `effect filesystem(path="/data")`
- **Runtime enforcement** — WASM imports gated by declared effects
- **Requirements first-class** — specs and tests are queryable nodes
- **WASM target** — sandboxed, capability-constrained, metered

### Snippet Structure

```
snippet id="module.function_name" kind="fn"

effects
  effect database
  effect network
  effect filesystem(path="/data")   (* parameterized effect *)
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

### Call Steps

Function calls use **fully-qualified snippet IDs** (not short names):

```
step id="s1" kind="call"
  fn="console.println"           // Must be the snippet ID
  arg name="message" lit="Hello"
  as="_"
end

step id="s2" kind="call"
  fn="math.factorial"            // Recursive calls also use snippet ID
  arg name="n" from="n_minus_1"
  as="result"
end
```

**Rationale:** Using canonical snippet IDs ensures:
- Unambiguous references (no scope resolution needed)
- One valid way to write every call (canonical form)
- Simpler compiler (exact string matching)
- Better LLM code generation (no implicit context)

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
- `dialect` required (postgres, sqlserver, mysql, sqlite, indexeddb)
- `body ... end` contains raw SQL (not parsed by Covenant)
- `params` declares parameter bindings
- `returns` type annotation required

#### IndexedDB Dialect (Cross-Platform Storage)

For cross-platform document storage, use `dialect="indexeddb"` with Covenant query syntax:

```
step id="s1" kind="query"
  dialect="indexeddb"
  target="std.storage"
  select all
  from="users"
  where
    and
      equals field="status" lit="active"
      greater field="age" lit=18
    end
  end
  order by="created_at" dir="desc"
  limit=10
  as="active_users"
end
```

**Key points:**
- Uses Covenant query syntax (not opaque body blocks)
- Compiles to IndexedDB (browser), SQLite (Node.js), or embedded DB (WASI)
- Same query works across all platforms
- Requires `effect std.storage`

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

### Parameterized Effects

Effects can take parameters for capability narrowing:

```
effects
  effect filesystem(path="/data")     (* restrict to /data directory *)
  effect database(readonly=true)      (* read-only database access *)
end
```

**Subsumption rules:**
- `effect filesystem` (no params) subsumes all `filesystem(path=...)` variants
- `effect filesystem(path="/data")` subsumes `filesystem(path="/data/users")`
- Parameters must match or be subsumed for effect validation to pass

**Example:**
```
(* This function declares narrow filesystem access *)
snippet id="files.read_config" kind="fn"

effects
  effect filesystem(path="/etc/app")
end

(* ... *)
end

(* Caller must declare at least the same or broader access *)
snippet id="app.init" kind="fn"

effects
  effect filesystem(path="/etc")      (* broader - OK *)
end

(* ... *)
end
```

### Runtime Effect Enforcement

WASM imports are gated at runtime based on declared effects:

```typescript
// TypeScript host (runtime/host/src/capabilities.ts)
const EFFECT_TO_IMPORTS = {
  database: ["db.execute_query"],
  network: ["http.fetch"],
  filesystem: ["fs.read", "fs.write", ...],
  console: ["console.println", ...],
  "std.storage": ["std.storage.kv.get", ...],
};
```

**Enforcement modes:**
- **strict** (default): Undeclared imports throw errors
- **warnOnly**: Log warnings but allow execution
- **disabled**: Allow all imports (backwards compatibility)

**How it works:**
1. Compiler embeds `required_capabilities` in WASM data section
2. Host extracts `CapabilityManifest` at module load
3. `buildFilteredImports()` provides only permitted imports
4. Unpermitted imports get error stubs

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

### Structured Concurrency

Parallel I/O without threads or async/await. `parallel` and `race` are built-in step kinds.

```
body
  step id="s1" kind="parallel"
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
    as="results"
  end
end
```

**Key points:**
- `parallel` — execute branches concurrently, wait for all
- `race` — execute branches, return first to complete
- Built-in step kinds (no effect import needed)
- Results in declaration order (deterministic)
- Branches isolated — no shared mutable state
- `on_error="fail_fast"` (default), `"collect_all"`, or `"ignore_errors"`
- `timeout=5s` with `on_timeout="cancel"` or `"return_partial"`

### Cross-Platform Storage (`std.storage`)

Unified key-value and document storage that works on browser, Node.js, and WASI.

#### Key-Value Store (`std.storage.kv`)

```
effects
  effect std.storage
end

body
  step id="s1" kind="call"
    fn="std.storage.kv.set"
    arg name="key" lit="user:theme"
    arg name="value" lit="dark"
    as="_"
  end

  step id="s2" kind="call"
    fn="std.storage.kv.get"
    arg name="key" lit="user:theme"
    as="theme"
  end
end
```

| Function | Description |
|----------|-------------|
| `std.storage.kv.set` | Store a value |
| `std.storage.kv.get` | Retrieve a value (returns optional) |
| `std.storage.kv.delete` | Delete a value |
| `std.storage.kv.has` | Check if key exists |
| `std.storage.kv.list` | List keys by prefix |
| `std.storage.kv.clear` | Clear keys (all or by prefix) |

#### Document Store (`std.storage.doc`)

```
effects
  effect std.storage
end

body
  (* Using query dialect *)
  step id="s1" kind="query"
    dialect="indexeddb"
    target="std.storage"
    select all
    from="users"
    where
      equals field="status" lit="active"
    end
    as="active_users"
  end

  (* Using function API *)
  step id="s2" kind="call"
    fn="std.storage.doc.put"
    arg name="collection" lit="users"
    arg name="id" lit="user123"
    arg name="data" from="user_json"
    as="doc"
  end
end
```

| Function | Description |
|----------|-------------|
| `std.storage.doc.put` | Insert/update document |
| `std.storage.doc.get` | Get document by ID |
| `std.storage.doc.delete` | Delete document |
| `std.storage.doc.query` | Query with JSON filter |
| `std.storage.doc.count` | Count matching documents |
| `std.storage.doc.create_index` | Create index for faster queries |

**Platform backends:**
- Deno: Deno KV (both kv and doc) — default target
- Browser: localStorage (kv), IndexedDB (doc)
- Node.js: Files (kv), SQLite (doc)
- WASI: Preopened dir (kv), Embedded DB (doc)

### Canonical Text Printer

The compiler can serialize AST back to canonical `.cov` format:

```bash
# Format a file (outputs to stdout)
covenant format examples/hello-world.cov

# Check if file is canonical (exit 1 if not)
covenant format --check examples/hello-world.cov

# Write to file
covenant format examples/hello-world.cov --output formatted.cov
```

**Use cases:**
- AI code generation feedback loops (generate → canonicalize → validate)
- Consistent formatting across the codebase
- Round-trip testing (parse → print → parse → compare)

**Implementation:** `crates/covenant-ast/src/printer.rs` with `ToCov` trait.

### Diagnostic Tooling

Enhanced error messages with fix suggestions and effect chain explanations:

```bash
# Verbose diagnostics with --explain
covenant check --explain examples/missing-effect.cov
```

**Example output:**
```
error[E-EFFECT-002]: Missing effect declaration
  --> examples/api.cov:15:3
   |
15 |   step id="s1" kind="call"
   |   ^^^^^^^^^^^^^^^^^^^^^^^^ calls effectful function
   |
   = note: `api.fetch_user` calls `db.query` which requires `database` effect
   = note: Call chain: api.fetch_user → user.get_by_id → db.query (database)
   = help: Add `effect database` to the effects section

  Suggestion:
    effects
      effect database    (* add this line *)
    end
```

**Features:**
- Rich error context with source spans
- Call chain tracing for effect violations
- Fix suggestions (AddEffect, RemoveCall, WrapInEffectfulFunction)
- Ariadne-powered structured output

**Implementation:** `crates/covenant-checker/src/diagnostics.rs`

---

## Plan Mode Instructions

**IMPORTANT: Follow these rules when creating or managing plans.**

### Writing Plans
- **Location**: Always write plans to `.claude/plans/` in the project root (never to `~/.claude/plans/`)
- **Naming**: Use descriptive kebab-case filenames derived from the plan title
  - Example: A plan titled "# Implement Query Optimizer" → `implement-query-optimizer.md`
  - Keep filenames under 60 characters
  - If a file with that name exists, append `-2`, `-3`, etc.
- **Format**: Start every plan with a `# Title` heading

### Plan Lifecycle
1. **Active plans** live in `.claude/plans/`
2. **When implementation is complete**: Move the plan to `.claude/implemented_plans/`
   - Add `## Status: Implemented` at the top (below the title) before moving
   - Preserve the original filename

### Directory Structure
```
.claude/
├── plans/                    # Active plans being worked on
│   └── implement-feature-x.md
├── implemented_plans/        # Archived completed plans
│   └── add-user-auth.md
└── CLAUDE.md
```

---

## Status

**Active development.** Compiler pipeline implemented and functional (13 crates, CLI, WASM codegen).

### Recently Implemented
- **Runtime effect enforcement** — WASM imports gated by declared effects
- **Parameterized effects** — `effect filesystem(path="/data")` with subsumption
- **Canonical text printer** — AST → `.cov` serialization via `covenant format`
- **Enhanced diagnostics** — Fix suggestions and effect chain explanations

### Current Focus
- Structured concurrency (`parallel` / `race` step kinds)
- Cross-platform storage (`std.storage`)
- Cross-snippet type checking
