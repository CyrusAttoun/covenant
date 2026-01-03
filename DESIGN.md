# Covenant: Design Philosophy

> **Natural language is for requesting.**
> **Source code is for meaning.**
> **AST is for tooling.**
> **Bytecode is for execution.**

---

## 1. Why Covenant Exists

AI coding agents struggle with traditional codebases:

- They parse **text**, not meaning
- They **search and guess** instead of query
- Tool use is **probabilistic**, not contractual
- Each task starts from scratch—no compounding benefit
- Reliability degrades as projects grow

The root cause: **existing languages optimize for human authorship, not machine comprehension.**

Covenant is a coordination language with a contract-first type system. The value is not in the syntax. The value is in:

1. **Tool contracts** — typed interfaces for external capabilities
2. **Queryable structure** — AST that tooling and AI can efficiently navigate
3. **Compounding clarity** — each feature written makes the next easier

---

## 2. The Four-Layer Model

### 2.1 Natural Language — *Requesting*
Ephemeral. Ambiguous by nature. Human-to-AI communication only. Discarded after translation.

### 2.2 Source Code — *The Artifact*
Human-readable and human-writable. Heavily typed, minimal syntax. Encodes intent, contracts, and constraints. The permanent project asset.

### 2.3 AST — *Tooling & AI Interface*
Derived from source. Queryable symbol graph with bidirectional references. Lossless round-trip to source. What AI agents operate against.

### 2.4 Bytecode — *Execution*
WASM target. Deterministic. Sandboxed and capability-constrained. Metered execution.

---

## 3. Core Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Equality operator | `=` (not `==`) | Aligns with SQL, avoids confusion |
| Assignment operator | `:=` | Distinct from equality |
| Effect declaration | Function-level imports | Imports *are* the effect declaration |
| Effect propagation | Transitive via imports | Importing a function imports its effects |
| Function keyword | None | Signature shape identifies functions |
| Error handling | Union returns + auto-propagation | Errors bubble up; use `handle` to catch |
| Null handling | `none` everywhere | `x = none` compiles to `IS NULL` in queries |
| Optional types | `?` suffix | `User?` instead of `Option<User>` |
| List syntax | `[]` suffix | `User[]` instead of `List<User>` |
| Query syntax | SQL-like, unified | Same syntax for databases and source code |
| Bidirectional refs | Compiler-computed | Every function knows its callers |
| Macros | None | Predictable for AI |

See [grammar.ebnf](grammar.ebnf) for the formal syntax definition.
See [examples/](examples/) for validated source code.

---

## 4. Imports as Effects

Most computation happens in **external tools**—other languages, APIs, system utilities, sandboxed runtimes.

Effects are declared via **imports**. Function-level imports scope capabilities precisely:

```covenant
// Effectful — imports database capability
get_user(id: Int) -> User | DbError
    import { app_db } from database
{
    query app_db {
        select * from users where id = id limit 1
    }
}

// Pure — no imports, no effects
validate_email(email: String) -> Bool {
    email.contains("@") && email.len() > 3
}
```

**Key principles:**
- **Imports are effects** — `import { app_db } from database` brings the `[database]` capability into scope
- **Function-level granularity** — each function declares exactly what it needs
- **Transitive propagation** — importing a function imports its effects

---

## 5. Unified Query System

Covenant uses SQL-like syntax for querying **all data sources**. The import type determines how the query compiles.

### Database Queries (compile to SQL)

```covenant
get_active_users() -> User[] | DbError
    import { app_db } from database
{
    query app_db {
        select * from users where is_active = true order by name
    }
}
```

### Project Queries (compile to AST traversal)

```covenant
find_db_functions() -> FunctionInfo[]
    import { project } from meta
{
    query project {
        select * from functions where effects contains "database"
    }
}
```

### CRUD Operations

Same syntax for both targets:

