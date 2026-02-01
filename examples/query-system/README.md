# Query System Examples

Covenant's embedded query system compiles documentation into WASM modules that can be queried at runtime. This directory demonstrates a cohesive pipeline from document ingestion to interactive querying.

## Pipeline Overview

```
docs/guide/*.md  →  doc-ingestion.wasm  →  output/*.cov  →  build.sh  →  *.wasm  →  query-repl.ts
   (source)           (ingestion)          (data nodes)    (compile)    (query)      (REPL)
```

1. **Ingest**: `doc-ingestion.cov` reads external documentation and generates `.cov` data files
2. **Build**: `build.sh` concatenates generated data with query examples and compiles to WASM
3. **Query**: `query-repl.ts` provides an interactive CLI to search and explore

## Quick Start

```bash
# Build everything
./build.sh

# Start the REPL
deno run --allow-read query-repl.ts

# In the REPL:
query> :load output/rag-query.wasm
query> :query effects
query> get_all_docs
query> :quit
```

## Files

| File | Description |
|------|-------------|
| `doc-ingestion.cov` | Ingests docs/guide/ → generates .cov data files + index.cov |
| `embedded-query.cov` | Basic queries: find_docs, ORDER BY, LIMIT |
| `parameterized-query.cov` | Dynamic search with runtime string parameters |
| `relation-traversal.cov` | Graph traversal from the generated index node |
| `rag-query.cov` | Full RAG system: search, traverse, hierarchy, code-doc linking |
| `build.sh` | Build script: ingest → concatenate → compile |
| `query-repl.ts` | Interactive REPL for querying compiled modules |
| `run-ingestion.ts` | Runner for the ingestion WASM |

## REPL Commands

```
:load <file.wasm>   Load a compiled module
:query <term>       Search all data nodes for term (easy search)
:list               List available query functions
:nodes              List all nodes in the module
:help               Show help
:quit               Exit

Direct function calls:
  get_all_docs            Call a no-arg function
  search_by_keyword "x"   Call with string argument
```

## Learning Progression

1. **embedded-query.cov** - Basic queries, ORDER BY, LIMIT
2. **parameterized-query.cov** - Runtime string parameters (`var="term"`)
3. **relation-traversal.cov** - Graph navigation from index node
4. **rag-query.cov** - Complete RAG with search + traversal + hierarchy

## Key Concepts

### Data Ingestion

`doc-ingestion.cov` reads markdown files and generates:
- Individual `.cov` files for each document
- An `index.cov` with `contains` relations to all documents

### Querying Embedded Data

Query functions use `target="project"` to query embedded data:

```covenant
step id="s1" kind="query"
  target="project"
  select all
  from="snippets"
  where
    contains field="content" var="search_term"
  end
  as="results"
end
```

### Relation Traversal

Navigate the document graph using traverse steps:

```covenant
step id="s1" kind="traverse"
  target="project"
  from="index"
  follow type=contains
  depth=1
  direction=outgoing
  as="all_docs"
end
```

## Testing

```bash
deno run --allow-read test-embedded.ts
deno run --allow-read test-parameterized.ts
deno run --allow-read test-rag.ts
```
