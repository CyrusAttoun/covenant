# Covenant

**A programming language where code is queryable data.**

Covenant is designed for a future where AI agents and humans collaborate on codebases. Traditional languages optimize for human authorship—Covenant optimizes for machine comprehension without sacrificing readability.

---

## The Problem

AI coding agents struggle with existing codebases:

- They parse **text**, not semantic structure
- They must **search and guess** instead of query
- Tool use is **probabilistic**, not contractual
- Each task starts from scratch—benefits don't compound
- Reliability degrades as codebases grow larger

We've been writing code for humans to read and machines to execute. But now machines need to *understand* code too—and grep isn't good enough.

---

## The Vision

> *Humans write contracts. AI navigates structure. Tools do the work. Every line makes the next easier.*

Covenant separates concerns across four layers:

| Layer | Purpose | Lifetime |
|-------|---------|----------|
| **Natural Language** | Human↔AI communication | Ephemeral (discarded after translation) |
| **Source Code** | Contracts, intent, constraints | Permanent artifact |
| **AST** | Queryable symbol graph | What AI agents operate against |
| **Bytecode** | WASM execution | Sandboxed, metered, deterministic |

The key insight: **source code should be as queryable as a database**. Not through text search, but through structured queries over a semantic graph.

---

## Core Principles

### Code is Data
Every symbol has computed metadata—who calls it, what it calls, what effects it has. Query your codebase like you query a database:

```covenant
find_db_functions() -> FunctionInfo[]
    import { project } from meta
{
    query project {
        select * from functions
        where effects contains "database"
    }
}
```

### Contracts, Not Comments
Types encode what a function *can* do, not just what it returns. Effects are declared as imports—capabilities, not annotations:

```covenant
get_user(id: Int) -> User | DbError
    import { app_db } from database
{
    query app_db {
        select * from users where id = id limit 1
    }
}
```

The function signature tells you everything: it takes an `Int`, returns a `User` or a `DbError`, and requires database access to `app_db`. No hidden side effects.

### Bidirectional References
The compiler computes `called_by` for every function. Find all callers without grep:

```covenant
find_auth_callers() -> FunctionInfo[]
    import { project } from meta
{
    query project {
        select * from functions
        where calls contains "authenticate"
    }
}
```

### Compounding Clarity
Each function you write makes the next one easier. Typed contracts, queryable structure, and computed metadata mean AI agents can navigate with precision instead of probability.

---

## What Makes It Different

| Traditional Languages | Covenant |
|-----------------------|----------|
| Search code with grep/regex | Query code with SQL-like syntax |
| Effects are implicit | Effects declared as imports |
| "Who calls this?" requires tooling | `called_by` computed automatically |
| Comments describe intent | Types encode intent |
| Each file is isolated text | Codebase is a queryable graph |

**Unified query syntax**: The same SQL-like syntax works for databases *and* source code. The import determines semantics:

```covenant
// Query external database (compiles to SQL)
query app_db { select * from users where is_active = true }

// Query source code (compiles to AST traversal)
query project { select * from functions where is_exported = false and called_by = [] }
```

**WASM target**: Compiles to WebAssembly for sandboxed, capability-constrained, metered execution.

---

## Documentation

### Quick Start by Goal

**"I want to learn Covenant"**
1. [Tutorial](docs/guide/tutorial.md) - Hello World and basics
2. [Reading Guide](docs/guide/reading-guide.md) - How to read Covenant code
3. [Syntax Examples](docs/guide/syntax-examples.md) - Cheat sheet
4. [Patterns](docs/guide/patterns.md) - Common idioms

**"I want to understand the language design"**
1. [Design](docs/design/DESIGN.md) - Philosophy and four-layer model
2. [Query Semantics](docs/design/QUERY_SEMANTICS.md) - Query system spec
3. [Compiler](docs/design/COMPILER.md) - Compilation phases
4. [Grammar](docs/design/grammar.ebnf) - Formal syntax

**"I want to build LLM integrations"**
1. [LLM Code Generation](docs/specs/llm-code-generation.md) - Generation system
2. [Explain Generator](docs/specs/explain-generator.md) - Explanation algorithm
3. [Comment Generator](docs/specs/comment-generator.md) - Auto-documentation

### Document Index

| Directory | Purpose |
|-----------|---------|
| [docs/guide/](docs/guide/) | Learning materials for language users |
| [docs/design/](docs/design/) | Language design and compiler specifications |
| [docs/specs/](docs/specs/) | LLM and tooling integration specifications |
| [examples/](examples/) | Example `.cov` source files |

---

## Status

**Design phase.** No compiler exists yet.

Current focus: finalize syntax, define AST, build parser.

---

## License

MIT
