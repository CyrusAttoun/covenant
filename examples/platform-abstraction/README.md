# Platform Abstraction

Demonstrates how to write portable code using abstract extern declarations with platform-specific implementations.

## Key Concepts

### Abstract Declarations

Declare an interface with `kind="extern-abstract"`:

```covenant
snippet id="console.println" kind="extern-abstract"

effects
  effect console
end

signature
  fn name="println"
    param name="msg" type="String"
    returns type="Unit"
  end
end

end
```

### Platform Implementations

Provide platform-specific implementations with `kind="extern-impl"`:

```covenant
snippet id="console.println.browser" kind="extern-impl"
  implements="console.println" platform="browser"

metadata
  contract="@browser/console.log"
end

end
```

Supported platforms: `browser`, `node`, `wasi`

### Compiler Selection

User code calls the abstract ID - the compiler selects the appropriate implementation based on the compilation target:

```covenant
step id="s1" kind="call"
  fn="console.println"    // Always use abstract ID
  arg name="msg" from="name"
  as="_"
end
```

Compile with: `covenant compile --target=browser ...`

## When to Use

- Console/logging across platforms
- File system access (Node.js vs WASI)
- HTTP clients (fetch vs Node http)
- Any capability that differs by runtime

## See Also

- `using-bindings/` - Basic extern usage patterns
- `extern-bindings/` - Extern declaration syntax
