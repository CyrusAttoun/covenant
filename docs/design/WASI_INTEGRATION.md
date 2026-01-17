# WASI 0.2 Integration

Covenant targets WASI 0.2 (Component Model) for portable, sandboxed execution across any compliant runtime.

---

## Overview

### Target Architecture

```
Covenant Source (.cov)
       ↓
   Compiler
       ↓
┌─────────────────────────────────────┐
│  WASM Component (.wasm)              │
│  ┌─────────────────────────────────┐ │
│  │ Imports (WIT interfaces)        │ │
│  │  - wasi:http                    │ │
│  │  - wasi:filesystem              │ │
│  │  - covenant:database            │ │
│  │  - covenant:project             │ │
│  └─────────────────────────────────┘ │
│  ┌─────────────────────────────────┐ │
│  │ Exports                          │ │
│  │  - main entry point             │ │
│  │  - exported functions           │ │
│  └─────────────────────────────────┘ │
└─────────────────────────────────────┘
       ↓
   WASI 0.2 Runtime
   (Wasmtime, Wasmer, wazero)
```

### Compilation Targets

```bash
# WASI 0.2 Component Model (recommended)
covenant compile --target=wasi app.cov

# JavaScript targets (backward compatible)
covenant compile --target=browser app.cov
covenant compile --target=node app.cov
```

---

## Effect to Interface Mapping

Covenant effects map directly to WIT interface imports:

| Effect | WIT Interface | Status |
|--------|---------------|--------|
| `effect network` | `wasi:http/outgoing-handler` | WASI 0.2 stable |
| `effect filesystem` | `wasi:filesystem/types` | WASI 0.2 stable |
| `effect storage` | `wasi:keyvalue/store` | WASI 0.2 Phase 2 |
| `effect random` | `wasi:random/random` | WASI 0.2 stable |
| `effect database` | `covenant:database/sql` | Custom (see below) |
| `effect std.concurrent` | `future<T>`, subtasks | WASI 0.3 (Nov 2025) |

### Example: Effect Declaration to Imports

```covenant
snippet id="app.fetch_user" kind="fn"

effects
  effect network
  effect database
end

// ... body
end
```

Compiles to component with imports:
```wit
// Generated component world
world app {
  import wasi:http/outgoing-handler;
  import covenant:database/sql;

  export fetch-user: func(id: s32) -> result<user, error>;
}
```

---

## Standard WASI 0.2 Interfaces

### wasi:http (effect network)

Used for HTTP client operations. Maps to extern snippets like `http.get`, `http.post`.

```wit
// From wasi:http/outgoing-handler
interface outgoing-handler {
  handle: func(
    request: outgoing-request,
    options: option<request-options>
  ) -> result<future-incoming-response, error-code>;
}
```

**Covenant binding:**
```covenant
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
  wasi_interface="wasi:http/outgoing-handler"
  wasi_function="handle"
end

end
```

### wasi:filesystem (effect filesystem)

Used for file operations. Maps to `fs.read`, `fs.write`, etc.

```wit
// From wasi:filesystem/types
interface types {
  read: func(fd: descriptor, len: filesize, offset: filesize) -> result<tuple<list<u8>, bool>, error-code>;
  write: func(fd: descriptor, buf: list<u8>, offset: filesize) -> result<filesize, error-code>;
  // ...
}
```

### wasi:keyvalue (effect storage)

Used for key-value storage operations.

```wit
// From wasi:keyvalue/store
interface store {
  get: func(bucket: borrow<bucket>, key: string) -> result<option<list<u8>>, error>;
  set: func(bucket: borrow<bucket>, key: string, value: list<u8>) -> result<_, error>;
  delete: func(bucket: borrow<bucket>, key: string) -> result<_, error>;
  exists: func(bucket: borrow<bucket>, key: string) -> result<bool, error>;
}
```

**Note:** wasi:keyvalue is Phase 2. Consider using it for `effect storage` but have fallback.

### wasi:random (effect random)

