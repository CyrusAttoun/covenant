# Covenant Patterns Catalog

Common patterns and idioms in Covenant, with examples and guidance on when to use each.

---

## Table of Contents

1. [Conditional Execution](#conditional-execution)
2. [Error Handling](#error-handling)
3. [Data Access](#data-access)
4. [Effect Composition](#effect-composition)
5. [Iteration](#iteration)
6. [Code as Data](#code-as-data)

---

## Conditional Execution

### Pattern: Simple Boolean Check

**Problem:** Execute different code paths based on a boolean condition.

**Solution:** Use `if` step with `condition` referencing a boolean binding.

```
step id="s1" kind="compute"
  op=greater
  input var="age"
  input lit=18
  as="is_adult"
end

step id="s2" kind="if"
  condition="is_adult"
  then
    step id="s2a" kind="return"
      lit="Access granted"
      as="_"
    end
  end
  else
    step id="s2b" kind="return"
      lit="Access denied"
      as="_"
    end
  end
  as="_"
end
```

**When to use:** Simple true/false decisions with distinct branches.

---

### Pattern: Multi-Way Branch with Match

**Problem:** Handle multiple variants of an enum or union type.

**Solution:** Use `match` step with `case` arms for each variant.

```
step id="s1" kind="match"
  on="result"
  case variant type="Success" bindings=("value")
    step id="s1a" kind="return"
      from="value"
      as="_"
    end
  end
  case variant type="NotFound"
    step id="s1b" kind="return"
      lit=none
      as="_"
    end
  end
  case variant type="Error" bindings=("msg")
    step id="s1c" kind="call"
      fn="log_error"
      arg name="message" from="msg"
      as="_"
    end
    step id="s1d" kind="return"
      lit=none
      as="_"
    end
  end
  as="_"
end
```

**When to use:**
- Union return types with multiple error cases
- Enum variants with different data
- Optional types (Some/None pattern)

---

### Pattern: Filter with Query Where Clause

**Problem:** Select items from a collection based on conditions.

**Solution:** Use query with `where` clause instead of loop + if.

```
step id="s1" kind="query"
  target="project"
  select all
  from="functions"
  where
    and
      contains field="effects" lit="database"
      equals field="kind" lit="fn"
    end
  end
  as="db_functions"
end
```

**When to use:**
- Filtering collections (prefer query over loop+if)
- Selecting from project AST
- Database queries

**Tradeoff:** Queries are declarative and optimizable; loops are imperative.

---

## Error Handling

### Pattern: Union Return Type

**Problem:** Function can succeed or fail in known ways.

**Solution:** Return a union of success type and error types.

```
signature
  fn name="parse_int"
    param name="s" type="String"
    returns union
      type="Int"
      type="ParseError"
    end
  end
end
```

Caller handles with match:

```
step id="s1" kind="call"
  fn="parse_int"
  arg name="s" from="input"
  as="result"
end

step id="s2" kind="match"
  on="result"
  case variant type="Int" bindings=("value")
    step id="s2a" kind="bind"
      from="value"
      as="parsed"
    end
  end
  case variant type="ParseError" bindings=("err")
    step id="s2b" kind="return"
      from="err"
      as="_"
    end
  end
  as="_"
end
```

**When to use:** Always for functions that can fail. Explicit error types are preferred over exceptions.

---

### Pattern: Inline Error Handler

**Problem:** Handle errors from a call immediately without separate match step.

**Solution:** Use `handle` block inside the call step.

```
step id="s1" kind="call"
  fn="parse_int"
  arg name="s" from="port_str"
  as="port"
  handle
    case type="ParseError"
      step id="s1a" kind="return"
        variant type="ConfigError::InvalidPort"
          field name="value" from="port_str"
        end
        as="_"
      end
    end
  end
end
```

**When to use:**
- Transforming errors at call site
- Early returns on error
- When the error handling is simple (1-2 steps)

**Tradeoff:** More compact but only supports simple error handling. Use match for complex multi-error scenarios.

---

### Pattern: Error Propagation

**Problem:** Pass errors up to caller without handling.

**Solution:** Return the error type in your signature and return it directly.

```
// This function propagates DbError to its caller
signature
  fn name="get_user_name"
    param name="id" type="Int"
    returns union
      type="String"
      type="DbError"
    end
  end
end

body
  step id="s1" kind="call"
    fn="get_user"
    arg name="id" from="id"
    as="user_result"
  end

  step id="s2" kind="match"
    on="user_result"
    case variant type="User" bindings=("user")
      step id="s2a" kind="return"
        field="name" of="user"
        as="_"
      end
    end
    case variant type="DbError" bindings=("err")
      // Propagate error to caller
      step id="s2b" kind="return"
        from="err"
        as="_"
      end
    end
    as="_"
  end
end
```

**When to use:** When you can't handle the error meaningfully at this level.

---

### Pattern: Default on Error

**Problem:** Use a fallback value when an operation fails.

**Solution:** Handle error case with a default binding.

```
step id="s1" kind="call"
  fn="parse_int"
  arg name="s" from="input"
  as="parsed"
  handle
    case type="ParseError"
      step id="s1a" kind="bind"
        lit=0
        as="parsed"
      end
    end
  end
end
// parsed is either the parsed int or 0
```

**When to use:**
- Optional configuration with defaults
- Non-critical parsing
- Graceful degradation

---

## Data Access

### Pattern: Simple Query

**Problem:** Retrieve data from a collection with conditions.

**Solution:** Use Covenant query syntax.

```
step id="s1" kind="query"
  target="project"
  select all
  from="functions"
  where
    equals field="module" lit="auth"
  end
  order by="name" dir="asc"
  limit=10
  as="auth_functions"
end
```

**When to use:** Querying Covenant-managed types (project AST, data nodes).

---

### Pattern: SQL Database Query

**Problem:** Query an external SQL database with full SQL power.

**Solution:** Use dialect query with raw SQL body.

```
step id="s1" kind="query"
  dialect="postgres"
  target="app_db"
  body
    SELECT u.id, u.name, u.email
    FROM users u
    WHERE u.active = true
      AND u.created_at > :cutoff
    ORDER BY u.created_at DESC
    LIMIT :limit
  end
  params
    param name="cutoff" from="cutoff_date"
    param name="limit" from="page_size"
  end
  returns collection of="User"
  as="active_users"
end
```

**When to use:** External databases where you need full SQL (joins, aggregates, CTEs).

---

### Pattern: Null Check

**Problem:** Handle potentially null/missing values.

**Solution:** Use `equals field="x" lit=none` in where clause, or match on Optional.

```
// In query
step id="s1" kind="query"
  target="app_db"
  select all
  from="users"
  where
    equals field="deleted_at" lit=none
  end
  as="active_users"
end

// With Optional type
step id="s2" kind="match"
  on="maybe_user"
  case variant type="Some" bindings=("user")
    // user exists
  end
  case variant type="None"
    // no user
  end
  as="_"
end
```

---

### Pattern: Insert with Generated ID

**Problem:** Create a new record and get the generated ID.

**Solution:** Insert returns the created record with ID.

```
step id="s1" kind="insert"
  into="project.data_nodes"
  set field="name" from="name"
  set field="content" from="content"
  set field="created_at" from="now"
  as="new_node"
end

// new_node contains the record with generated id
step id="s2" kind="bind"
  field="id" of="new_node"
  as="new_id"
end
```

---

### Pattern: Update with Condition

**Problem:** Update records matching a condition.

**Solution:** Use update step with where clause.

```
step id="s1" kind="update"
  target="project.data_nodes"
  set field="status" lit="archived"
  set field="archived_at" from="now"
  where
    and
      equals field="status" lit="draft"
      less field="created_at" var="cutoff"
    end
  end
  as="archived_count"
end
```

---

## Effect Composition

### Pattern: Pure Function

**Problem:** Create a function with no side effects.

**Solution:** Omit the effects section entirely.

```
snippet id="math.double" kind="fn"

// No effects section = pure function

signature
  fn name="double"
    param name="x" type="Int"
    returns type="Int"
  end
end

body
  step id="s1" kind="compute"
    op=mul
    input var="x"
    input lit=2
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
```

**Compiler guarantees:** Pure functions cannot call effectful functions.

---

### Pattern: Effectful Function

**Problem:** Function needs to perform I/O or other effects.

**Solution:** Declare all required effects.

```
snippet id="io.read_file" kind="fn"

effects
  effect filesystem(path="/data")
end

signature
  fn name="read_file"
    param name="path" type="String"
    returns union
      type="String"
      type="IoError"
    end
  end
end

// ...
end
```

---

### Pattern: Multiple Effects

**Problem:** Function needs several different capabilities.

**Solution:** List all effects in the effects section.

```
effects
  effect database
  effect network
  effect console
end
```

**Effect propagation:** If function A calls function B, A must declare all of B's effects. The compiler enforces this transitively.

---

### Pattern: Scoped Effects with Parameters

**Problem:** Constrain an effect to specific resources.

**Solution:** Use effect parameters.

```
effects
  effect filesystem(path="/data/uploads")
  effect network(host="api.example.com")
end
```

This declares access to only specific paths/hosts, not arbitrary filesystem/network access.

---

## Iteration

### Pattern: For Loop

**Problem:** Process each item in a collection.

**Solution:** Use `for` step.

```
step id="s1" kind="for"
  var="item" in="items"

  step id="s1a" kind="call"
    fn="process"
    arg name="x" from="item"
    as="processed"
  end

  step id="s1b" kind="call"
    fn="save"
    arg name="item" from="processed"
    as="_"
  end

  as="results"
end
```

**The `as` binding:** The for loop's output is a collection of the last step's outputs.

---

### Pattern: Transform Collection

**Problem:** Apply a transformation to each item, producing a new collection.

**Solution:** For loop with a single transformation step.

```
step id="s1" kind="for"
  var="user" in="users"

  step id="s1a" kind="bind"
    field="email" of="user"
    as="email"
  end

  as="emails"
end
// emails is now a collection of email strings
```

---

### Pattern: Filter in Loop (avoid this)

**Problem:** Select items matching a condition.

**Anti-pattern:**
```
// Don't do this - use query instead
step id="s1" kind="for"
  var="item" in="items"
  step id="s1a" kind="if"
    condition="matches"
    then
      // collect
    end
  end
  as="filtered"
end
```

**Better:**
```
step id="s1" kind="query"
  target="project"
  select all
  from="items"
  where
    equals field="status" lit="active"
  end
  as="filtered"
end
```

---

## Code as Data

### Pattern: Query Project AST

**Problem:** Find functions, types, or other code elements.

**Solution:** Query with `target="project"`.

```
// Find all functions with database effect
step id="s1" kind="query"
  target="project"
  select all
  from="functions"
  where
    contains field="effects" lit="database"
  end
  as="db_functions"
end

// Find functions without tests
step id="s2" kind="query"
  target="project"
  select all
  from="functions"
  where
    equals field="tests" lit=[]
  end
  as="untested"
end
```

---

### Pattern: Traverse Relations

**Problem:** Follow semantic links between code and documentation.

**Solution:** Use traverse step.

```
// Find all documentation for a function
step id="s1" kind="traverse"
  target="project"
  from="auth.login"
  follow type=described_by
  depth=unbounded
  direction=incoming
  as="docs"
end

// Find all children of a documentation node
step id="s2" kind="traverse"
  target="project"
  from="docs.overview"
  follow type=contains
  depth=3
  as="sections"
end
```

---

### Pattern: Refactoring

**Problem:** Rename a function across the codebase.

**Solution:** Use refactor block for atomic multi-snippet updates.

```
refactor id="rename_validate"
  // Update the function name
  step id="r1" kind="update_snippet"
    target="auth.validate"
    set field="signature.fn.name" lit="validate_credentials"
    as="_"
  end

  // Find all call sites
  step id="r2" kind="query"
    target="project"
    select all
    from="steps"
    where
      and
        equals field="kind" lit="call"
        equals field="fn" lit="validate"
      end
    end
    as="call_sites"
  end

  // Update all call sites
  step id="r3" kind="update_all"
    target="call_sites"
    set field="fn" lit="validate_credentials"
    as="_"
  end
end
```

**Atomicity:** All changes in a refactor block are applied together or not at all.

---

## Pattern Summary

| Pattern | Use When |
|---------|----------|
| `if` | Simple boolean condition |
| `match` | Multiple variants/error cases |
| Query `where` | Filtering collections |
| Union returns | Function can fail |
| `handle` block | Simple inline error handling |
| Match on result | Complex error handling |
| Covenant query | Querying project/managed types |
| Dialect query | External SQL databases |
| Pure function | No I/O needed |
| Effects section | Function needs capabilities |
| `for` loop | Process each item |
| Query filter | Select matching items |
| `traverse` | Follow relation links |
| `refactor` | Multi-snippet atomic changes |
