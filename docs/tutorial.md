# Covenant Tutorial

A step-by-step introduction to writing Covenant programs.

---

## Prerequisites

This tutorial assumes you understand:
- Basic programming concepts (functions, variables, types)
- What a compiler does
- Why type systems help catch bugs

No prior Covenant experience is required.

---

## 1. Hello World

Every Covenant program is made of **snippets**. A snippet is a self-contained unit of code with an ID and a kind.

### Your First Snippet

```
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
    fn="println"
    arg name="message" lit="Hello, world!"
    as="_"
  end
end

end
```

### Breaking It Down

**Line 1: Snippet Header**
```
snippet id="main.hello" kind="fn"
```
- `snippet` - Starts a code unit
- `id="main.hello"` - Unique identifier (module.name format)
- `kind="fn"` - This is a function

**Lines 3-5: Effects**
```
effects
  effect console
end
```
- Declares this function needs console access
- Without this, calling `println` would be a compiler error

**Lines 7-11: Signature**
```
signature
  fn name="main"
    returns type="Unit"
  end
end
```
- Function named `main`
- Returns `Unit` (like `void` in other languages)

**Lines 13-18: Body**
```
body
  step id="s1" kind="call"
    fn="println"
    arg name="message" lit="Hello, world!"
    as="_"
  end
end
```
- `step id="s1"` - First step with ID "s1"
- `kind="call"` - This step calls a function
- `fn="println"` - The function to call
- `arg name="message" lit="..."` - Pass a literal string argument
- `as="_"` - Discard the return value

**Line 20: End**
```
end
```
- Closes the snippet (every block ends with `end`)

---

## 2. Pure Functions

Functions without side effects don't need an `effects` section.

### Adding Two Numbers

```
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

### Key Concepts

**Parameters**
```
param name="a" type="Int"
param name="b" type="Int"
```
Two integer parameters named `a` and `b`.

**Compute Step**
```
step id="s1" kind="compute"
  op=add
  input var="a"
  input var="b"
  as="result"
end
```
- `kind="compute"` - Arithmetic/logic operation
- `op=add` - Addition (not `+`!)
- `input var="a"` - First operand from variable `a`
- `input var="b"` - Second operand from variable `b`
- `as="result"` - Bind output to name `result`

**Return Step**
```
step id="s2" kind="return"
  from="result"
  as="_"
end
```
- Return the value bound to `result`

### Why Keywords Instead of Operators?

Covenant uses `add` instead of `+` because:
1. **Unambiguous** - No operator precedence rules needed
2. **Queryable** - Easy to find all additions in the codebase
3. **Consistent** - Same syntax for all operations

---

## 3. Adding Effects

When your function needs to do I/O, declare the effects.

### Reading a File

```
snippet id="files.read_config" kind="fn"

effects
  effect filesystem
end

signature
  fn name="read_config"
    returns union
      type="String"
      type="IoError"
    end
  end
end

body
  step id="s1" kind="call"
    fn="read_file"
    arg name="path" lit="config.txt"
    as="content"
  end

  step id="s2" kind="return"
    from="content"
    as="_"
  end
end

end
```

### Key Concepts

**Effects Declaration**
```
effects
  effect filesystem
end
```
This function needs filesystem access.

**Union Return Type**
```
returns union
  type="String"
  type="IoError"
end
```
Returns either a `String` (success) or `IoError` (failure).

**Effect Propagation**

If function A calls function B, A must declare all of B's effects:
- `read_file` has `effect filesystem`
- So `read_config` must also declare `effect filesystem`

The compiler enforces this automatically.

---

## 4. Error Handling

Covenant makes errors explicit through union types and pattern matching.

### Parsing an Integer

```
snippet id="parse.parse_port" kind="fn"

signature
  fn name="parse_port"
    param name="s" type="String"
    returns union
      type="Int"
      type="ParseError"
    end
  end
end

body
  // Try to parse the string
  step id="s1" kind="call"
    fn="parse_int"
    arg name="s" from="s"
    as="result"
  end

  // Handle the result
  step id="s2" kind="match"
    on="result"
    case variant type="Ok" bindings=("value")
      // Validate range
      step id="s2a" kind="compute"
        op=less
        input var="value"
        input lit=1
        as="too_low"
      end
      step id="s2b" kind="compute"
        op=greater
        input var="value"
        input lit=65535
        as="too_high"
      end
      step id="s2c" kind="compute"
        op=or
        input var="too_low"
        input var="too_high"
        as="invalid"
      end
      step id="s2d" kind="if"
        condition="invalid"
        then
          step id="s2e" kind="return"
            variant type="ParseError"
              field name="message" lit="Port out of range"
            end
            as="_"
          end
        end
        else
          step id="s2f" kind="return"
            from="value"
            as="_"
          end
        end
        as="_"
      end
    end
    case variant type="Err" bindings=("err")
      step id="s2g" kind="return"
        from="err"
        as="_"
      end
    end
    as="_"
  end