Used for cryptographic random number generation.

```wit
// From wasi:random/random
interface random {
  get-random-bytes: func(len: u64) -> list<u8>;
  get-random-u64: func() -> u64;
}
```

### wasi:clocks

Used for time operations and metering.

```wit
// From wasi:clocks/monotonic-clock
interface monotonic-clock {
  now: func() -> instant;
  resolution: func() -> duration;
  subscribe-instant: func(when: instant) -> pollable;
  subscribe-duration: func(when: duration) -> pollable;
}
```

---

## Custom Covenant Interfaces

### covenant:database/sql

wasi-sql is dormant (last commit Feb 2024). Covenant defines its own database interface supporting dialect-specific SQL.

```wit
// covenant:database/sql
package covenant:database;

interface sql {
    /// Supported SQL dialects
    enum dialect {
        postgres,
        mysql,
        sqlserver,
        sqlite,
    }

    /// Parameter value types
    variant value {
        null,
        bool(bool),
        int(s64),
        float(f64),
        text(string),
        blob(list<u8>),
    }

    /// A single row from query results
    record row {
        columns: list<tuple<string, value>>,
    }

    /// Query result set
    resource rows {
        /// Get next row, none if exhausted
        next: func() -> option<row>;

        /// Get column names
        columns: func() -> list<string>;

        /// Get number of rows affected (for INSERT/UPDATE/DELETE)
        rows-affected: func() -> u64;
    }

    /// Database connection handle
    resource connection {
        /// Open connection to database
        /// connection-string format is dialect-specific
        open: static func(dialect: dialect, connection-string: string) -> result<connection, error>;

        /// Execute query with parameters, return result rows
        query: func(sql: string, params: list<value>) -> result<rows, error>;

        /// Execute statement (INSERT/UPDATE/DELETE), return rows affected
        execute: func(sql: string, params: list<value>) -> result<u64, error>;

        /// Begin transaction
        begin: func() -> result<transaction, error>;
    }

    /// Transaction handle
    resource transaction {
        /// Execute query within transaction
        query: func(sql: string, params: list<value>) -> result<rows, error>;

        /// Execute statement within transaction
        execute: func(sql: string, params: list<value>) -> result<u64, error>;

        /// Commit transaction
        commit: func() -> result<_, error>;

        /// Rollback transaction
        rollback: func() -> result<_, error>;
    }

    /// Database error
    record error {
        code: string,
        message: string,
        /// Dialect-specific error code if available
        dialect-code: option<s32>,
    }
}
```

**Placeholder syntax by dialect:**

| Dialect | Placeholder | Example |
|---------|-------------|---------|
| postgres | `$1, $2, ...` | `SELECT * FROM users WHERE id = $1` |
| mysql | `?` | `SELECT * FROM users WHERE id = ?` |
| sqlserver | `@p1, @p2, ...` | `SELECT * FROM users WHERE id = @p1` |
| sqlite | `?1, ?2, ...` or `?` | `SELECT * FROM users WHERE id = ?1` |

The host runtime translates parameter indices to dialect-appropriate placeholders.

### covenant:project/query

For Covenant's project query system (querying the symbol graph, AST navigation).

```wit
// covenant:project/query
package covenant:project;

interface query {
    /// Node kinds in the symbol graph
    enum node-kind {
        snippet,
        function,
        type-def,
        requirement,
        test,
        data,
        relation,
    }

    /// A node in the symbol graph
    record node {
        id: string,
        kind: node-kind,
        /// JSON-encoded node data
        data: string,
    }

    /// Query result iterator
    resource query-result {
        /// Get next matching node
        next: func() -> option<node>;

        /// Get count of results (may require full iteration)
        count: func() -> u64;
    }

    /// Execute a Covenant query against the project symbol graph
    /// query-ast is the JSON-encoded query AST
    query: func(query-ast: string) -> result<query-result, error>;

    /// Get a single node by ID
    get: func(id: string) -> option<node>;

    /// Get nodes related to a given node
    get-related: func(id: string, relation-type: string) -> result<query-result, error>;

    /// Error from query execution
    record error {
        code: string,
        message: string,
    }
}
```

