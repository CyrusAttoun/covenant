# External Bindings Examples

Demonstrates how to wrap JavaScript/npm libraries using `kind="extern"` snippets with Covenant's type and effect system.

## Examples

| File | Description |
|------|-------------|
| `extern-bindings.cov` | FFI bindings for HTTP, filesystem, crypto, console, and database |

## Bindings Defined

| Binding | Effect | Description |
|---------|--------|-------------|
| `http.get` | network | HTTP GET request |
| `http.post` | network | HTTP POST request |
| `fs.read_file` | filesystem | Read file contents |
| `fs.write_file` | filesystem | Write file contents |
| `crypto.sha256` | (pure) | SHA-256 hash |
| `crypto.random_bytes` | random | Generate random bytes |
| `console.println` | console | Print to stdout |
| `db.query` | database | Execute SQL query |
| `db.connect` | database | Connect to database |

## Key Concepts

### Extern Snippet Structure

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
  contract="axios.get@1"
  cost_hint=moderate
  latency_hint=slow
end

end
```

### Metadata Section

The `metadata` section provides hints for the runtime and tooling:

- `contract` — The npm package and function being wrapped (e.g., `axios.get@1`)
- `cost_hint` — Computational cost: `cheap`, `moderate`, `expensive`
- `latency_hint` — Expected latency: `fast`, `slow`, `variable`

### Effect Declarations

Effects on extern bindings propagate to callers:

```covenant
snippet id="crypto.random_bytes" kind="extern"

effects
  effect random
end

signature
  fn name="random_bytes"
    param name="length" type="Int"
    returns type="Bytes"
  end
end

end
```

### Pure Externs

Some externs have no effects (pure computation):

```covenant
snippet id="crypto.sha256" kind="extern"

// No effects section = pure function

signature
  fn name="sha256"
    param name="input" type="String"
    returns type="String"
  end
end

end
```
