# Using External Bindings

Demonstrates how to call extern functions from user-defined Covenant functions.

## Key Concepts

### Calling Extern Functions

Use `kind="call"` steps with the extern's snippet ID:

```covenant
step id="s1" kind="call"
  fn="http.get"
  arg name="url" from="url"
  as="response"
end
```

### Effect Propagation

When your function calls an extern with effects, your function must declare those effects too:

```covenant
snippet id="fetch.fetch_and_save" kind="fn"

effects
  effect network     // Required because we call http.get
  effect filesystem  // Required because we call fs.write_file
end
```

The compiler enforces this - you cannot call a function with effects you haven't declared.

### Extracting Fields

Use `kind="bind"` to extract fields from struct results:

```covenant
step id="s2" kind="bind"
  field="body" of="response"
  as="body"
end
```

### Union Return Types

Functions that call multiple externs with different error types return a union of all possible errors:

```covenant
returns union
  type="Unit"
  type="HttpError"
  type="IoError"
end
```

## See Also

- `extern-bindings/` - How to declare extern bindings
- `platform-abstraction/` - Cross-platform extern patterns
