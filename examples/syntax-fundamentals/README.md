# Syntax Fundamentals Examples

Core Covenant syntax patterns: snippet structure, pure vs effectful functions, compute steps, pattern matching, and text operations.

## Examples

| File | Description |
|------|-------------|
| `hello-world.cov` | Minimal effectful function with console output |
| `pure-functions.cov` | Pure functions: arithmetic, recursion, no side effects |
| `effects.cov` | Effect declarations and propagation |
| `pattern-matching.cov` | `kind="match"` for enum and union handling |
| `higher-order.cov` | Higher-order function patterns (map, filter) |
| `regex.cov` | Regex operations via host calls |
| `text-operations.cov` | String manipulation (upper, lower, trim, etc.) |

## Progression

1. **Start with `hello-world.cov`** — Basic snippet structure
2. **Then `pure-functions.cov`** — Compute steps, recursion
3. **Then `effects.cov`** — Adding and combining effects
4. **Explore others** — Pattern matching, text ops, regex

## Key Concepts

### Snippet Structure

Every snippet has `id`, `kind`, and optional sections:

```covenant
snippet id="main.hello" kind="fn"

effects
  effect console
end

signature
  fn name="main"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="console.println"
    arg name="message" lit="Hello, world!"
    as="_"
  end
end

end
```

### Pure vs Effectful

Functions without an `effects` section are pure. The compiler enforces this:

```covenant
// Pure - no effects section
snippet id="math.add" kind="fn"
signature
  fn name="add"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=add
    input var="a"
    input var="b"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
```

### Compute Steps

Arithmetic and logic use keyword operators (not symbols):

```covenant
step id="s1" kind="compute"
  op=add           // not +
  input var="a"
  input var="b"
  as="result"
end

step id="s2" kind="compute"
  op=less_eq       // not <=
  input var="n"
  input lit=1
  as="is_base"
end
```

### Conditional Branching

```covenant
step id="s2" kind="if"
  condition="is_base"
  then
    step id="s2a" kind="return"
      lit=1
      as="_"
    end
  end
  else
    // recursive case
  end
  as="_"
end
```

### Pattern Matching

```covenant
step id="s2" kind="match"
  on="json_value"
  case variant type="Json::String" bindings=("value")
    step id="s2a" kind="return"
      from="value"
      as="_"
    end
  end
  case variant type="Json::Null"
    step id="s2b" kind="return"
      lit="<null>"
      as="_"
    end
  end
  as="_"
end
```

### Text Operations

String functions via the host:

```covenant
step id="s1" kind="call"
  fn="text.upper"
  arg name="s" lit="hello world"
  as="upper_result"
end
```

### Regex

Pattern matching via host V8 RegExp:

```covenant
step id="s1" kind="call"
  fn="std.text.regex_replace_all"
  arg name="pattern" lit="\\d"
  arg name="input" lit="abc123def456"
  arg name="replacement" lit="X"
  as="replaced"
end
```
