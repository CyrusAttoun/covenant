# Covenant

**A language designed for AI to generate and navigate.**

Covenant is not a language for humans to write by hand. It's an intermediate representation (IR) optimized for machine generation—deterministic structure, queryable symbols, explicit effects. Humans work in natural language; the AI generates and navigates the IR.

---

## Installation

**macOS / Linux:**

```sh
curl -fsSL https://raw.githubusercontent.com/Cyronius/covenant/master/install/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/Cyronius/covenant/master/install/install.ps1 | iex
```

**Auto-install shim** — drop into your project or CI for on-demand installation:

```sh
# Unix
./install/shim.sh run myfile.cov

# Windows
.\install\shim.bat run myfile.cov
```

---

## Building from Source

### Prerequisites

**Required:**
- [Rust](https://rustup.rs/) 1.70+ (stable toolchain)
- `cargo` (comes with Rust)

**Optional (for running Covenant programs):**
- [Deno](https://deno.land/) 1.40+ (default runtime, recommended)
- [Node.js](https://nodejs.org/) 18+ (fallback runtime)

### Build Instructions

Clone the repository and build the compiler:

```sh
git clone https://github.com/Cyronius/covenant.git
cd covenant
cargo build --release -p covenant-cli
```

The compiled binary will be at `target/release/covenant`.

**Note:** You must build the `covenant-cli` package specifically to create the CLI binary.

### Install Locally

To install the `covenant` command to your system:

```sh
cargo install --path crates/covenant-cli
```

This installs the binary to `~/.cargo/bin/covenant` (ensure `~/.cargo/bin` is in your PATH).

### Running Tests

Run the full test suite:

```sh
cargo test --workspace
```

Run specific test suites:

```sh
# Parser tests
cargo test -p covenant-parser

# Type checker tests
cargo test -p covenant-checker

# Integration tests
cargo test --test '*'
```

### Development Workflow

Compile and run a Covenant program in one step:

```sh
# Using the binary directly
./target/release/covenant run examples/hello-world/hello-world.cov

# Or if installed
covenant run examples/hello-world/hello-world.cov
```

Other useful commands:

```sh
# Parse and check for errors
covenant check examples/hello-world/hello-world.cov

# Check with detailed diagnostics and fix suggestions
covenant check --explain examples/hello-world/hello-world.cov

# Format to canonical form
covenant format examples/hello-world/hello-world.cov

# Check if file is already canonical (exit 1 if not)
covenant format --check examples/hello-world/hello-world.cov

# Show symbol information
covenant info examples/hello-world/hello-world.cov

# Query the codebase
covenant query --query "select all from functions" examples/hello-world/hello-world.cov

# Generate explanations
covenant explain examples/hello-world/hello-world.cov

# Interactive REPL
covenant repl
```

### Runtime Dependencies

Covenant compiles to WebAssembly (WASM). To execute programs, you need a runtime:

- **Deno** (recommended): Provides WASI support and I/O APIs
- **Node.js**: Fallback option with similar capabilities
- **Browser**: For web-based execution (requires custom loader)

The `covenant run` command automatically uses Deno if available, falling back to Node.js.

Install Deno (recommended):

```sh
# macOS/Linux
curl -fsSL https://deno.land/install.sh | sh

# Windows (PowerShell)
irm https://deno.land/install.ps1 | iex
```

---

## Why AI-First?

Traditional languages optimize for human authorship. Covenant optimizes for machine generation:

| Design Choice | Why It Helps AI |
|---------------|-----------------|
| **No operators** | Keywords only (`add`, `equals`, `and`)—no symbol ambiguity |
| **SSA form** | One operation per step, no nesting to parse |
| **Canonical ordering** | One valid way to write everything—deterministic output |
| **Small grammar** | ~50 keywords, predictable token sequences |
| **Every node has ID** | Precise queries and references, no guessing |
| **Parameterized effects** | `effect filesystem(path="/data")` for capability narrowing |
| **Runtime enforcement** | WASM imports gated by declared effects |

The result: AI can generate valid code reliably and navigate codebases through structured queries instead of text search.

---

## Effects Are Explicit

Every snippet declares what capabilities it needs. No effects section means pure.

Effects can be parameterized for capability narrowing:

```covenant
snippet id="user.get_by_id" kind="fn"

effects
  effect database
  effect filesystem(path="/data")   (* parameterized effect *)
end

signature
  fn name="get_by_id"
    param name="id" type="Int"
    returns type="User" optional
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

end
```

Effects propagate transitively—if A calls B, A inherits B's effects. The compiler enforces this.

**Why this matters for AI:** Explicit contracts let AI close the loop. The signature defines what to test, the effects define what to mock, and requirements link to coverage. AI can generate code, generate tests, run them, and iterate—without human intervention.

---

## The Query System

Covenant treats codebases as queryable databases—the foundation for retrieval-augmented generation (RAG). Documents and code can be ingested, indexed, and searched at runtime through compiled WASM modules. See the [query system example](examples/query-system/README.md) for a complete pipeline from document ingestion to interactive querying.

Query your codebase like a database. Find all functions that use the database:

```covenant
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

The same query syntax works for external databases (with dialect-specific SQL in `body` blocks) and for the codebase itself. The `target` determines semantics.

The compiler computes bidirectional references for every symbol:
- `called_by` / `calls`
- `references` / `referenced_by`
- `effects` (full transitive closure)
- `tests` / `requirements`

No grep. No guessing. Structured queries over a semantic graph.

---

## The Four-Layer Model

| Layer | Purpose | Lifetime |
|-------|---------|----------|
| **Natural Language** | Human↔AI communication | Ephemeral |
| **IR (Source)** | Machine-readable contracts | Permanent artifact |
| **Symbol Graph** | Queryable, bidirectional refs | Derived by compiler |
| **Bytecode** | WASM execution | Sandboxed, metered |

---

## What Makes It Different

| Traditional Languages | Covenant |
|-----------------------|----------|
| `x + y * z` | `op=add`, `op=mul` (keywords, SSA) |
| Effects implicit | `effects` block declares capabilities |
| grep for callers | `called_by` computed automatically |
| Comments describe intent | Types and effects encode intent |
| Files are text | Codebase is a queryable graph |

### Runtime Targets

Covenant compiles to WASM and runs on multiple platforms:

| Target | Runtime | Command |
|--------|---------|---------|
| **Deno** (default) | `run.deno.ts` — loads WASM, provides I/O | `covenant run <file>` |
| **Node.js** (fallback) | `run.mjs` — same interface, Node APIs | `covenant run <file>` |
| **Browser** | Host loader — fetch WASM, link modules | Import via `loader.ts` |
| **WASI** | WASI 0.2 Components | `--target=wasi` (planned) |

`covenant run` compiles and executes in one step, using Deno by default with Node.js as fallback.

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

**Active development.** The compiler pipeline is implemented and functional.

### What Works
- **Full compiler pipeline**: lex → parse → symbol graph → type check → codegen → WASM
- **CLI tool** (`covenant`): `parse`, `check`, `compile`, `query`, `info`, `explain`, `effects`, `requirements`, `repl`, `run`, `format`
- **Parameterized effects**: `effect filesystem(path="/data")` with subsumption rules
- **Runtime effect enforcement**: WASM imports gated by declared effects
- **Canonical text printer**: AST → `.cov` serialization via `format` command
- **Enhanced diagnostics**: Fix suggestions and effect chain explanations with `--explain`
- **23 example programs** covering all major features
- **Integration tests** passing (parsing, symbol graphs, type checking, effect validation, WASM codegen)

### Architecture (13 crates)

| Crate | Role |
|-------|------|
| `covenant-lexer` | Tokenization |
| `covenant-parser` | Recursive descent parser with error recovery |
| `covenant-ast` | AST definitions |
| `covenant-symbols` | Symbol graph with bidirectional references |
| `covenant-checker` | Type checker and effect validator |
| `covenant-graph` | Query engine |
| `covenant-codegen` | WASM code generation |
| `covenant-runtime` | Runtime query and mutation engine |
| `covenant-storage` | Symbol store with versioning |
| `covenant-optimizer` | Optimization passes |
| `covenant-requirements` | Requirement coverage validation |
| `covenant-llm` | AI explanation and code generation |
| `covenant-cli` | Command-line interface |

### Recent Additions

**Parameterized Effects** — Effects can take parameters for capability narrowing:
```covenant
effects
  effect filesystem(path="/data")     (* restrict to /data directory *)
  effect database(readonly=true)      (* read-only database access *)
end
```

**Runtime Effect Enforcement** — WASM imports are gated at module instantiation:
- Compiler embeds `required_capabilities` in WASM data section
- Host extracts `CapabilityManifest` and filters imports
- Strict mode (default) throws errors for undeclared capabilities

**Canonical Text Printer** — Round-trip AST to `.cov` text:
```sh
covenant format file.cov           # Print canonical form
covenant format --check file.cov   # Verify canonical (exit 1 if not)
```

**Enhanced Diagnostics** — Rich error context with fix suggestions:
```sh
covenant check --explain file.cov
# Shows: call chains, effect propagation, suggested fixes
```

### Current Focus
- Structured concurrency (built-in `parallel` / `race` step kinds)
- Cross-platform storage (`std.storage`)
- Cross-snippet type checking

---

## License

MIT
