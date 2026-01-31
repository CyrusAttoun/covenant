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
| `cov_get_node_kind(idx)` | `(i32) -> i64` | Get node kind as fat pointer |
| `cov_get_node_content(idx)` | `(i32) -> i64` | Get node content as fat pointer |
| `cov_get_outgoing_count(idx)` | `(i32) -> i32` | Get count of outgoing relations |
| `cov_get_outgoing_rel(idx, i)` | `(i32, i32) -> i64` | Get outgoing relation (packed) |
| `cov_get_incoming_count(idx)` | `(i32) -> i32` | Get count of incoming relations |
| `cov_get_incoming_rel(idx, i)` | `(i32, i32) -> i64` | Get incoming relation (packed) |
| `cov_find_by_id(ptr, len)` | `(i32, i32) -> i32` | Find node index by ID |
| `cov_content_contains(idx, ptr, len)` | `(i32, i32, i32) -> i32` | Check if content contains substring |
| `cov_get_rel_type_name(type_idx)` | `(i32) -> i64` | Get relation type name |
| `_cov_get_symbol_metadata()` | `() -> i64` | Get embedded symbol metadata JSON |

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

(* ORDER BY - sort by ID ascending *)
step id="s4" kind="query"
  target="project"
  select all
  from="snippets"
  where
    equals field="kind" lit="data"
  end
  order by="id" dir="asc"
  as="sorted_docs"
end

(* ORDER BY - sort by ID descending *)
step id="s5" kind="query"
  target="project"
  select all
  from="snippets"
  where
    equals field="kind" lit="data"
  end
  order by="id" dir="desc"
  as="reverse_sorted"
end
```

### ORDER BY

Sort query results by a field value:

```covenant
order by="id" dir="asc"    (* Ascending - alphabetical order *)
order by="id" dir="desc"   (* Descending - reverse alphabetical *)
```

**Supported fields for ORDER BY:**
- `id` - Sort by node ID (✅ works correctly)
- `kind` - Sort by node kind (⚠️ known bug - produces inverted results)
- `content` - Sort by content string

**Algorithm:** Uses insertion sort in WASM. Efficient for typical embedded data sizes (<1000 nodes).

### Relation Traversal

Navigate graph relationships using the `traverse` step:

```covenant
(* Find nodes that the current node points to *)
step id="s1" kind="traverse"
  target="project"
  from var="start_node"
  follow rel="describes"
  direction=outgoing
  as="related_docs"
end

(* Find nodes that point to the current node *)
step id="s2" kind="traverse"
  target="project"
  from lit="main.hello"
  follow rel="describes"
  direction=incoming
  as="documentation"
end
```

**Traverse options:**
- `direction=outgoing` - Follow relations where current node is the source
- `direction=incoming` - Follow relations where current node is the target
- `direction=both` - Follow relations in either direction

**Depth:** Currently supports single-hop traversal only. Multi-hop (`depth > 1`) is planned.

### Current Implementation Status

**✅ Fully Implemented:**
- Data snippet parsing and compilation
- GAI function generation (12 functions)
- Data graph embedding in WASM
- Query syntax parsing and type checking
- Query routing (`target="project"` vs external DB)
- Runtime host (CovenantQueryRunner)
- Working examples and tests
- **Query execution** - `compile_project_query` generates working WASM code
- **WHERE clause** - `equals`, `contains`, `and`, `or`, `not` conditions
- **LIMIT clause** - Result count limiting
- **ORDER BY clause** - Sorting by `id` field (ascending/descending)
- **Symbol metadata embedding** - Functions, effects, requirements, tests embedded as JSON
- **Relation traversal** - `traverse` step for graph navigation (single-hop)

**⚠️ Known Limitations:**
- ORDER BY `kind` field produces inverted results (bug under investigation)
- Multi-hop traversal (`depth > 1`) not yet implemented
- `rel_to`/`rel_from` conditions are stubs

**🔄 Pending:**
- ORDER BY `kind` field fix
- Multi-hop relation traversal
- Indexing for O(1) lookups (currently O(n) scan)
- JOIN support in WASM codegen

## Examples

### Example 50: Comprehensive Query Tests

Tests data queries with WHERE, ORDER BY, and LIMIT clauses.

```bash
cargo run -p covenant-cli -- compile examples/50-embedded-query-simple.cov \
  --output examples/50-embedded-query-simple.wasm
deno run --allow-read examples/50-test-comprehensive.ts
```

**Files:**
- [examples/50-embedded-query-simple.cov](../../examples/50-embedded-query-simple.cov) - Source with query functions
- [examples/50-test-comprehensive.ts](../../examples/50-test-comprehensive.ts) - Comprehensive test suite (20 tests)

**Test coverage:**
- `find_docs` - Find all data nodes
- `find_hello_doc` - Find specific node with AND + LIMIT
- `find_docs_sorted_asc` - ORDER BY id ascending
- `find_docs_sorted_desc` - ORDER BY id descending
- `find_all_sorted_by_kind` - ORDER BY kind (known bug)
- `find_all_unsorted` - All nodes without sorting

### Example 51: Symbol Metadata

Tests embedded symbol metadata including effects, requirements, and tests.

```bash
cargo run -p covenant-cli -- compile examples/51-symbol-metadata-test.cov \
  --output examples/51-symbol-metadata-test.wasm
