# Structured Concurrency Examples

Demonstrates parallel I/O without threads or async/await. `parallel` and `race` are built-in step kinds that require no effect imports.

## Examples

| File | Description |
|------|-------------|
| `structured-concurrency.cov` | Parallel fetches, race patterns, and timeout handling |

## Key Concepts

### Parallel Execution

Execute branches concurrently and wait for all to complete:

```covenant
step id="s1" kind="parallel"
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

### Race Pattern

Execute branches and return the first to complete:

```covenant
step id="s1" kind="race"
  branch id="b1"
    step id="b1.1" kind="call"
      fn="redis.get"
      arg name="key" from="key"
      as="cached"
    end
  end
  branch id="b2"
    step id="b2.1" kind="call"
      fn="postgres.query"
      arg name="sql" lit="SELECT * FROM entries WHERE key = $1"
      arg name="params" from="key"
      as="db_result"
    end
  end
  as="first_result"
end
```

### Error Handling Strategies

- `on_error="fail_fast"` (default) — Cancel other branches on first error
- `on_error="collect_all"` — Wait for all branches, collect successes and failures
- `on_error="ignore_errors"` — Continue with successful results only

### Timeout Handling

```covenant
step id="s1" kind="parallel"
  timeout=5s
  on_timeout="cancel"  // or "return_partial"
  branch id="b1"
    // ...
  end
  as="results"
end
```

### Key Properties

1. **No threads** — Concurrency without thread management
2. **No async/await** — No function coloring
3. **Scoped** — Always wait for results before proceeding
4. **Deterministic** — Results collected in declaration order
5. **Isolated** — Branches share no mutable state
6. **Built-in** — No effect import required for `parallel` or `race`
