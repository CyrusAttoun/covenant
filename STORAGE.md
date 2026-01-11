# Covenant Storage Provider Specification

Defines the pluggable storage interface for Covenant's hybrid storage model.

---

## Overview

Covenant uses a **hybrid storage model**:

- **Source of truth:** `.cov` text files (human-readable, git-friendly)
- **Derived index:** Pluggable key-value store for fast queries

The storage provider interface abstracts the index implementation, allowing different backends (LMDB, redb, sled, etc.) without changing the compiler or language semantics.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Covenant Project                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  .cov Source Files (Source of Truth)                        │
│  ├── src/                                                   │
│  │   ├── auth.cov                                           │
│  │   ├── users.cov                                          │
│  │   └── docs/                                              │
│  │       └── overview.cov                                   │
│  │                                                          │
│  .covenant/  (Derived Index)                                │
│  ├── index.db          ← Storage Provider                   │
│  ├── version            (Symbol graph version)              │
│  └── config.json        (Provider configuration)            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Storage Provider Interface

### Layer 1: Core Operations (Required)

Every storage provider MUST implement these operations:

```
interface StorageProvider {
    // Read a node by ID
    // Returns None if node doesn't exist
    fn get(id: String) -> Option<Node>

    // Write a node (insert or update)
    // Returns error if write fails
    fn put(id: String, node: Node) -> Result<(), StorageError>

    // Delete a node by ID
    // Returns error if delete fails (no-op if node doesn't exist)
    fn delete(id: String) -> Result<(), StorageError>

    // List all nodes with IDs matching a prefix
    // Used for namespace queries like "auth.*"
    fn list(prefix: String) -> Iterator<Node>

    // Check if provider is initialized and ready
    fn is_ready() -> Bool

    // Initialize provider (create database, indexes, etc.)
    fn initialize() -> Result<(), StorageError>
}
```

### Layer 2: Index Queries (Required)

Providers MUST support efficient indexed lookups:

```
interface IndexedStorageProvider extends StorageProvider {
    // Find all nodes of a specific kind
    // O(k) where k = number of matching nodes
    fn query_by_kind(kind: SnippetKind) -> Iterator<Node>

    // Find all nodes with a specific effect
    // O(e) where e = number of nodes with effect
    fn query_by_effect(effect: String) -> Iterator<Node>

    // Find all nodes with a relation to a target
    // O(r) where r = number of relations to target
    fn query_by_relation(
        target: String,
        rel_type: RelationType
    ) -> Iterator<Node>

    // Execute a full query pattern
    // Complexity varies by query
    fn query(pattern: QueryAST) -> Iterator<Node>
}
```

**Required Indexes:**

| Index | Key | Value |
|-------|-----|-------|
| `kind_index` | SnippetKind | List[SnippetId] |
| `effect_index` | EffectName | List[SnippetId] |
| `relation_index` | (TargetId, RelationType) | List[SourceId] |
| `relation_reverse_index` | (SourceId, RelationType) | List[TargetId] |

### Layer 3: Transaction Semantics (Required)

Providers MUST support atomic transactions:

```
interface TransactionalStorageProvider extends IndexedStorageProvider {
    // Execute multiple operations atomically
    // All operations succeed or all fail (rollback)
    fn transaction(ops: List<StorageOp>) -> Result<(), StorageError>

    // Get current version of a node
    // Used for optimistic locking / conflict detection
    fn get_version(id: String) -> Option<Int>

    // Begin a read transaction (snapshot isolation)
    fn begin_read() -> ReadTransaction

    // Begin a write transaction
    fn begin_write() -> WriteTransaction
}

interface Transaction {
    fn commit() -> Result<(), StorageError>
    fn rollback()
}

enum StorageOp {
    Put { id: String, node: Node },
    Delete { id: String },
    UpdateIndex { index: String, key: String, value: Any },
}
```

**Transaction Guarantees:**

1. **Atomicity:** All operations in a transaction succeed or fail together
2. **Isolation:** Read transactions see a consistent snapshot
3. **Consistency:** Invariants (I1-I5) validated before commit
4. **Durability:** Committed transactions persist to disk

**Conflict Detection:**

```
// Optimistic locking using version numbers
fn update_with_lock(id: String, new_node: Node) -> Result<(), ConflictError> {
    expected_version = get_version(id)

    transaction([
        CheckVersion { id, expected_version },
        Put { id, node: new_node with version = expected_version + 1 }
    ])
}
```

---

## Node Schema

The `Node` type represents any storable entity:

```
struct Node {
    // Identity
    id: String,                    // e.g., "auth.login", "docs.overview"
    kind: SnippetKind,             // fn, struct, enum, data, etc.
    version: Int,                  // For optimistic locking

    // Source location
    source_file: String,           // e.g., "src/auth.cov"
    line_start: Int,
    line_end: Int,
    content_hash: String,          // SHA-256 of source text

    // AST
    ast: JSON,                     // Full AST of the snippet

    // Symbol graph data
    calls: List[String],           // Forward: functions this calls
    called_by: List[String],       // Backward: functions that call this
    references: List[String],      // Forward: types this references
    referenced_by: List[String],   // Backward: what references this

    // Effects
    effects: List[String],         // Declared effects
    effect_closure: List[String],  // Computed transitive closure

    // Relations (new)
    relations_to: List[Relation],  // Outgoing relations
    relations_from: List[Relation],// Incoming relations (auto-computed)

    // Notes (new)
    notes: List[Note],             // Queryable annotations

    // Requirements / Tests
    requirements: List[String],    // Requirement IDs in this snippet
    tests: List[String],           // Test IDs in this snippet
    covers: List[String],          // Requirements this test covers
    covered_by: List[String],      // Tests that cover this requirement

    // Metadata
    metadata: Map[String, Any],    // Arbitrary metadata
}

struct Relation {
    target: String,                // Target snippet ID
    type: RelationType,            // contains, describes, etc.
}

struct Note {
    lang: Optional[String],        // "en", "pseudo", "es", etc.
    content: String,               // Note text
}
```

