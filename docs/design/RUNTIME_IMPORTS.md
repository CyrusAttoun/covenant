# Runtime Imports and Cross-Environment Support

## Summary

Covenant uses **compile-time binding** to resolve extern snippets to actual JavaScript/npm libraries. The compiler finds installed packages and generates the appropriate WASM imports and glue code.

---

## The Binding Model

### Extern Snippets Reference Real Libraries

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
  contract="axios.get@1"   // <-- References real npm package
end

end
```

The `contract="axios.get@1"` means:
- Look for the `axios` package (installed via npm/yarn/pnpm)
- Bind to its `.get` method
- Version constraint `@1` for semver compatibility

### Two Sources of Bindings

1. **npm packages** — Standard JS libraries installed in the project
   - `contract="axios.get@1"` → `node_modules/axios`
   - `contract="pg.query@8"` → `node_modules/pg`

2. **Curated Covenant registry** — Pre-approved bindings with verified type signatures
   - Standard library bindings (console, math, etc.)
   - Platform-specific bindings with known behavior
   - Security-vetted external service bindings

---

## Compile-Time Resolution

### How the Compiler Finds Bindings

```
Source Code (.cov) with extern snippets
       ↓
   Compiler scans for contract="..." references
       ↓
   Resolves each contract:
     - Check node_modules first (project-local bindings)
     - Fall back to Covenant registry (shipped defaults)
     - Emit info if local shadows registry
       ↓
   Validates signature matches actual library
       ↓
   Generates WASM imports + JS glue code
       ↓
   .wasm module + runtime.js
```

### Validation at Compile Time

The compiler must verify:
1. **Package exists** — Is `axios` installed?
2. **Export exists** — Does `axios` have a `.get` method?
3. **Signature compatible** — Can we marshal Covenant types to/from the JS function?
4. **Effects match** — Does this function actually do network I/O?

If any check fails → **compile error**, not runtime error.

### Overriding Registry Bindings

Resolution is **project-first**: local bindings in `node_modules` take precedence over Covenant registry defaults. This mirrors standard npm resolution behavior.

**Why project-first?**
- Users can patch bugs without waiting for registry updates
- Easy to test with mock bindings
- Reduces single-point-of-failure on the registry
- Matches developer expectations from npm ecosystem

**Informational output:**
```
Info: Using local binding '@covenant/http' (shadows registry default)
```

This is informational only, not a warning—overriding is a legitimate choice.

---

## Multi-Environment Support

### Separate Extern Files Per Platform

For functionality that differs by platform, use **separate extern snippet files**:

```
stdlib/
  http-browser.cov    # Uses fetch() API
  http-node.cov       # Uses undici/node-fetch
  fs-node.cov         # Node filesystem (no browser equivalent)
  storage-browser.cov # IndexedDB
  storage-node.cov    # SQLite or filesystem
```

**http-browser.cov:**
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
  contract="@browser/fetch.get"  // Built-in browser fetch
  platform="browser"
end

end
```

**http-node.cov:**
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
  contract="undici.request@6"    // npm package for Node
  platform="node"
end

end
```

### Compilation Selects the Right Bindings

```bash
# Compiles with browser extern snippets
covenant compile --target=browser app.cov

# Compiles with Node extern snippets
covenant compile --target=node app.cov
```

The compiler:
1. Loads extern snippets matching the target platform
2. Resolves contracts to installed packages (or browser built-ins)
3. Rejects code that uses unavailable platform features

---

## What Gets Generated

### For a Browser Target

```
dist/
  app.wasm           # Compiled Covenant code
  runtime.js         # Generated glue code
```

**runtime.js** (generated):
```javascript
// Auto-generated bindings for browser target
const imports = {
  "http.get": async (urlPtr, urlLen) => {
    const url = readString(memory, urlPtr, urlLen);
    const response = await fetch(url);  // Browser fetch API
    return writeResponse(memory, response);
  }
};

export async function instantiate() {
  const wasm = await WebAssembly.instantiateStreaming(
    fetch('app.wasm'),
    { env: imports }
  );
  return wasm.instance.exports;
}
```

### For a Node Target

**runtime.js** (generated):
```javascript
import { request } from 'undici';  // Resolved from node_modules