deno run --allow-read examples/51-test.ts
```

**Files:**
- [examples/51-symbol-metadata-test.cov](../../examples/51-symbol-metadata-test.cov) - Source with requirements and tests
- [examples/51-test.ts](../../examples/51-test.ts) - Symbol metadata assertions

### Example 52: Relation Traversal

Tests graph traversal with outgoing, incoming, and chained queries.

```bash
cargo run -p covenant-cli -- compile examples/52-relation-traversal.cov \
  --output examples/52-relation-traversal.wasm
deno run --allow-read examples/52-test.ts
```

**Files:**
- [examples/52-relation-traversal.cov](../../examples/52-relation-traversal.cov) - Source with hierarchical relations
- [examples/52-test.ts](../../examples/52-test.ts) - Traversal test suite (11 tests)

**Test coverage:**
- `get_children` - Outgoing relations (contains)
- `get_parent` - Incoming relations (contained_by)
- `get_grandchildren` - Multi-step traversal
- `chain_traverse` - Traverse from query result

### Example 20: Knowledge Base Traversal

Knowledge graph with hierarchical structure and relations.

```bash
deno run --allow-read examples/20-test.ts
```

**Files:**
- [examples/20-knowledge-base.cov](../../examples/20-knowledge-base.cov) - Source
- [examples/20-test.ts](../../examples/20-test.ts) - Test script

### Example 14: Project Queries (Symbol Graph)

Demonstrates symbol graph queries.

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

## Working with Query Results

Query functions return **fat pointers to node index arrays**, not the nodes themselves. This section explains how to unpack and use query results.

### Query Return Format

When you call a query function like `find_docs()`, it returns a 64-bit fat pointer:

```
i64 = (ptr << 32) | count
```

- **Upper 32 bits**: Memory pointer to an array of u32 node indices
- **Lower 32 bits**: Number of matching nodes (count)

### Complete Example: Unpacking Query Results

```typescript
import { CovenantQueryRunner } from "../runtime/host/query-runner.ts";

const runner = new CovenantQueryRunner();
await runner.load("./examples/50-embedded-query-simple.wasm");

// Call a query function
const result = runner.call("find_docs") as bigint;

// Unpack fat pointer
const ptr = Number(result >> 32n);
const count = Number(result & 0xFFFFFFFFn);

console.log(`Query returned ${count} nodes`);

// Read node indices from WASM memory
const view = new DataView(runner.memory!.buffer, ptr, count * 4);
const indices: number[] = [];
for (let i = 0; i < count; i++) {
  indices.push(view.getUint32(i * 4, true)); // little-endian
}

// Get node data using GAI functions
for (const idx of indices) {
  const idPtr = runner.call("cov_get_node_id", idx) as bigint;
  const contentPtr = runner.call("cov_get_node_content", idx) as bigint;

  const id = runner.readString(idPtr);
  const content = runner.readString(contentPtr);
  console.log(`${id}: ${content.substring(0, 50)}...`);
}
```

### Helper Pattern: Extracting Results as Objects

```typescript
function extractQueryResults(runner: CovenantQueryRunner, fatPtr: bigint) {
  const ptr = Number(fatPtr >> 32n);
  const count = Number(fatPtr & 0xFFFFFFFFn);

  if (count === 0) return [];

  const view = new DataView(runner.memory!.buffer, ptr, count * 4);
  const results = [];

  for (let i = 0; i < count; i++) {
    const idx = view.getUint32(i * 4, true);
    const id = runner.readString(runner.call("cov_get_node_id", idx) as bigint);
    const content = runner.readString(runner.call("cov_get_node_content", idx) as bigint);
    const kind = runner.readString(runner.call("cov_get_node_kind", idx) as bigint);
    results.push({ idx, id, content, kind });
  }

  return results;
}

// Usage
const docs = extractQueryResults(runner, runner.call("find_docs") as bigint);
docs.forEach(doc => console.log(doc.id));
```

### Alternative: Direct GAI Function Filtering

For advanced use cases or fine-grained control, you can use GAI functions directly for runtime filtering:

```typescript
// Search for content at runtime using GAI functions directly
function searchContent(runner: CovenantQueryRunner, searchTerm: string) {
  const results = [];
  const count = runner.nodeCount();

  for (let i = 0; i < count; i++) {
    const content = runner.readString(
      runner.call("cov_get_node_content", i) as bigint
    );
    if (content.toLowerCase().includes(searchTerm.toLowerCase())) {
      const id = runner.readString(
        runner.call("cov_get_node_id", i) as bigint
      );
      results.push({ idx: i, id, content });
    }
  }

  return results;
}

