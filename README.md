# Covenant

A programming language designed for AI-assisted development. Compiles to WASM.

> **Natural language is for requesting.**
> **Source code is for meaning.**
> **AST is for tooling.**
> **Bytecode is for execution.**

---

## Why Covenant?

AI coding agents struggle with traditional codebases:
- They parse **text**, not meaning
- They **search and guess** instead of query
- Tool use is **probabilistic**, not contractual
- Each task starts from scratch—no compounding benefit

Covenant is a **coordination language** with a **contract-first type system**. The value is in:

1. **Tool contracts** — typed interfaces for external capabilities
2. **Queryable structure** — AST that tooling and AI can efficiently navigate
3. **Compounding clarity** — each feature written makes the next easier

---

## Documentation

| Doc | Purpose |
|-----|---------|
| [DESIGN.md](DESIGN.md) | Philosophy, four-layer model, core decisions |
| [grammar.ebnf](grammar.ebnf) | Formal syntax definition (machine-parseable) |
| [examples/](examples/) | Validated `.cov` source files |
| [prior-art.md](prior-art.md) | Lessons from Austral and Koka |

---

## Key Design Decisions

- **Imports are effects** — function-level imports declare capabilities
- **No `fn` keyword** — functions defined by signature shape alone
- **`=` is equality** — not `==`; `:=` is assignment
- **Unified query syntax** — same SQL-like syntax for databases and source code
- **`none` for null** — `x = none` compiles to `IS NULL` in queries
- **Union returns** — `-> User | DbError`, no `Result<T, E>` wrapper
- **Auto error propagation** — errors bubble up; use `handle` to catch
- **Postfix types** — `User?` for optional, `User[]` for list
- **Bidirectional refs** — compiler computes `called_by` for every function
- **WASM target** — Sandboxed, capability-constrained, metered

---

## Syntax at a Glance

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
```

### Unified Query Syntax

The same syntax works for databases and source code:

```covenant
// DATABASE: compiles to SQL
query app_db {
    select * from users where is_active = true
}

// PROJECT: compiles to AST traversal
query project {
    select * from functions where effects contains "database"
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

---

## Status

**Design phase.** No compiler exists yet.

Current focus: finalize syntax, define AST, build parser.

---

## License

MIT
