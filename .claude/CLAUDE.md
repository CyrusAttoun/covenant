# Covenant

A programming language designed for AI-assisted development. Compiles to WASM.

---

## Documents

| Doc | Purpose |
|-----|---------|
| [DESIGN.md](../DESIGN.md) | Philosophy, four-layer model, core decisions |
| [grammar.ebnf](../grammar.ebnf) | Formal syntax definition (machine-parseable) |
| [examples/](../examples/) | Validated `.cov` source files |
| [prior-art.md](../prior-art.md) | Lessons from Austral and Koka |

---

## Quick Reference

### Key Decisions
- **Imports are effects** — function-level imports declare capabilities, no separate `requires` clause
- **No `fn` keyword** — functions defined by signature shape alone
- **`=` is equality** — not `==`; `:=` is assignment
- **Unified query syntax** — same SQL-like syntax for databases and source code (AST)
- **`none` for null** — `x = none` compiles to `IS NULL` in database queries
- **Union returns** — `-> User | DbError | NetworkError`, no `Result<T, E>` wrapper
- **Auto error propagation** — errors bubble up by default; use `handle` to catch
- **Postfix types** — `User?` for optional, `User[]` for list
- **Bidirectional refs** — compiler computes `called_by` for every function
- **WASM target** — Sandboxed, capability-constrained, metered

### Syntax at a Glance

```covenant
// Effectful function — import declares the capability
get_users() -> User[] | DbError
    import { app_db } from database
{
    query app_db {
        select * from users where is_active = true
    }
}

// Pure function — no imports, no effects
double(x: Int) -> Int {
    x * 2
}

// Types
struct User { id: Int, name: String }
enum Status { Active, Inactive, Pending(String) }

// Operators
let x = 5              // binding uses =
x := 10                // reassignment uses :=
if x = 10 { ... }      // equality uses =
if x != none { ... }   // inequality uses !=
```

### Unified Query Syntax

The same SQL-like syntax works for databases and source code. The import determines semantics.

```covenant
// DATABASE: Query external database (compiles to SQL)
get_active_users() -> User[] | DbError
    import { app_db } from database
{
    query app_db {
        select * from users where is_active = true
    }
}

// PROJECT: Query source code / AST (compiles to traversal)
find_db_functions() -> FunctionInfo[]
    import { project } from meta
{
    query project {
        select * from functions where effects contains "database"
    }
}
```

### CRUD Operations

```covenant
// DATABASE CRUD
insert into app_db.users { name: "Alice", email: "alice@example.com" }
update app_db.users set is_active: false where last_login < days_ago(30)
delete from app_db.users where id = user_id

// PROJECT (SOURCE CODE) CRUD
insert into project.functions { name: "new_fn", module: "auth", ... }
update project.functions set name: "new_name" where name = "old_name"
delete from project.functions where called_by = [] and is_exported = false
```

### Null Handling

```covenant
// In Covenant code
let user: User? = none           // optional with no value

// In database queries
where deleted_at = none          // → compiles to: WHERE deleted_at IS NULL
where deleted_at != none         // → compiles to: WHERE deleted_at IS NOT NULL

// Nullable columns become optional types
table users {
    deleted_at: DateTime nullable  // becomes DateTime? when queried
}
```

### Error Handling

```covenant
// Errors propagate automatically
get_user(id: Int) -> User | DbError | NetworkError {
    let data = fetch(id)     // NetworkError bubbles up
    parse(data)              // returns User
}

// Use handle to catch errors locally
get_user_safe(id: Int) -> User | DbError {
    let data = fetch(id) handle {
        NetworkError(e) => return DbError::from(e),
    }
    parse(data)
}
```

### Bidirectional References

The compiler computes and stores metadata on every symbol:

```
ast_metadata = {
    called_by: [symbol_id],      // who calls this function
    calls: [symbol_id],          // what this function calls
    references: [symbol_id],     // types/symbols referenced
    referenced_by: [symbol_id],  // what references this
    effects: [effect_id],        // computed effect set
}
```

Query it like any other data:
```covenant
// Find all callers of authenticate
query project {
    select * from functions where calls contains "authenticate"
}
```

### External Bindings

```covenant
// Wrap JS/npm libraries with typed effect declarations
extern get(url: String) -> Response | HttpError
    from "axios"
    effect [network]
```

---

## Status

**Design phase.** No compiler exists yet.

Current focus: finalize syntax, define AST, build parser.