const imports = {
  "http.get": async (urlPtr, urlLen) => {
    const url = readString(memory, urlPtr, urlLen);
    const response = await request(url);  // undici
    return writeResponse(memory, response);
  }
};

export async function instantiate() {
  const wasmBuffer = await fs.readFile('app.wasm');
  const wasm = await WebAssembly.instantiate(wasmBuffer, { env: imports });
  return wasm.instance.exports;
}
```

---

## Handling Incompatible Libraries

### Scenario: Code Uses Node-Only Library

```covenant
step id="s1" kind="call"
  fn="fs.readFile"              // Only available on Node
  arg name="path" lit="data.txt"
  as="contents"
end
```

**Compile for Node:** ✅ Works — `fs-node.cov` provides the binding

**Compile for browser:** ❌ Error
```
Error E_BINDING_001: No binding for 'fs.readFile' on target 'browser'
  --> mycode.cov:5:3
   |
 5 |   fn="fs.readFile"
   |   ^^^^^^^^^^^^^^^^ 'fs.readFile' is only available on 'node' target
   |
   = hint: Consider using 'storage.get' for cross-platform storage
```

---

## Architecture Summary

```
┌─────────────────────────────────────────────────────────────┐
│                     Covenant Project                         │
├─────────────────────────────────────────────────────────────┤
│  src/                                                        │
│    app.cov          # Application code                       │
│                                                              │
│  stdlib/            # Platform-specific extern bindings      │
│    http-browser.cov                                          │
│    http-node.cov                                             │
│    fs-node.cov                                               │
│    storage-browser.cov                                       │
│    storage-node.cov                                          │
│                                                              │
│  node_modules/      # Installed npm packages                 │
│    axios/                                                    │
│    undici/                                                   │
│    pg/                                                       │
└─────────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────┐
│              covenant compile --target=browser               │
├─────────────────────────────────────────────────────────────┤
│  1. Parse app.cov                                            │
│  2. Load *-browser.cov extern snippets                       │
│  3. Resolve contracts to browser APIs / npm packages         │
│  4. Validate all bindings exist                              │
│  5. Generate app.wasm + runtime.js                           │
└─────────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────┐
│                    dist/ (browser)                           │
│  app.wasm      - Compiled WASM module                        │
│  runtime.js    - Generated glue code with browser bindings   │
└─────────────────────────────────────────────────────────────┘
```

---

## Cross-Platform Abstractions

Some capabilities can be abstracted to work on both platforms with different underlying implementations.

### Example: Storage Abstraction

Instead of platform-specific `fs.readFile` (Node) or `localStorage` (browser), define a **unified storage interface**:

**storage.cov** (abstract interface):
```covenant
snippet id="storage.get" kind="extern"

effects
  effect storage
end

signature
  fn name="get"
    param name="key" type="String"
    returns union
      type="Bytes" optional
      type="StorageError"
    end
  end
end

end

snippet id="storage.put" kind="extern"

effects
  effect storage
end

signature
  fn name="put"
    param name="key" type="String"
    param name="value" type="Bytes"
    returns union
      type="Unit"
      type="StorageError"
    end
  end
end

end
```

### Platform Implementations

**storage-browser.cov:**
```covenant
snippet id="storage.get" kind="extern"

effects
  effect storage
end

signature
  fn name="get"
    param name="key" type="String"
    returns union
      type="Bytes" optional
      type="StorageError"
    end
  end
end

metadata
  contract="@browser/indexeddb.get"
  platform="browser"
  // Could also use: localStorage, service worker cache,
  // or CRUD requests to a backend endpoint
end

end
```

**storage-node.cov:**
```covenant
snippet id="storage.get" kind="extern"

effects
  effect storage
end

signature
  fn name="get"
    param name="key" type="String"
    returns union
      type="Bytes" optional
      type="StorageError"
    end
  end
end

metadata
  contract="@covenant/storage-fs.get"
  platform="node"
  // Implementation uses fs module or SQLite
end