end

end
```

### Key Concepts

**Match Step**
```
step id="s2" kind="match"
  on="result"
  case variant type="Ok" bindings=("value")
    // handle success
  end
  case variant type="Err" bindings=("err")
    // handle error
  end
  as="_"
end
```
- `on="result"` - Match on the `result` variable
- `case variant type="Ok"` - Handle the success case
- `bindings=("value")` - Extract the inner value

**If Step**
```
step id="s2d" kind="if"
  condition="invalid"
  then
    // true branch
  end
  else
    // false branch
  end
  as="_"
end
```
- `condition="invalid"` - The boolean variable to check

---

## 5. Database Queries

Covenant has two query styles: structured (for Covenant types) and dialect (for SQL databases).

### Structured Query

```
snippet id="users.find_active" kind="fn"

effects
  effect database
end

signature
  fn name="find_active_users"
    returns collection of="User"
  end
end

body
  step id="s1" kind="query"
    target="app_db"
    select all
    from="users"
    where
      equals field="active" lit=true
    end
    order by="name" dir="asc"
    as="result"
  end

  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
```

### SQL Dialect Query

For complex SQL, use a dialect block:

```
snippet id="analytics.top_customers" kind="fn"

effects
  effect database
end

signature
  fn name="get_top_customers"
    param name="min_orders" type="Int"
    returns collection of="CustomerStats"
  end
end

body
  step id="s1" kind="query"
    dialect="postgres"
    target="app_db"
    body
      SELECT
        c.id, c.name,
        COUNT(o.id) as order_count,
        SUM(o.total) as total_spent
      FROM customers c
      JOIN orders o ON o.customer_id = c.id
      GROUP BY c.id, c.name
      HAVING COUNT(o.id) >= :min_orders
      ORDER BY total_spent DESC
    end
    params
      param name="min_orders" from="min_orders"
    end
    returns collection of="CustomerStats"
    as="result"
  end

  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
```

### Key Concepts

**Structured Query**
- `target="app_db"` - The database to query
- `select all` - Select all fields
- `from="users"` - From the users table
- `where ... end` - Filter conditions
- `order by="name" dir="asc"` - Sort results

**Dialect Query**
- `dialect="postgres"` - Use PostgreSQL syntax
- `body ... end` - Raw SQL (not parsed by Covenant)
- `params ... end` - Parameter bindings
- `returns` - Required type annotation

---

## 6. Composition and Modules

Covenant programs are organized into modules via snippet IDs.

### Module Structure

IDs follow the pattern `module.name`:

```
users.find_by_id      // users module, find_by_id function
users.create          // users module, create function
auth.login            // auth module, login function
auth.validate_token   // auth module, validate_token function
```

### Calling Functions

Call functions by their full ID:

```
step id="s1" kind="call"
  fn="users.find_by_id"
  arg name="id" from="user_id"
  as="user"
end
```

### Effect Composition

When building larger functions, effects compose:

```
snippet id="app.process_order" kind="fn"

// This function calls database and network functions,
// so it needs both effects
effects
  effect database
  effect network
end

signature
  fn name="process_order"
    param name="order_id" type="Int"
    returns union
      type="OrderResult"
      type="ProcessingError"
    end
  end
end

body
  // Get order from database
  step id="s1" kind="call"
    fn="orders.get_by_id"
    arg name="id" from="order_id"
    as="order"
  end

  // Call payment API
  step id="s2" kind="call"
    fn="payments.charge"
    arg name="amount" field="total" of="order"
    as="payment_result"
  end

  // Update order status
  step id="s3" kind="update"
    target="app_db.orders"
    set field="status" lit="paid"
    where
      equals field="id" var="order_id"
    end
    as="_"
  end

  step id="s4" kind="return"
    from="payment_result"
    as="_"
  end
end

end
```

---

## Summary

| Concept | Syntax |
|---------|--------|
| Define function | `snippet id="mod.name" kind="fn"` |
| Declare effects | `effects ... effect X ... end` |
| Function signature | `signature ... fn name="X" ... end` |
| Implementation | `body ... step ... end ... end` |
| Computation | `step kind="compute" op=add input...` |
| Function call | `step kind="call" fn="X" arg...` |
| Return | `step kind="return" from="X"` |
| Conditional | `step kind="if" condition="X" then... else...` |
| Pattern match | `step kind="match" on="X" case...` |
| Query | `step kind="query" target="X" select... from...` |

---

## Next Steps

1. Read [syntax-reference.md](syntax-reference.md) for complete keyword reference
2. Study [patterns.md](patterns.md) for common idioms
3. Browse [examples/](../examples/) for real-world usage
4. Check [reading-guide.md](reading-guide.md) for understanding existing code