---

## Recommended Backends

### LMDB (Lightning Memory-Mapped Database)

**Characteristics:**
- Memory-mapped for extremely fast reads
- Used by LDAP, Caffe, many production systems
- ACID compliant
- Read transactions are zero-copy

**Best for:** Read-heavy workloads, IDE integrations

```
// Configuration
{
    "provider": "lmdb",
    "path": ".covenant/index.lmdb",
    "map_size": "10GB",
    "max_readers": 128
}
```

### redb (Pure Rust Embedded Database)

**Characteristics:**
- Pure Rust, no C dependencies
- Simple API, good documentation
- ACID compliant
- Smaller binary size

**Best for:** Simplicity, cross-platform builds

```
{
    "provider": "redb",
    "path": ".covenant/index.redb"
}
```

### sled (Modern Embedded Database)

**Characteristics:**
- Lock-free reads
- Modern architecture
- Good for write-heavy workloads
- Still maturing (check stability before production)

**Best for:** Write-heavy workloads, modern systems

```
{
    "provider": "sled",
    "path": ".covenant/index.sled",
    "cache_capacity": "1GB"
}
```

### Memory (In-Memory Only)

**Characteristics:**
- No persistence
- Fastest possible reads/writes
- Data lost on process exit

**Best for:** Testing, development, short-lived processes

```
{
    "provider": "memory"
}
```

---

## Synchronization

### File → Index Sync

When `.cov` files change, the index must update:

```
fn sync_file(path: String) {
    // Parse file
    snippets = parse(read_file(path))

    // Find existing snippets from this file
    existing = provider.list(prefix = path)

    // Compute delta
    to_add = snippets - existing
    to_update = snippets ∩ existing (where content changed)
    to_delete = existing - snippets

    // Apply in transaction
    provider.transaction([
        ...to_add.map(s => Put { id: s.id, node: s }),
        ...to_update.map(s => Put { id: s.id, node: s }),
        ...to_delete.map(s => Delete { id: s.id }),
    ])
}
```

### Index → File Sync (for self-modifying code)

When code modifies the AST via `meta` effect:

```
fn persist_ast_change(id: String, new_ast: AST) {
    // Get source location
    node = provider.get(id)

    // Serialize AST back to .cov syntax
    new_source = serialize_to_cov(new_ast)

    // Write to file (replace lines)
    replace_lines(
        file = node.source_file,
        start = node.line_start,
        end = node.line_end,
        content = new_source
    )

    // Update index
    provider.put(id, node with ast = new_ast)
}
```

### Conflict Resolution

When both file and index have changes:

1. **File wins by default** — `.cov` files are source of truth
2. **Prompt on conflict** — If index has uncommitted AST mutations
3. **Version tracking** — Detect stale reads via `content_hash`

---

## Performance Requirements

| Operation | Target Latency | Notes |
|-----------|---------------|-------|
| `get(id)` | < 1ms | O(1) hash lookup |
| `put(id, node)` | < 5ms | Includes index updates |
| `query_by_kind(kind)` | < 10ms | For <1000 results |
| `query_by_relation(target, type)` | < 10ms | For <100 results |
| `transaction(ops)` | < 50ms | For <100 operations |
| `sync_file(path)` | < 100ms | For files with <50 snippets |

**Scalability Targets:**
- Support 100,000+ snippets per project
- Support 1,000,000+ relations
- Incremental updates without full rebuild

---

## Error Handling

```
enum StorageError {
    // Provider not initialized
    NotInitialized,

    // Node not found (for operations requiring existence)
    NotFound { id: String },

    // Version conflict during optimistic locking
    VersionConflict { id: String, expected: Int, actual: Int },

    // Transaction failed (details in inner error)
    TransactionFailed { reason: String },

    // I/O error (disk full, permissions, etc.)
    IoError { message: String },

    // Corruption detected
    Corruption { message: String },
}
```

---

## Configuration

Storage provider configuration in `.covenant/config.json`:

```json
{
    "storage": {
        "provider": "lmdb",
        "path": ".covenant/index.lmdb",
        "options": {
            "map_size": "10GB",
            "max_readers": 128,
            "sync_mode": "normal"
        }
    },
    "indexes": {
        "kind_index": true,
        "effect_index": true,
        "relation_index": true,
        "fulltext_index": false
    },
    "sync": {
        "auto_sync": true,
        "watch_files": true,
        "debounce_ms": 100
    }
}
```

---

## Migration

When changing providers or upgrading schema:

```
fn migrate(old_provider: StorageProvider, new_provider: StorageProvider) {
    new_provider.initialize()

    for node in old_provider.list("") {
        new_provider.put(node.id, node)
    }

    // Verify
    assert old_provider.list("").count() == new_provider.list("").count()
}
```

---

## Related Documents

- [DESIGN.md](DESIGN.md) - Section 11 (Storage Architecture)
- [COMPILER.md](COMPILER.md) - Incremental compilation
- [grammar.ebnf](grammar.ebnf) - Data node syntax