**Usage:** The host runtime loads the compiled symbol graph and executes queries against it. This enables Covenant's powerful introspection capabilities.

---

## Structured Concurrency (WASI 0.3)

Covenant's `std.concurrent.parallel` and `std.concurrent.race` constructs require native async support in the Component Model.

### WASI 0.3 Features (Preview Aug 2025, Stable Nov 2025)

WASI 0.3 introduces composable concurrency at the component model level:

1. **`stream<T>` and `future<T>` types** — First-class async primitives in WIT
2. **No function coloring** — Async is a runtime property, not a code property
3. **Composable concurrency** — Components handle I/O without blocking others
4. **Language-agnostic** — Works across Rust, C++, Go, etc.

### Mapping Covenant to WASI 0.3

| Covenant Construct | WASI 0.3 Mapping |
|--------------------|------------------|
| `std.concurrent.parallel` | Multiple `future<T>` spawned, all awaited |
| `std.concurrent.race` | Multiple `future<T>` spawned, first-to-complete returned |
| `on_error="fail_fast"` | Cancel pending futures on first error |
| `on_error="collect_all"` | Await all futures, aggregate errors |
| `timeout=5s` | Race with `future<T>` from clock subscription |
| Branch isolation | Each branch is separate subtask with own memory |

### Compilation Strategy for WASI 0.3

**`std.concurrent.parallel` compiles to:**

```wit
// Each branch becomes a future
parallel-results: func() -> tuple<future<users-result>, future<products-result>>;
```

```
// Covenant source
step id="s1" kind="std.concurrent.parallel"
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
```

**Compiles to (conceptual):**
```wit
// Generated async interface
get-users: async func() -> result<list<user>, error>;
get-products: async func() -> result<list<product>, error>;

// Parallel execution spawns subtasks
parallel-fetch: func() -> tuple<future<result<list<user>, error>>, future<result<list<product>, error>>>;
```

The runtime:
1. Spawns each branch as a subtask (WASI 0.3 subtask model)
2. Each subtask runs concurrently without blocking others
3. Results are collected in declaration order (deterministic)
4. Cancellation propagates on `fail_fast` or timeout

### `stream<T>` for Iterative Results

For operations that produce multiple values over time, Covenant queries could use `stream<T>`:

```wit
// Future enhancement: streaming query results
query-stream: func(query-ast: string) -> stream<node>;
```

This enables:
- Memory-efficient large result sets
- Backpressure-aware iteration
- Early termination without loading all results

### Compatibility Notes

**What aligns well:**
- Covenant's branch isolation matches WASI 0.3's subtask model
- Deterministic result ordering is preserved
- Effect tracking maps to capability-based imports
- No shared mutable state (Covenant design principle)

**What may need adjustment:**
- `on_timeout="return_partial"` — May need custom handling if WASI 0.3 doesn't support partial results natively
- Nested parallel blocks — Verify subtask spawning is recursive
- Error aggregation (`collect_all`) — May need manual future collection

### Timeline

| Milestone | Date | Covenant Impact |
|-----------|------|-----------------|
| WASI 0.3 Preview | Aug 2025 | Begin integration testing |
| WASI 0.3 Stable | Nov 2025 | Enable `--target=wasi` with `effect std.concurrent` |
| Covenant 1.0 | TBD | Full WASI 0.3 support |

### Until WASI 0.3

- Structured concurrency works in `--target=browser` and `--target=node` (Promise-based)
- `--target=wasi` compilation fails if `effect std.concurrent` is used

```
Error E_WASI_001: Structured concurrency requires WASI 0.3
  --> app.cov:5:3
   |
 5 |   effect std.concurrent
   |   ^^^^^^^^^^^^^^^^^^^^^ 'std.concurrent' not available on WASI 0.2 target
   |
   = hint: Use --target=browser or --target=node, or wait for WASI 0.3 support
```

