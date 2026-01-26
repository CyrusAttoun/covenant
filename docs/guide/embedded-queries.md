# Embedded Queries in Covenant

This guide explains how to compile Covenant programs with embedded data and query them at runtime using in-WASM functions.

## Overview

Covenant supports two types of embedded queries:

1. **Data Queries** - Query embedded documentation and knowledge graphs (`kind="data"` snippets)
2. **Symbol Queries** - Query the code's own metadata (functions, types, effects, requirements)

Both use the `target="project"` query syntax, but operate on different embedded graphs:
- Data queries access the **data graph** (content, relations, metadata)
- Symbol queries access the **symbol graph** (AST metadata, call graph, effect closure)

## Quick Start

### 1. Define Data Snippets

Create documentation or knowledge base entries using `kind="data"`:

```covenant
snippet id="docs.hello" kind="data"
  note "Hello World Documentation"

  content
    """
    Hello World Documentation
    This is a simple example.
    """
  end

  relations
    rel to="main.hello" type=describes
  end
end
```

### 2. Query Embedded Data

Use `target="project"` queries to search the embedded graph:

```covenant
snippet id="query.find_docs" kind="fn"
  effects
    effect meta
  end

  signature
    fn name="find_docs"
      returns type="Any"
    end
  end

  body
    step id="s1" kind="query"
      target="project"
      select all
      from="snippets"
      where
        equals field="kind" lit="data"
      end
      as="docs"
    end

    step id="s2" kind="return"
      from="docs"
      as="_"
    end
  end
end
```

### 3. Compile to WASM

Compile your program with data embedding:

```bash
cargo run -p covenant-cli -- compile examples/my-program.cov \
  --output examples/my-program.wasm
```

Data snippets are automatically embedded in the WASM data segment.

### 4. Load and Query

Use the `CovenantQueryRunner` to load and query the WASM module:

```typescript
import { CovenantQueryRunner } from "../runtime/host/query-runner.ts";

const runner = new CovenantQueryRunner();
await runner.load("./examples/my-program.wasm");

// Access embedded data via GAI functions
const nodeCount = runner.nodeCount();
const nodeIds = runner.getAllNodeIds();
const nodes = runner.getAllNodes();

// Call exported query functions
const docs = runner.call("find_docs");
```

## Graph Access Interface (GAI)

When you compile Covenant programs with `kind="data"` snippets, the compiler generates **Graph Access Interface (GAI)** functions embedded in the WASM module.

### GAI Functions

These functions are automatically exported and provide low-level access to the embedded graph:

| Function | Signature | Description |
|----------|-----------|-------------|
| `cov_node_count()` | `() -> i32` | Get total number of nodes |
| `cov_get_node_id(idx)` | `(i32) -> i64` | Get node ID as fat pointer |
| `cov_get_node_content(idx)` | `(i32) -> i64` | Get node content as fat pointer |
| `cov_get_outgoing_count(idx)` | `(i32) -> i32` | Get count of outgoing relations |
| `cov_get_outgoing_rel(idx, i)` | `(i32, i32) -> i64` | Get outgoing relation (packed) |
| `cov_get_incoming_count(idx)` | `(i32) -> i32` | Get count of incoming relations |
| `cov_get_incoming_rel(idx, i)` | `(i32, i32) -> i64` | Get incoming relation (packed) |
| `cov_find_by_id(ptr, len)` | `(i32, i32) -> i32` | Find node index by ID |
| `cov_content_contains(idx, ptr, len)` | `(i32, i32, i32) -> i32` | Check if content contains substring |
| `cov_get_rel_type_name(type_idx)` | `(i32) -> i64` | Get relation type name |

### Fat Pointers

String values are returned as **fat pointers** (i64) with this format:
```
i64 = (ptr << 32) | len
```

Where:
- Upper 32 bits: Memory pointer
- Lower 32 bits: String length

To extract a string in TypeScript/JavaScript:

```typescript
const fatPtr = runner.getNodeId(0);
const len = Number(fatPtr & 0xFFFFFFFFn);
const ptr = Number((fatPtr >> 32n) & 0xFFFFFFFFn);
const bytes = new Uint8Array(memory.buffer, ptr, len);
const str = new TextDecoder().decode(bytes);
```

## Data Segment Layout

The compiler embeds graph data in the WASM data segment with this layout:

```
┌────────────────────────────┐
│ String Pool                │ ← All IDs, content, metadata as bytes
├────────────────────────────┤
│ Node ID Table              │ ← Fat pointers into string pool
├────────────────────────────┤
│ Content Table              │ ← Fat pointers to content strings
├────────────────────────────┤
│ Notes Table                │ ← Fat pointers to note arrays
├────────────────────────────┤
│ Outgoing Relations         │ ← Sorted by from_idx
├────────────────────────────┤
│ Incoming Relations         │ ← Sorted by to_idx
├────────────────────────────┤
│ Adjacency Index            │ ← Fast relation lookups
├────────────────────────────┤
│ Relation Types             │ ← Type name strings
├────────────────────────────┤
│ Metadata                   │ ← Key-value pairs
└────────────────────────────┘
```

This layout enables efficient querying without external databases.

## Query Syntax

### Supported Queries

```covenant
(* Find all data snippets *)
step id="s1" kind="query"
  target="project"
  select all
  from="snippets"
  where
    equals field="kind" lit="data"
  end
  as="docs"
end

(* Find by ID *)
step id="s2" kind="query"
  target="project"
  select all
  from="snippets"
  where
    equals field="id" lit="docs.hello"
  end
  limit=1
  as="doc"
end

(* Content search *)
step id="s3" kind="query"
  target="project"
  select all
  from="snippets"
  where
    contains field="content" lit="design"
  end
  as="results"
end
```

