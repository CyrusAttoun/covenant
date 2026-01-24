# Standard Library Reference

All functions available in Covenant's standard library, organized by module. Each function is called using its fully-qualified snippet ID and requires its declared effect.

---

## Table of Contents

1. [Console](#console)
2. [HTTP](#http)
3. [Filesystem](#filesystem)
4. [Crypto](#crypto)
5. [Database](#database)
6. [Process](#process)
7. [OS/Environment](#osenvironment)
8. [Path](#path)
9. [Timers](#timers)
10. [URL](#url)
11. [Storage: Key-Value](#storage-key-value)
12. [Storage: Document](#storage-document)

---

## Console

Output to the runtime console. All functions accept a primary message and an optional list of additional values.

**Effect:** `console`
**Platforms:** deno, browser, node, wasi

| Function | Parameters | Returns |
|----------|-----------|---------|
| `console.println` | `msg: String`, `values: List<String>` (optional) | `Unit` |
| `console.info` | `msg: String`, `values: List<String>` (optional) | `Unit` |
| `console.debug` | `msg: String`, `values: List<String>` (optional) | `Unit` |
| `console.warn` | `msg: String`, `values: List<String>` (optional) | `Unit` |
| `console.error` | `msg: String`, `values: List<String>` (optional) | `Unit` |

```
step id="s1" kind="call"
  fn="console.println"
  arg name="msg" lit="Hello, world!"
  as="_"
end

step id="s2" kind="call"
  fn="console.warn"
  arg name="msg" lit="Connection pool running low"
  arg name="values" from="pool_stats"
  as="_"
end

step id="s3" kind="call"
  fn="console.error"
  arg name="msg" lit="Failed to connect"
  arg name="values" from="error_details"
  as="_"
end
```

---

## HTTP

HTTP client operations using the platform's native fetch API.

**Effect:** `network`
**Platforms:** deno, browser, node

### Types

```
struct name="Response"
  field name="status" type="Int"
  field name="body" type="String"
  field name="headers" type="Map<String,String>"
end

struct name="HttpError"
  field name="message" type="String"
  field name="status" type="Int" optional
end
```

### Functions

| Function | Parameters | Returns |
|----------|-----------|---------|
| `http.get` | `url: String`, `headers: Map<String,String>` (optional), `timeout: Int` (optional) | `Response \| HttpError` |
| `http.post` | `url: String`, `body: String`, `headers: Map<String,String>` (optional), `content_type: String` (optional), `timeout: Int` (optional) | `Response \| HttpError` |
| `http.put` | `url: String`, `body: String`, `headers: Map<String,String>` (optional), `content_type: String` (optional), `timeout: Int` (optional) | `Response \| HttpError` |
| `http.delete` | `url: String`, `headers: Map<String,String>` (optional), `timeout: Int` (optional) | `Response \| HttpError` |
| `http.request` | `url: String`, `method: String`, `body: String` (optional), `headers: Map<String,String>` (optional), `content_type: String` (optional), `timeout: Int` (optional) | `Response \| HttpError` |

```
step id="s1" kind="call"
  fn="http.get"
  arg name="url" lit="https://api.example.com/users"
  arg name="headers" from="auth_headers"
  arg name="timeout" lit=5000
  as="response"
end

step id="s2" kind="call"
  fn="http.post"
  arg name="url" lit="https://api.example.com/users"
  arg name="body" from="payload"
  arg name="content_type" lit="application/json"
  as="result"
end

step id="s3" kind="call"
  fn="http.request"
  arg name="url" lit="https://api.example.com/resource"
  arg name="method" lit="PATCH"
  arg name="body" from="patch_data"
  arg name="headers" from="headers"
  arg name="content_type" lit="application/json"
  arg name="timeout" lit=10000
  as="patch_result"
end
```

---

## Filesystem

Read, write, and manage files and directories on the local filesystem.

**Effect:** `filesystem`
**Platforms:** deno
**Deno permission:** `--allow-read`, `--allow-write`

### Types

```
struct name="FileStat"
  field name="size" type="Int"
  field name="is_file" type="Bool"
  field name="is_directory" type="Bool"
  field name="modified" type="DateTime" optional
end

struct name="DirEntry"
  field name="name" type="String"
  field name="is_file" type="Bool"
  field name="is_directory" type="Bool"
end

struct name="FileHandle"
  field name="id" type="Int"
  field name="path" type="String"
end

struct name="IoError"
  field name="message" type="String"
  field name="code" type="String"
end
```

### Functions

| Function | Parameters | Returns | Description |
|----------|-----------|---------|-------------|
| `fs.read_file` | `path: String` | `String \| IoError` | Read entire file as text |
| `fs.write_file` | `path: String`, `content: String` | `Unit \| IoError` | Write text to file |
| `fs.write_bytes` | `path: String`, `content: Bytes` | `Unit \| IoError` | Write binary data to file |
| `fs.remove` | `path: String` | `Unit \| IoError` | Delete file or directory |
| `fs.stat` | `path: String` | `FileStat \| IoError` | Get file/directory metadata |
| `fs.readDir` | `path: String` | `List<DirEntry> \| IoError` | List directory contents |
| `fs.exists` | `path: String` | `Bool` | Check if path exists |
| `fs.mkdir` | `path: String`, `recursive: Bool` (optional) | `Unit \| IoError` | Create directory |
| `fs.copy` | `src: String`, `dest: String` | `Unit \| IoError` | Copy file |
| `fs.rename` | `src: String`, `dest: String` | `Unit \| IoError` | Rename or move file |
| `fs.chmod` | `path: String`, `mode: Int` | `Unit \| IoError` | Set file permissions |
| `fs.open` | `path: String`, `mode: String` | `FileHandle \| IoError` | Open file for streaming I/O |
| `fs.read_bytes` | `handle: FileHandle`, `offset: Int` (optional), `length: Int` (optional) | `Bytes \| IoError` | Read bytes from open file |
| `fs.close` | `handle: FileHandle` | `Unit` | Close an open file handle |

### Basic File I/O

```
step id="s1" kind="call"
  fn="fs.read_file"
  arg name="path" lit="./config.json"
  as="contents"
end

step id="s2" kind="call"
  fn="fs.write_file"
  arg name="path" lit="./output.txt"
  arg name="content" from="data"
  as="_"
end
```

### Directory Operations

```
step id="s1" kind="call"
  fn="fs.mkdir"
  arg name="path" lit="./output/reports"
  arg name="recursive" lit=true
  as="_"
end

step id="s2" kind="call"
  fn="fs.readDir"
  arg name="path" lit="./output"
  as="entries"
end
```

### Streaming I/O (Partial Reads)

For reading large files in chunks rather than loading the entire file into memory:

```
step id="s1" kind="call"
  fn="fs.open"
  arg name="path" lit="./large-file.bin"
  arg name="mode" lit="read"
  as="handle"
end

step id="s2" kind="call"
  fn="fs.read_bytes"
  arg name="handle" from="handle"
  arg name="offset" lit=0
  arg name="length" lit=4096
  as="chunk"
end

step id="s3" kind="call"
  fn="fs.close"
  arg name="handle" from="handle"
  as="_"
end
```

### File Management

```
step id="s1" kind="call"
  fn="fs.exists"
  arg name="path" lit="./config.json"
  as="config_exists"
end

step id="s2" kind="call"
  fn="fs.copy"
  arg name="src" lit="./config.json"
  arg name="dest" lit="./config.backup.json"
  as="_"
end

step id="s3" kind="call"
  fn="fs.chmod"
  arg name="path" lit="./script.sh"
  arg name="mode" lit=755
  as="_"
end
```

---

## Crypto

Cryptographic hashing and random number generation.

**Platforms:** deno, node

| Function | Parameters | Returns | Effect |
|----------|-----------|---------|--------|
| `crypto.sha256` | `input: String` | `String` | _(pure)_ |
| `crypto.random_bytes` | `length: Int` | `Bytes` | `random` |

```
step id="s1" kind="call"
  fn="crypto.sha256"
  arg name="input" lit="hello"
  as="hash"
end

step id="s2" kind="call"
  fn="crypto.random_bytes"
  arg name="length" lit=32
  as="token"
end
```

---

## Database

Connect to and query external SQL databases.

**Effect:** `database`
**Platforms:** deno, node

| Function | Parameters | Returns |
|----------|-----------|---------|
| `db.connect` | `url: String` | `Connection \| DbError` |
| `db.query` | `conn: Connection`, `sql: String`, `params: List<String>` (optional) | `Rows \| DbError` |

### Basic Query

```
step id="s1" kind="call"
  fn="db.connect"
  arg name="url" lit="env:DATABASE_URL"
  as="conn"
end

step id="s2" kind="call"
  fn="db.query"
  arg name="conn" from="conn"
  arg name="sql" lit="SELECT * FROM users WHERE active = true"
  as="rows"
end
```

### Parameterized Query

Use the `params` argument for safe parameter binding (prevents SQL injection):

```
step id="s1" kind="call"
  fn="db.query"
  arg name="conn" from="conn"
  arg name="sql" lit="SELECT * FROM users WHERE age > $1 AND status = $2"
  arg name="params" from="query_params"
  as="rows"
end
```

### SQL Dialect Syntax (Preferred for Complex Queries)

For complex queries with compile-time type checking, prefer the SQL dialect query syntax:

```
step id="s1" kind="query"
  dialect="postgres"
  target="app_db"
  body
    SELECT id, name FROM users WHERE created_at > :cutoff AND status = :status
  end
  params
    param name="cutoff" from="cutoff_date"
    param name="status" lit="active"
  end
  returns collection of="User"
  as="recent_users"
end
```

---

## Process

Execute external programs and manage subprocesses.

**Effect:** `process`
**Platforms:** deno
**Deno permission:** `--allow-run`

### Types

```
struct name="ProcessResult"
  field name="exit_code" type="Int"
  field name="stdout" type="String"
  field name="stderr" type="String"
end

struct name="ProcessHandle"
  field name="pid" type="Int"
end

struct name="ProcessError"
  field name="message" type="String"
  field name="code" type="String"
end
```

### Functions

| Function | Parameters | Returns | Description |
|----------|-----------|---------|-------------|
| `process.exec` | `command: String`, `args: List<String>` (optional), `cwd: String` (optional), `env: Map<String,String>` (optional), `timeout: Int` (optional) | `ProcessResult \| ProcessError` | Run command and wait for completion |
| `process.spawn` | `command: String`, `args: List<String>` (optional), `cwd: String` (optional), `env: Map<String,String>` (optional) | `ProcessHandle \| ProcessError` | Start a background process |
| `process.kill` | `handle: ProcessHandle` | `Unit` | Terminate a spawned process |
| `process.wait` | `handle: ProcessHandle` | `ProcessResult \| ProcessError` | Wait for a spawned process to finish |

### Run a Command

```
step id="s1" kind="call"
  fn="process.exec"
  arg name="command" lit="git"
  arg name="args" lit=["status", "--porcelain"]
  arg name="cwd" lit="/project"
  arg name="timeout" lit=30000
  as="result"
end

step id="s2" kind="bind"
  field="stdout" of="result"
  as="git_status"
end
```

### Spawn and Manage a Background Process

```
step id="s1" kind="call"
  fn="process.spawn"
  arg name="command" lit="node"
  arg name="args" lit=["server.js"]
  arg name="env" from="server_env"
  as="server"
end

// ... do other work ...

step id="s2" kind="call"
  fn="process.kill"
  arg name="handle" from="server"
  as="_"
end
```

---

## OS/Environment

Access operating system information and environment variables.

**Effect:** `os`
**Platforms:** deno, node
**Deno permission:** `--allow-env`

| Function | Parameters | Returns | Description |
|----------|-----------|---------|-------------|
| `os.env_get` | `name: String` | `String?` | Get environment variable (none if unset) |
| `os.env_set` | `name: String`, `value: String` | `Unit` | Set environment variable |
| `os.platform` | _(none)_ | `String` | OS name: "linux", "darwin", "windows" |
| `os.arch` | _(none)_ | `String` | CPU architecture: "x86_64", "aarch64" |

```
step id="s1" kind="call"
  fn="os.env_get"
  arg name="name" lit="DATABASE_URL"
  as="db_url"
end

step id="s2" kind="call"
  fn="os.env_set"
  arg name="name" lit="NODE_ENV"
  arg name="value" lit="production"
  as="_"
end

step id="s3" kind="call"
  fn="os.platform"
  as="platform"
end

step id="s4" kind="call"
  fn="os.arch"
  as="arch"
end
```

---

## Path

File path manipulation utilities. Most functions are pure (no side effects). `path.resolve` requires the `os` effect to access the current working directory.

**Platforms:** deno, browser, node, wasi

| Function | Parameters | Returns | Effect |
|----------|-----------|---------|--------|
| `path.join` | `base: String`, `segment: String` | `String` | _(pure)_ |
| `path.resolve` | `path: String` | `String` | `os` |
| `path.dirname` | `path: String` | `String` | _(pure)_ |
| `path.basename` | `path: String` | `String` | _(pure)_ |
| `path.extname` | `path: String` | `String` | _(pure)_ |
| `path.is_absolute` | `path: String` | `Bool` | _(pure)_ |

**Note:** `path.join` takes exactly 2 segments. For multi-segment joins, chain calls:

```
step id="s1" kind="call"
  fn="path.join"
  arg name="base" lit="/home/user"
  arg name="segment" lit="documents"
  as="p1"
end

step id="s2" kind="call"
  fn="path.join"
  arg name="base" from="p1"
  arg name="segment" lit="report.txt"
  as="full_path"
end

step id="s3" kind="call"
  fn="path.dirname"
  arg name="path" from="full_path"
  as="dir"
end

step id="s4" kind="call"
  fn="path.extname"
  arg name="path" from="full_path"
  as="ext"
end

step id="s5" kind="call"
  fn="path.is_absolute"
  arg name="path" from="full_path"
  as="is_abs"
end
```

### Resolving to Absolute Path

Requires `os` effect since it accesses the current working directory:

```
effects
  effect os
end

body
  step id="s1" kind="call"
    fn="path.resolve"
    arg name="path" lit="./relative/file.txt"
    as="absolute_path"
  end
end
```

---

## Timers

Delay execution. For timeout behavior on concurrent operations, use the `timeout` attribute on `parallel` blocks instead.

**Effect:** `timers`
**Platforms:** deno, browser, node

| Function | Parameters | Returns | Description |
|----------|-----------|---------|-------------|
| `timers.delay` | `ms: Int` | `Unit` | Pause execution for N milliseconds |

```
step id="s1" kind="call"
  fn="console.println"
  arg name="msg" lit="Starting..."
  as="_"
end

step id="s2" kind="call"
  fn="timers.delay"
  arg name="ms" lit=2000
  as="_"
end

step id="s3" kind="call"
  fn="console.println"
  arg name="msg" lit="2 seconds later"
  as="_"
end
```

For repeated delays, use an explicit loop with `timers.delay` inside. For concurrent timeout behavior, use structured concurrency:

```
step id="s1" kind="parallel"
  timeout=5s
  on_timeout="cancel"

  branch id="b1"
    step id="b1.1" kind="call"
      fn="http.get"
      arg name="url" lit="https://slow-api.example.com/data"
      as="response"
    end
  end

  as="result"
end
```

---

## URL

URL parsing and formatting utilities.

**Platforms:** deno, browser, node, wasi

### Types

```
struct name="Url"
  field name="protocol" type="String"
  field name="host" type="String"
  field name="port" type="Int" optional
  field name="pathname" type="String"
  field name="search" type="String" optional
  field name="hash" type="String" optional
end

struct name="UrlError"
  field name="message" type="String"
end
```

### Functions

| Function | Parameters | Returns | Effect |
|----------|-----------|---------|--------|
| `url.parse` | `raw: String` | `Url \| UrlError` | _(pure)_ |
| `url.format` | `url: Url` | `String` | _(pure)_ |
| `url.resolve` | `base: String`, `relative: String` | `String` | _(pure)_ |

```
step id="s1" kind="call"
  fn="url.parse"
  arg name="raw" lit="https://api.example.com:8080/v2/users?active=true#top"
  as="parsed"
end

step id="s2" kind="bind"
  field="host" of="parsed"
  as="api_host"
end

step id="s3" kind="bind"
  field="pathname" of="parsed"
  as="api_path"
end

step id="s4" kind="call"
  fn="url.resolve"
  arg name="base" lit="https://example.com/api/v1/"
  arg name="relative" lit="../v2/users"
  as="resolved"
end

step id="s5" kind="call"
  fn="url.format"
  arg name="url" from="parsed"
  as="url_string"
end
```

---

## Storage: Key-Value

Simple key-value storage that works across all platforms.

**Effect:** `std.storage`
**Platforms:** deno, browser, node, wasi

**Platform backends:**
- Deno: Deno KV (built-in)
- Browser: localStorage
- Node.js: File-based storage (`~/.covenant-storage/kv/`)
- WASI: Preopened directory

| Function | Parameters | Returns | Description |
|----------|-----------|---------|-------------|
| `std.storage.kv.set` | `key: String`, `value: String` | `Unit` | Store a value |
| `std.storage.kv.get` | `key: String` | `String?` | Retrieve a value (none if missing) |
| `std.storage.kv.delete` | `key: String` | `Unit` | Delete a value |
| `std.storage.kv.has` | `key: String` | `Bool` | Check if key exists |
| `std.storage.kv.list` | `prefix: String` | `String[]` | List keys matching prefix |
| `std.storage.kv.clear` | `prefix: String` (optional) | `Unit` | Clear all keys or by prefix |

```
step id="s1" kind="call"
  fn="std.storage.kv.set"
  arg name="key" lit="user:theme"
  arg name="value" lit="dark"
  as="_"
end

step id="s2" kind="call"
  fn="std.storage.kv.get"
  arg name="key" lit="user:theme"
  as="theme"
end

step id="s3" kind="call"
  fn="std.storage.kv.has"
  arg name="key" lit="user:theme"
  as="exists"
end

step id="s4" kind="call"
  fn="std.storage.kv.list"
  arg name="prefix" lit="user:"
  as="user_keys"
end
```

---

## Storage: Document

Document storage with queries, supporting structured data across all platforms.

**Effect:** `std.storage`
**Platforms:** deno, browser, node, wasi

**Platform backends:**
- Deno: Deno KV (structured keys)
- Browser: IndexedDB
- Node.js: SQLite (better-sqlite3)
- WASI: Embedded database

### Types

```
struct name="Document"
  field name="id" type="String"
  field name="data" type="Json"
  field name="created_at" type="DateTime"
  field name="updated_at" type="DateTime"
end

struct name="QueryResult"
  field name="documents" type="Document[]"
  field name="total" type="Int"
end
```

### Functions

| Function | Parameters | Returns | Description |
|----------|-----------|---------|-------------|
| `std.storage.doc.put` | `collection: String`, `id: String`, `data: Json` | `Document` | Insert or update a document |
| `std.storage.doc.get` | `collection: String`, `id: String` | `Document?` | Get document by ID |
| `std.storage.doc.delete` | `collection: String`, `id: String` | `Bool` | Delete document (true if existed) |
| `std.storage.doc.query` | `collection: String`, `filter: Json?`, `order_by: String?`, `order_dir: String?`, `limit: Int?`, `offset: Int?` | `QueryResult` | Query with JSON filter |
| `std.storage.doc.count` | `collection: String`, `filter: Json?` | `Int` | Count matching documents |
| `std.storage.doc.create_index` | `collection: String`, `field: String` | `Unit` | Create index on a field |

```
step id="s1" kind="call"
  fn="std.storage.doc.put"
  arg name="collection" lit="users"
  arg name="id" lit="user123"
  arg name="data" from="user_json"
  as="doc"
end

step id="s2" kind="call"
  fn="std.storage.doc.get"
  arg name="collection" lit="users"
  arg name="id" lit="user123"
  as="user"
end

step id="s3" kind="call"
  fn="std.storage.doc.query"
  arg name="collection" lit="users"
  arg name="filter" lit='{"status": "active"}'
  arg name="order_by" lit="created_at"
  arg name="order_dir" lit="desc"
  arg name="limit" lit=10
  as="results"
end
```

### JSON Filter Syntax

The `filter` parameter accepts a JSON object with these operators:

| Pattern | Example | Description |
|---------|---------|-------------|
| Exact match | `{"status": "active"}` | Field equals value |
| `$gt`, `$gte`, `$lt`, `$lte` | `{"age": {"$gt": 18}}` | Comparison |
| `$and` | `{"$and": [{...}, {...}]}` | Logical AND |
| `$or` | `{"$or": [{...}, {...}]}` | Logical OR |
| `$contains` | `{"tags": {"$contains": "important"}}` | Array contains |
| `$starts_with` | `{"name": {"$starts_with": "Jo"}}` | String prefix |
| `$ends_with` | `{"email": {"$ends_with": "@co.com"}}` | String suffix |
| `$ne` | `{"deleted_at": {"$ne": null}}` | Not equal |
| `null` | `{"deleted_at": null}` | Is null |

### Query Dialect Alternative

For complex queries with compile-time type checking, use `dialect="indexeddb"` instead of the function API:

```
step id="s1" kind="query"
  dialect="indexeddb"
  target="std.storage"
  select all
  from="users"
  where
    and
      equals field="status" lit="active"
      greater field="age" lit=18
    end
  end
  order by="created_at" dir="desc"
  limit=10
  as="active_users"
end
```
