# Error Handling Examples

Demonstrates Covenant's approach to error handling using union return types, enum error variants, and pattern matching.

## Examples

| File | Description |
|------|-------------|
| `error-handling.cov` | Config parsing with validation and error types |

## Key Concepts

### Union Return Types

Functions that can fail return a union of success and error types:

```covenant
signature
  fn name="parse_config"
    param name="input" type="String"
    returns union
      type="Config"
      type="ParseError"
    end
  end
end
```

### Error Enum Variants

Define structured error types with `kind="enum"`:

```covenant
snippet id="config.ParseError" kind="enum"

signature
  enum name="ParseError"
    variant name="InvalidFormat"
      field name="message" type="String"
    end
    variant name="MissingField"
      field name="name" type="String"
    end
    variant name="OutOfRange"
      field name="field" type="String"
      field name="value" type="Int"
    end
  end
end

end
```

### Pattern Matching

Handle different cases with `kind="match"`:

```covenant
step id="s2" kind="match"
  on="host_result"
  case variant type="Some" bindings=("value")
    step id="s2a" kind="bind"
      from="value"
      as="host"
    end
  end
  case variant type="None"
    step id="s2b" kind="return"
      variant type="ParseError::MissingField"
        field name="name" lit="host"
      end
      as="_"
    end
  end
  as="_"
end
```

### Handle Blocks

Transform errors at call sites using `handle`:

```covenant
step id="s5" kind="call"
  fn="parse_int"
  arg name="s" from="port_str"
  as="port_parse_result"
  handle
    case type="ParseIntError"
      step id="s5a" kind="return"
        variant type="ParseError::InvalidFormat"
          field name="message" lit="port must be a number"
        end
        as="_"
      end
    end
  end
end
```