### Current Implementation Status

**✅ Implemented:**
- Data snippet parsing and compilation
- GAI function generation
- Data graph embedding in WASM
- Query syntax parsing and type checking
- Query routing (`target="project"` vs external DB)
- Runtime host (CovenantQueryRunner)
- Working examples and tests

**⚠️ Stub Implementation:**
- Query execution (`compile_project_query` returns empty results)
- Queries compile without errors but don't execute yet

**🔄 Pending:**
- Full `compile_project_query` implementation
- Mapping Covenant query syntax to GAI function calls
- Result collection and serialization
- `traverse` step for relation traversal
- Relation queries (`rel_to`, `rel_from`)

## Examples

### Example 50: Simple Embedded Query

Basic example showing data snippets and simple queries.

```bash
./scripts/build-query-examples.sh
deno run --allow-read examples/50-test.ts
```

**Files:**
- [examples/50-embedded-query-simple.cov](../../examples/50-embedded-query-simple.cov) - Source
- [examples/50-test.ts](../../examples/50-test.ts) - Test script

### Example 20: Knowledge Base Traversal

Knowledge graph with hierarchical structure and relations.

```bash
deno run --allow-read examples/20-test.ts
```

**Files:**
- [examples/20-knowledge-base.cov](../../examples/20-knowledge-base.cov) - Source
- [examples/20-test.ts](../../examples/20-test.ts) - Test script

### Example 14: Project Queries (Symbol Graph)

Demonstrates symbol graph queries (requires symbol embedding - future work).

```bash
deno run --allow-read examples/14-test.ts
```

**Files:**
- [examples/14-project-queries.cov](../../examples/14-project-queries.cov) - Source
- [examples/14-test.ts](../../examples/14-test.ts) - Test script

## Compilation Flags

Current compilation automatically embeds:
- **Data graph**: All `kind="data"` snippets

Future compilation flags (planned):
- `--embed-symbols=none|api|reachable|full` - Control symbol graph embedding
- `--embed-data=none|all` - Control data graph embedding

## Runtime API

### CovenantQueryRunner

The `CovenantQueryRunner` class provides a simple interface for loading and querying WASM modules.

```typescript
class CovenantQueryRunner {
  // Load compiled WASM module
  async load(wasmPath: string): Promise<void>

  // Call exported function
  call(functionName: string, ...args: unknown[]): unknown

  // GAI functions
  nodeCount(): number
  getNodeId(idx: number): bigint
  getNodeContent(idx: number): bigint
  findByIdRaw(id: string): number
  readString(fatPtr: bigint): string

  // Convenience methods
  getAllNodeIds(): string[]
  getAllNodes(): Array<{ id: string; content: string }>
  listExports(): string[]
}
```

### Usage Example

```typescript
import { CovenantQueryRunner } from "../runtime/host/query-runner.ts";

const runner = new CovenantQueryRunner();
await runner.load("./program.wasm");

// Access embedded data
console.log(`Total nodes: ${runner.nodeCount()}`);

// List all node IDs
const ids = runner.getAllNodeIds();
ids.forEach(id => console.log(`- ${id}`));

// Get nodes with content
const nodes = runner.getAllNodes();
nodes.forEach(node => {
  console.log(`${node.id}: ${node.content.substring(0, 50)}...`);
});

// Call query functions
const docs = runner.call("find_docs");
```

## Architecture

### Two Query Paths

1. **Project Queries** (`target="project"`)
   - Execute against embedded graph data
   - Use GAI functions (in-WASM execution)
   - No external database needed
   - Fast, deterministic, portable

2. **Database Queries** (`target="db_name"`)
   - Execute against external databases
   - Call host imports (external execution)
   - Require database connection
   - SQL or Covenant dialect

### Query Routing

The compiler routes queries based on the `target` attribute:

```rust
fn compile_query_step(&mut self, query: &QueryStep, func: &mut Function) {
    if query.target == "project" {
        // Route to GAI functions (embedded execution)
        self.compile_project_query(query, func)
    } else {
        // Route to external database (host imports)
        self.compile_database_query(query, func)
    }
}
```

## Future Work

### Full Query Execution

Implement `compile_project_query` to generate WASM code that:

1. Calls `cov_node_count()` to get total nodes
2. Iterates through nodes using `cov_get_node_id(idx)`
3. Filters nodes based on `where` clause:
   - `contains field="content"` → `cov_content_contains()`
   - `equals field="id"` → `cov_find_by_id()`
   - Other predicates → load and compare in WASM
4. Collects matching nodes into result array
5. Applies `order` and `limit` clauses
6. Returns fat pointer to result array

### Traverse Step

Support graph traversal:

```covenant
step id="s1" kind="traverse"
  target="project"
  from="kb.design.philosophy"
  follow type=contained_by
  depth=unbounded
  direction=outgoing
  as="ancestors"
end
```

### Relation Queries

Support relation-based filtering:

```covenant
where
  and
    equals field="kind" lit="data"
    rel_to target="auth.login" type=describes
  end
end
```

### Symbol Graph Embedding

Embed symbol metadata alongside data graph:
- Function signatures and call graph
- Type definitions and dependencies
- Effect declarations and closures
- Requirements and test coverage

## See Also

- [QUERY_SEMANTICS.md](../design/QUERY_SEMANTICS.md) - Full query specification
- [runtime-query-system.md](../../.claude/implemented_plans/runtime-query-system.md) - WIT-based query architecture
- [gai_codegen.rs](../../crates/covenant-codegen/src/gai_codegen.rs) - GAI implementation
- [data_graph.rs](../../crates/covenant-codegen/src/data_graph.rs) - Data graph construction
