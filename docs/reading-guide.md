# Reading Covenant: A Guide for Humans

Covenant is designed as a machine-first intermediate representation. This guide explains how to read and understand Covenant syntax as a human developer.

---

## Table of Contents

1. [Why the Verbosity?](#why-the-verbosity)
2. [The Reading Strategy](#the-reading-strategy)
3. [Understanding IDs](#understanding-ids)
4. [SSA Form Explained](#ssa-form-explained)
5. [Attribute Syntax](#attribute-syntax)
6. [Block Structure](#block-structure)
7. [Why No Operators?](#why-no-operators)
8. [Worked Example](#worked-example)

---

## Why the Verbosity?

Covenant looks verbose compared to traditional languages. This is intentional:

**Traditional code optimizes for:**
- Minimal keystrokes
- Compact visual appearance
- Human pattern recognition

**Covenant optimizes for:**
- Deterministic generation (one way to write everything)
- Queryable structure (every node is addressable)
- Explicit effects (no hidden side effects)
- Token stability (predictable sequences for AI)

The verbosity is a feature, not a bug. It makes code:
- **Unambiguous** — No parsing context needed to understand meaning
- **Queryable** — Find all functions with database effects in one query
- **Traceable** — Every step has an ID for debugging and profiling
- **Verifiable** — Effects are declared, not inferred

---

## The Reading Strategy

### Top-to-Bottom, Always

Unlike languages with operator precedence or complex scoping rules, Covenant reads strictly top-to-bottom:

1. **Snippet header** — What is this? (`id`, `kind`)
2. **Effects** — What capabilities does it need?
3. **Requirements** — What specs does it implement?
4. **Signature** — What's the interface?
5. **Body** — What does it do, step by step?
6. **Tests** — How is it verified?

You never need to jump around to understand the code.

### The "What-How-Why" Pattern

Each snippet answers three questions:

| Section | Question |
|---------|----------|
| `signature` | **What** does this do? |
| `body` | **How** does it work? |
| `requires`, `tests` | **Why** does it exist? |

### Read Sections Independently

Each section is self-contained. You can understand `signature` without reading `body`. You can scan `effects` without understanding implementation details.

---

## Understanding IDs

Every construct in Covenant has an ID. This enables precise references.

### Snippet IDs

Format: `module.name`

```
snippet id="auth.login" kind="fn"
```

This is the function `login` in module `auth`.

### Step IDs

Format: `sN` (sequential within snippet)

```
step id="s1" kind="compute"
  // first step
end

step id="s2" kind="call"
  // second step
end
```

### Why IDs Matter

IDs enable:
- **Queries** — "Find all steps that call `validate`"
- **References** — Tests can reference specific steps
- **Debugging** — Error messages point to `s5` not "line 42"
- **Profiling** — Performance data keyed by step ID

---

## SSA Form Explained

SSA (Static Single Assignment) means:
- Each variable is assigned exactly once
- No mutation after assignment
- Every operation creates a new binding

### Traditional vs SSA

**Traditional:**
```javascript
let x = 5;
x = x + 1;  // mutation
x = x * 2;  // more mutation
return x;
```

**Covenant (SSA):**
```
step id="s1" kind="bind"
  lit=5
  as="x"
end

step id="s2" kind="compute"
  op=add
  input var="x"
  input lit=1
  as="x_plus_one"      // new binding, doesn't mutate x
end

step id="s3" kind="compute"
  op=mul
  input var="x_plus_one"
  input lit=2
  as="result"          // another new binding
end

step id="s4" kind="return"
  from="result"
  as="_"
end
```

### Why SSA?

1. **Traceability** — Each intermediate value has a name
2. **Debugging** — Inspect any step's output by name
3. **Parallelization** — Compiler can analyze data dependencies
4. **No Side Effects** — Pure transformations are explicit

### Reading SSA Code

Follow the `as="..."` trail:
1. Look at `as="result"` — what produces this?
2. Look at `input var="x"` — where was `x` defined?
3. Trace the data flow step by step

---

## Attribute Syntax

Covenant uses explicit attributes instead of positional arguments.

### The Four Value Sources

| Keyword | Meaning | Example |
|---------|---------|---------|
| `var="x"` | Reference a binding | `input var="user_id"` |
| `lit=X` | Literal value | `input lit=42` |
| `field="x"` | Access a field | `input field="user" of="name"` |
| `from="x"` | Source binding | `arg name="id" from="user_id"` |

### `var` vs `from`

Both reference bindings, but in different contexts:

- `var` — Used in `input` (computation inputs)
- `from` — Used in `arg`, `set`, `return` (data flow)

```
step id="s1" kind="compute"
  op=add
  input var="x"           // var for input
  input lit=1
  as="incremented"
end

step id="s2" kind="call"
  fn="process"
  arg name="value" from="incremented"  // from for argument
  as="result"
end
```

### `as` — The Output Binding

Every step ends with `as="name"` declaring its output:

```
step id="s1" kind="compute"
  op=add
  input var="x"
  input var="y"
  as="sum"               // result is bound to "sum"
end
```

Use `as="_"` when the output is unused (like `return` steps).

---

## Block Structure

Everything in Covenant is a block with explicit `end`.

### Nesting is Visible

```
snippet id="foo" kind="fn"
  body
    step id="s1" kind="if"
      condition="flag"
      then
        step id="s1a" kind="return"
          lit="yes"
          as="_"
        end                           // closes step s1a
      end                             // closes then
      as="_"
    end                               // closes step s1
  end                                 // closes body
end                                   // closes snippet
```

### Why Explicit `end`?

1. **No ambiguity** — Unlike indentation-based or brace-based syntax
2. **Easy parsing** — AI doesn't need to track indentation
3. **Clear scope** — You always know what closes what
4. **Grep-friendly** — Count `end` keywords to verify structure

---

## Why No Operators?

Covenant uses keywords instead of symbols:

| Traditional | Covenant |
|-------------|----------|
| `x + y` | `op=add input var="x" input var="y"` |
| `x == y` | `op=equals input var="x" input var="y"` |
| `x && y` | `op=and input var="x" input var="y"` |
| `!x` | `op=not input var="x"` |

### Benefits

1. **No precedence rules** — `a + b * c` is ambiguous; SSA isn't
2. **Queryable** — "Find all equality checks" is trivial
3. **Explicit order** — One operation per step, explicit data flow
4. **Token stability** — Symbols vary across languages; `add` is universal

### Reading Compute Steps

Pattern: `op=OPERATOR input SOURCE input SOURCE as="OUTPUT"`

```
step id="s1" kind="compute"
  op=add                    // operation: addition
  input var="price"         // first operand
  input var="tax"           // second operand
  as="total"                // result binding
end
```

Read as: "total = price + tax"

---

## Worked Example

Let's read through a complete function that parses a configuration string.

### The Code

```
snippet id="config.parse" kind="fn"

signature
  fn name="parse_config"
    param name="input" type="String"
    returns union
      type="Config"
      type="ParseError"
    end
  end
end

body
  // Find host line
  step id="s1" kind="call"
    fn="find_config_line"
    arg name="input" from="input"
    arg name="prefix" lit="host="
    as="host_result"
  end

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

  // Validate port range
  step id="s7" kind="compute"
    op=less
    input var="port"
    input lit=1
    as="too_low"
  end

  step id="s8" kind="compute"
    op=greater
    input var="port"
    input lit=65535
    as="too_high"
  end

  step id="s9" kind="compute"
    op=or
    input var="too_low"
    input var="too_high"
    as="out_of_range"
  end

  step id="s10" kind="if"
    condition="out_of_range"
    then
      step id="s10a" kind="return"
        variant type="ParseError::OutOfRange"
          field name="field" lit="port"
          field name="value" from="port"
        end
        as="_"
      end
    end
    as="_"
  end

  // Return config
  step id="s13" kind="return"
    struct type="Config"
      field name="host" from="host"
      field name="port" from="port"
      field name="debug" from="debug"
    end
    as="_"
  end
end

end
```

### Reading It Step by Step

**1. Snippet Header**
```
snippet id="config.parse" kind="fn"
```
This is a function called `parse` in the `config` module.

**2. Signature**
```
fn name="parse_config"
  param name="input" type="String"
  returns union
    type="Config"
    type="ParseError"
  end
```
- Takes one parameter: `input` (a String)
- Returns either `Config` or `ParseError` (union type)

**3. Step s1 — Find host line**
```
step id="s1" kind="call"
  fn="find_config_line"
  arg name="input" from="input"
  arg name="prefix" lit="host="
  as="host_result"
end
```
- Calls `find_config_line` function
- Passes `input` parameter and literal `"host="`
- Result stored in `host_result`

**4. Step s2 — Handle result**
```
step id="s2" kind="match"
  on="host_result"
  case variant type="Some" bindings=("value")
    ...bind value to "host"...
  end
  case variant type="None"
    ...return MissingField error...
  end
```
- Pattern match on `host_result`
- If `Some`, extract the value and bind to `host`
- If `None`, return an error

**5. Steps s7-s9 — Port validation**
```
s7: too_low = (port < 1)
s8: too_high = (port > 65535)
s9: out_of_range = (too_low OR too_high)
```
Three separate computations building up a boolean.

**6. Step s10 — Conditional error**
```
step id="s10" kind="if"
  condition="out_of_range"
  then
    ...return OutOfRange error...
  end
```
If port is out of range, return an error.

**7. Step s13 — Success return**
```
step id="s13" kind="return"
  struct type="Config"
    field name="host" from="host"
    field name="port" from="port"
    field name="debug" from="debug"
  end
```
Construct and return a `Config` struct with all parsed values.

### Key Observations

1. **Data flow is explicit** — Each binding (`host`, `port`, `debug`) is created once and used later
2. **Error paths are clear** — Each error condition has its own return step
3. **No hidden control flow** — You can trace every path through the function
4. **Queryable structure** — "Find all steps that return ParseError" is one query

---

## Summary

| Concept | Traditional | Covenant |
|---------|-------------|----------|
| Assignment | `x = y + 1` | `step ... op=add ... as="x"` |
| Condition | `if (x > 0)` | `step kind="if" condition="is_positive"` |
| Return | `return value` | `step kind="return" from="value"` |
| Function call | `foo(x, y)` | `step kind="call" fn="foo" arg name="a" from="x"` |

**Reading tips:**
1. Start with `signature` to understand the interface
2. Scan `effects` to know what capabilities are used
3. Follow the `as="..."` trail through `body`
4. Check `tests` to see expected behavior

The verbosity buys you:
- Unambiguous semantics
- Full queryability
- Explicit data flow
- Machine-verifiable properties