// Search with custom logic (e.g., case-insensitive)
const matches = searchContent(runner, "user input here");
```

This approach gives you full control over the search logic but doesn't leverage the compiled query optimizations.

## Parameterized Queries (Runtime String Parameters)

Starting with the `cov_alloc` export, you can now pass runtime string parameters to query functions. This enables using Covenant as a queryable document database.

### Writing Parameterized Query Functions

Define query functions with string parameters:

```covenant
snippet id="query.search_content" kind="fn"
  effects
    effect meta
  end

  signature
    fn name="search_content"
      param name="term" type="String"
      returns type="Any"
    end
  end

  body
    step id="s1" kind="query"
      target="project"
      select all
      from="snippets"
      where
        contains field="content" var="term"
      end
      as="results"
    end

    step id="s2" kind="return"
      from="results"
      as="_"
    end
  end
end
```

The key differences from literal queries:
- The function has a `param name="term" type="String"` in its signature
- The WHERE clause uses `var="term"` instead of `lit="some value"`

### Calling Parameterized Queries

Use the runtime API to allocate strings and call functions:

```typescript
const runner = new CovenantQueryRunner();
await runner.load("./my-docs.wasm");

// Method 1: Using queryWithString convenience method
const results = runner.queryWithString("search_content", "user input");
const nodes = runner.getQueryResultNodes(results);
console.log(`Found ${nodes.length} matching documents`);

// Method 2: Manual allocation
const fatPtr = runner.allocString("user input");
const results2 = runner.call("search_content", fatPtr) as bigint;
```

### How It Works

1. **Host allocates memory**: Call `cov_alloc(size)` to get a pointer
2. **Host writes string**: Write UTF-8 bytes to WASM memory
3. **Host packs fat pointer**: `(ptr << 32) | len` as BigInt
4. **Host calls function**: Pass the fat pointer as the string parameter
5. **WASM unpacks and uses**: Function unpacks the fat pointer and uses it in WHERE clauses

### Supported Operations with Variables

| Operation | Example |
|-----------|---------|
| Exact match | `equals field="id" var="param_name"` |
| Content search | `contains field="content" var="param_name"` |
| Kind filter | `equals field="kind" var="param_name"` |

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

## Symbol Metadata

Covenant automatically embeds symbol metadata in the WASM data segment, accessible via `_cov_get_symbol_metadata()`.

### Embedded Metadata

Each symbol includes:
- `id` - Fully qualified symbol ID (e.g., `"main.hello"`)
- `kind` - Symbol kind (`"function"`, `"data"`, etc.)
- `line` - Source line number
- `calls` - Functions this symbol calls
- `called_by` - Functions that call this symbol
- `references` - Types/symbols referenced
- `referenced_by` - Symbols that reference this
- `effects` - Declared effects
- `effect_closure` - Transitive effect closure
- `requirements` - Linked requirement IDs
- `tests` - Test IDs that cover this symbol
- `covers` - Requirement IDs covered (for tests)

### Accessing Symbol Metadata

```typescript
const runner = new CovenantQueryRunner();
await runner.load("./program.wasm");

// Get symbol metadata JSON
const metadataPtr = runner.call("_cov_get_symbol_metadata") as bigint;
const json = runner.readString(metadataPtr);
const symbols = JSON.parse(json);

// Find functions with database effect
const dbFunctions = symbols.filter(
  s => s.effect_closure.includes("database")
);
```

## Future Work

### Multi-hop Traversal

Support unbounded depth traversal:

```covenant
step id="s1" kind="traverse"
  target="project"
  from="kb.design.philosophy"
  follow rel=contained_by
  depth=unbounded
  direction=outgoing
  as="ancestors"
end
```

### Relation Conditions in WHERE

Support relation-based filtering:

```covenant
where
  and
    equals field="kind" lit="data"
    rel_to target="auth.login" type=describes
  end
end
```

### Query Indexing

Build indexes for O(1) lookups:
- Hash index on `kind` field
- Hash index on `id` field
- Inverted index on content terms

### JOIN Support

Compile JOIN clauses to WASM:

```covenant
step id="s1" kind="query"
  target="project"
  select all
  from="functions"
  join from="requirements" on field="covers" equals var="req_id"
  as="covered_functions"
end
```

## See Also

- [QUERY_SEMANTICS.md](../design/QUERY_SEMANTICS.md) - Full query specification
- [complete-runtime-query-system.md](../../.claude/plans/complete-runtime-query-system.md) - Implementation plan with phases
- [gai_codegen.rs](../../crates/covenant-codegen/src/gai_codegen.rs) - GAI function implementation
- [snippet_wasm.rs](../../crates/covenant-codegen/src/snippet_wasm.rs) - Query compilation (compile_project_query)
- [data_graph.rs](../../crates/covenant-codegen/src/data_graph.rs) - Data graph construction
- [embeddable.rs](../../crates/covenant-codegen/src/embeddable.rs) - Symbol metadata embedding
