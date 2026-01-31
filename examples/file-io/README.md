# File I/O Examples

Demonstrates combining multiple effects in a single function, sequential pipelines, and how effects propagate from called functions.

## Examples

| File | Description |
|------|-------------|
| `file-io.cov` | Read, transform, and write files with console output |

## Key Concepts

### Multiple Effects

Functions can require multiple effects. The compiler ensures all called functions have compatible effects:

```covenant
snippet id="main.file_transform" kind="fn"

effects
  effect filesystem
  effect console
end

signature
  fn name="main"
    returns union
      type="Unit"
      type="IoError"
    end
  end
end
```

### Sequential Pipelines

Build data pipelines by chaining function calls:

```covenant
body
  // Step 1: Read the input file
  step id="s1" kind="call"
    fn="fs.read_file"
    arg name="path" lit="input.txt"
    as="content"
  end

  // Step 2: Transform (pure operation)
  step id="s2" kind="call"
    fn="to_uppercase"
    arg name="s" from="content"
    as="upper"
  end

  // Step 3: Write the result
  step id="s3" kind="call"
    fn="fs.write_file"
    arg name="path" lit="output.txt"
    arg name="content" from="upper"
    as="_"
  end

  // Step 4: Log completion
  step id="s4" kind="call"
    fn="console.println"
    arg name="message" lit="Done!"
    as="_"
  end
end
```

### Effect Propagation

A function's effect set is the union of all effects from functions it calls. Pure functions (like `to_uppercase`) contribute no effects.