end
```

### Browser Storage Options

For browser, the storage abstraction could be backed by:

| Backend | Use Case |
|---------|----------|
| **IndexedDB** | Large binary data, offline-first apps |
| **localStorage** | Small key-value data (5MB limit) |
| **Service Worker Cache** | HTTP response caching |
| **Backend CRUD API** | Server-side persistence via REST/GraphQL |

**Example: Browser storage backed by REST API:**
```javascript
// Generated runtime.js for browser with REST backend
const imports = {
  "storage.get": async (keyPtr, keyLen) => {
    const key = readString(memory, keyPtr, keyLen);
    const response = await fetch(`/api/storage/${key}`);
    if (response.status === 404) return writeNone(memory);
    const data = await response.arrayBuffer();
    return writeBytes(memory, data);
  },
  "storage.put": async (keyPtr, keyLen, valuePtr, valueLen) => {
    const key = readString(memory, keyPtr, keyLen);
    const value = readBytes(memory, valuePtr, valueLen);
    await fetch(`/api/storage/${key}`, {
      method: 'PUT',
      body: value
    });
    return writeUnit(memory);
  }
};
```

### Abstraction Levels

```
┌─────────────────────────────────────────────────────────────┐
│           Application Code (platform-agnostic)              │
│                                                             │
│   step id="s1" kind="call"                                  │
│     fn="storage.get"     // Uses abstract interface         │
│     arg name="key" lit="user-prefs"                         │
│     as="prefs"                                              │
│   end                                                       │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              Abstract Extern Interface                       │
│                   storage.cov                                │
│         (defines signature, effects, types)                  │
└─────────────────────────────────────────────────────────────┘
                            │
              ┌─────────────┴─────────────┐
              ▼                           ▼
┌───────────────────────┐   ┌───────────────────────┐
│   storage-browser.cov │   │   storage-node.cov    │
│                       │   │                       │
│ IndexedDB             │   │ fs module             │
│ localStorage          │   │ SQLite                │
│ REST API backend      │   │ LevelDB               │
└───────────────────────┘   └───────────────────────┘
```

### Benefits

1. **Write once, run anywhere** — Application code uses `storage.get`, works on both platforms
2. **Configurable backends** — Browser apps can choose IndexedDB vs REST API based on needs
3. **Same type safety** — Compiler validates calls against the abstract interface
4. **Progressive enhancement** — Start with localStorage, upgrade to IndexedDB or backend API

---

## WASI Target

In addition to JavaScript targets, Covenant supports **WASI 0.2 Component Model** output for maximum portability.

### Compilation

```bash
# WASI 0.2 Component Model
covenant compile --target=wasi app.cov

# JavaScript targets (existing)
covenant compile --target=browser app.cov
covenant compile --target=node app.cov
```

### How WASI Differs from JS Targets

| Aspect | `--target=browser/node` | `--target=wasi` |
|--------|-------------------------|-----------------|
| Output | `.wasm` + `runtime.js` | `.wasm` component only |
| Host | Browser or Node.js | Any WASI 0.2 runtime |
| Bindings | Generated JS glue | WIT interface imports |
| Extern resolution | npm packages | Host-provided interfaces |

### Extern Snippets with WASI

Extern snippets can specify both JS and WASI bindings:

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
  // JS targets
  contract="undici.request@6"
  platform="node"

  // WASI target
  wasi_interface="wasi:http/outgoing-handler"
end

end
```

The compiler selects the appropriate binding based on `--target`.

### Effect to WASI Interface Mapping

| Effect | WASI Interface |
|--------|----------------|
| `effect network` | `wasi:http/outgoing-handler` |
| `effect filesystem` | `wasi:filesystem/types` |
| `effect storage` | `wasi:keyvalue/store` |
| `effect random` | `wasi:random/random` |
| `effect database` | `covenant:database/sql` (custom) |

See [WASI_INTEGRATION.md](WASI_INTEGRATION.md) for full details on WIT definitions and custom interfaces.

---

## Key Points

1. **Contracts reference real packages** — `contract="axios.get@1"` finds axios in node_modules
2. **Compile-time resolution** — Bindings are resolved and validated before WASM generation
3. **Separate files per platform** — `http-browser.cov` vs `http-node.cov`
4. **Incompatible usage = compile error** — Can't use `fs.readFile` when targeting browser
5. **Generated glue code** — Compiler produces `runtime.js` with platform-appropriate bindings
6. **Cross-platform abstractions** — Higher-level interfaces (like `storage`) can have different implementations per platform while keeping application code portable
7. **WASI target** — Use `--target=wasi` for portable Component Model output that runs on any WASI 0.2 runtime