---

## Runtime Host Requirements

A WASI 0.2 host running Covenant components must provide:

### Required (WASI 0.2 Standard)
- `wasi:http/outgoing-handler` — HTTP client
- `wasi:filesystem/types` — File operations
- `wasi:clocks/monotonic-clock` — Time operations
- `wasi:random/random` — Random number generation

### Required (Covenant Custom)
- `covenant:database/sql` — Database connections
- `covenant:project/query` — Symbol graph queries

### Optional
- `wasi:keyvalue/store` — Key-value storage (if `effect storage` used)

### Recommended Runtimes

| Runtime | Language | WASI 0.2 | Notes |
|---------|----------|----------|-------|
| Wasmtime | Rust | Full | Reference implementation, fuel metering |
| Wasmer | Rust | Full | Fast, WASI-X extensions |
| wazero | Go | Full | Pure Go, no CGO |
| WasmEdge | C++ | Partial | Focus on edge/AI |

---

## Metering and Resource Limits

WASI 0.2 does not standardize metering. Covenant relies on runtime-specific fuel systems.

### Wasmtime Fuel

```rust
// Host configuration
let mut config = Config::new();
config.consume_fuel(true);

let engine = Engine::new(&config)?;
let mut store = Store::new(&engine, ());
store.set_fuel(1_000_000)?; // 1M instructions

// Run component
let result = component.call(&mut store, args)?;
let remaining = store.get_fuel()?;
```

### Covenant Integration

The `timeout` and `cost_hint` metadata map to fuel budgets:

```covenant
metadata
  timeout=30s        // Translates to fuel budget based on runtime calibration
  cost_hint=moderate // Compiler hint, not enforced
end
```

---

## Migration from JavaScript Targets

Existing code using `--target=browser` or `--target=node` continues to work unchanged.

### What Changes with --target=wasi

| Aspect | JS Targets | WASI Target |
|--------|------------|-------------|
| Output | `.wasm` + `runtime.js` | `.wasm` component only |
| Host | Browser/Node.js | Any WASI 0.2 runtime |
| Bindings | Generated JS glue | WIT interfaces |
| Async | Promise-based | WASI 0.3 native (future) |
| Interop | JS ↔ WASM | WASM ↔ WASM components |

### Extern Snippet Compatibility

Extern snippets work across all targets. The `metadata` section specifies bindings:

```covenant
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
  // JS targets use npm package
  contract="undici.request@6"
  platform="node"

  // WASI target uses WIT interface
  wasi_interface="wasi:http/outgoing-handler"
  wasi_function="handle"
end

end
```

The compiler selects the appropriate binding based on `--target`.

---

## Future: WASI 0.3 and Beyond

### WASI 0.3 (Preview Aug 2025, Stable Nov 2025)

Key features for Covenant:
- **`future<T>` type** — Enables `std.concurrent.parallel` and `race`
- **`stream<T>` type** — Efficient streaming for query results
- **Subtask model** — Branch isolation maps directly
- **No function coloring** — Async is transparent at the ABI level

The "no function coloring" property is particularly important: Covenant functions don't need to be marked async/sync. The runtime handles concurrency transparently, which aligns with Covenant's design philosophy of keeping complexity out of the IR.

### Potential wasi-sql Revival
If Covenant gains traction, contributing dialect support to wasi-sql could benefit the broader ecosystem. Current status: dormant (last commit Feb 2024).

### WASI 1.0 (Expected 2026)
Stable standard for the next decade. Covenant should align with 1.0 when finalized.

---

## Related Documents

- [DESIGN.md](DESIGN.md) — Language design philosophy
- [RUNTIME_IMPORTS.md](RUNTIME_IMPORTS.md) — JavaScript binding model
- [COMPILER.md](COMPILER.md) — Compilation phases
- [EXTENSIBLE_KINDS.md](EXTENSIBLE_KINDS.md) — Custom kind definitions