```covenant
// Database
insert into app_db.users { name, email, created_at: now() }
update app_db.users set is_active: false where last_login < days_ago(30)
delete from app_db.users where id = user_id

// Source code (AST mutations)
insert into project.functions { name: "new_fn", module: "auth", ... }
update project.functions set name: "new_name" where name = "old_name"
delete from project.functions where called_by = [] and is_exported = false
```

### Null Handling

`none` represents absence of value. In queries, it maps to SQL NULL:

```covenant
where deleted_at = none      // → WHERE deleted_at IS NULL
where deleted_at != none     // → WHERE deleted_at IS NOT NULL
```

---

## 6. Bidirectional References

The compiler computes metadata on every symbol during type-checking:

```
ast_metadata = {
    called_by: [symbol_id],      // functions that call this function
    calls: [symbol_id],          // functions this function calls
    references: [symbol_id],     // types/symbols this references
    referenced_by: [symbol_id],  // what references this
    effects: [effect_id],        // computed effect set (transitive)
}
```

This enables queries like:

```covenant
// Find all callers of authenticate (no grep required)
query project {
    select * from functions where calls contains "authenticate"
}

// Find unused code
query project {
    select * from functions
    where called_by = []
      and is_exported = false
      and is_entry_point = false
}
```

---

## 7. External Bindings (FFI)

Covenant accesses the JavaScript/npm ecosystem via **extern declarations**:

```covenant
extern get(url: String) -> Response | HttpError
    from "axios"
    effect [network]
```

Architecture:
```
┌─────────────────────────────────┐
│  Covenant (.cov)                │  ← Effect-tracked, type-safe
├─────────────────────────────────┤
│  Extern Bindings                │  ← Thin typed wrappers
├─────────────────────────────────┤
│  Host Runtime (JS/Node)         │  ← Provides implementations
├─────────────────────────────────┤
│  npm Libraries                  │  ← The ecosystem
└─────────────────────────────────┘
```

---

## 8. Database Modules

Typed database schemas that compile queries to SQL:

```covenant
database app_db
    connection: "postgres://localhost:5432/myapp"
{
    table users {
        id: Int primary auto
        email: String unique
        name: String
        deleted_at: DateTime nullable  // becomes DateTime? in queries

        index(email)
    }

    table posts {
        id: Int primary auto
        author_id: Int
        title: String

        foreign author_id -> users
    }
}
```

The compiler:
1. Type-checks queries against the declared schema
2. Generates parameterized SQL (no injection possible)
3. Maps result rows to Covenant types

---

## 9. WASM Compilation

### Target Runtime
- **Sandboxed execution** — memory-safe by default
- **Capability-constrained** — WASI for controlled host access
- **Metered execution** — fuel-based limits
- **Deterministic** — reproducible execution
- **Portable** — runs anywhere WASM runs

### Compilation Pipeline
```
Covenant Source (.cov)
       ↓
    Parser
       ↓
    AST (queryable, round-trippable, bidirectional refs)
       ↓
    Type Checker (capabilities, effects, contracts)
       ↓
    IR Generation
       ↓
    WASM Emitter
       ↓
    .wasm Module
```

---

## 10. What This Is and Isn't

**Is:**
- A general-purpose programming language
- A coordination language at heart
- A contract-first type system
- A machine-comprehensible project format
- A unified query interface for data and code

**Is Not:**
- A replacement for all languages
- A natural-language programming system
- An AI that writes perfect code

---

## 11. North Star

> **Humans write contracts.**
> **AI navigates structure.**
> **Tools do the work.**
> **Every line makes the next easier.**

---

## Related Documents

- [grammar.ebnf](grammar.ebnf) — Formal syntax definition
- [prior-art.md](prior-art.md) — Lessons from Austral and Koka
- [examples/](examples/) — Validated example programs
- [plans/bidirectional-refs-and-queries.md](plans/bidirectional-refs-and-queries.md) — Query system design
