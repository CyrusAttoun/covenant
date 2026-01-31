# Cross-Platform Storage Examples

Demonstrates `std.storage.kv` (key-value) and `std.storage.doc` (document) modules. The same code works identically on browser, Node.js, Deno, and WASI.

## Examples

| File | Description |
|------|-------------|
| `cross-platform-storage.cov` | Key-value and document storage patterns |

## Platform Backends

| Platform | Key-Value | Document |
|----------|-----------|----------|
| Deno | Deno KV | Deno KV |
| Browser | localStorage | IndexedDB |
| Node.js | Files | SQLite |
| WASI | Preopened dir | Embedded DB |

## Key Concepts

### Key-Value Storage

Simple string key-value pairs with `effect std.storage`:

```covenant
effects
  effect std.storage
end

body
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
end
```

### Document Storage with Query Dialect

Query documents using `target="std.storage"`:

```covenant
step id="s1" kind="query"
  target="std.storage"
  select all
  from="users"
  where
    and
      equals field="status" lit="active"
      greater field="age" var="min_age"
    end
  end
  order by="created_at" dir="desc"
  limit=50
  as="users"
end
```

### Document Storage with Function API

Use `std.storage.doc.*` functions for CRUD operations:

```covenant
step id="s1" kind="call"
  fn="std.storage.doc.put"
  arg name="collection" lit="users"
  arg name="id" from="user_id"
  arg name="data" from="user_data"
  as="doc"
end
```

### Creating Indexes

Optimize queries by creating indexes on frequently queried fields:

```covenant
step id="s1" kind="call"
  fn="std.storage.doc.create_index"
  arg name="collection" lit="users"
  arg name="field" lit="status"
  as="_"
end
```
