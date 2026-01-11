# Covenant Language Specification (Condensed for LLM Generation)

**Target token count: 2,500-3,000 tokens**

## Core Philosophy

Machine-first IR for LLM generation. Human-readable views are derived. IR is source of truth.

**Design Goals:**
1. **Deterministic generation** — one canonical way to write everything
2. **Queryable structure** — every symbol has an ID, relationships explicit
3. **Explicit effects** — capabilities declared per-snippet
4. **Requirements linkage** — specs/tests are first-class nodes
5. **Token stability** — small grammar (~50 keywords), predictable sequences

## Critical Rules (MUST FOLLOW)

1. **No operators** — Use keywords: `add`, `equals`, `and`, `or`, `not`
2. **No expression nesting** — One operation per step (SSA form)
3. **Canonical ordering** — Fields appear in fixed order within blocks
4. **Every node has ID** — Use format: `module.name` or `M-001`
5. **Double quotes only** — Strings use `"text"` or `"""multi-line"""`
6. **Comments** — Use `//` (ignored) or `note` keyword (queryable)

## Snippet Structure Template

```
snippet id="<module>.<name>" kind="<kind>"

effects
  effect <capability>
end

requires
  req id="<R-001>"
    text "<requirement>"
    priority <critical|high|medium|low>
  end
end

signature
  fn name="<name>"
    param name="<name>" type="<Type>"
    returns <type_spec>
    end
  end
end

body
  step id="<s1>" kind="<kind>" <step_body> as="<result>" end
  // ... more steps
end

tests
  test id="<T-001>" kind="<unit|property|integration>" covers="<R-001>"
    // test steps
  end
end

end
```

## Snippet Kinds

- `fn` — Function
- `struct` — Struct type
- `enum` — Enum type
- `module` — Module/namespace
- `database` — Database binding
- `extern` — External tool binding
- `test` — Test suite
- `data` — Data/documentation node

## Type System

### Primitive Types
`Int`, `Float`, `String`, `Bool`

### Type References
```
type="User"                    // Simple type
type="User" optional           // Optional (nullable)
type="List<String>"           // Generic
type="Result<User, Error>"    // Multiple type params
```

### Returns Declarations
```
returns type="User" end                          // Single type
returns type="User" optional end                 // Optional single
returns collection of="User" end                 // Collection
returns union                                    // Union type
  type="User"
  type="DbError"
end
```

## Effects System

**Common Effects:**
- `database` — Database access
- `network` — Network I/O
- `filesystem` — File operations
- `stdio` — Console I/O
- `random` — Non-deterministic RNG
- `time` — Clock access

**Declaration:**
```
effects
  effect database
  effect network
end
```

**Rule:** Effects are transitive. If `fn_a` calls `fn_b` with `effect database`, then `fn_a` must also declare `effect database`.

## Step Types

### Compute
```
step id="s1" kind="compute"
  op=add
  input var="x"
  input var="y"
  as="sum"
end

step id="s2" kind="compute"
  op=equals
  input var="a"
  input lit="hello"
  as="is_hello"
end
```

**Binary ops:** `add`, `sub`, `mul`, `div`, `mod`, `equals`, `not_equals`, `less`, `greater`, `less_eq`, `greater_eq`, `and`, `or`, `concat`, `contains`

**Unary ops:** `not`, `neg`

### Call
```
step id="s3" kind="call"
  fn="user.get_by_id"
  arg name="id" from="user_id"
  as="user_result"
end

step id="s4" kind="call"
  fn="user.get_by_id"
  arg name="id" from="user_id"
  handle
    case type="DbError"
      step id="s4a" kind="return" from="default_user" as="_" end
    end
  end
  as="user_result"
end
```

### Query (Covenant Dialect)

Simple queries for Covenant types (project AST, structs, collections):

```
step id="s5" kind="query"
  target="project"
  select all
  from="functions"
  where
    contains field="effects" lit="database"
  end
  order by="name" dir="asc"
  limit=10
  as="db_functions"
end
```

**Select:** `select all` or `select field="name" field="id"`
**From:** `from="<collection>"`
**Where conditions:** `equals`, `not_equals`, `less`, `greater`, `contains`
**Compound:** `and ... end`, `or ... end`, `not <condition>`

### Query (SQL Dialects)

For external databases (postgres, mysql, sqlserver, sqlite):

```
step id="s6" kind="query"
  dialect="postgres"
  target="app_db"
  body
    SELECT u.id, u.name, COUNT(o.id) as order_count
    FROM users u
    LEFT JOIN orders o ON o.user_id = u.id
    WHERE u.created_at > :cutoff
    GROUP BY u.id, u.name
  end
  params
    param name="cutoff" from="cutoff_date"
  end
  returns collection of="UserOrderStats"
  as="high_volume_users"
end
```

**Key points:**
- `dialect` required (postgres, sqlserver, mysql, sqlite)
- `body ... end` contains raw SQL (not parsed by Covenant)
- `params` declares parameter bindings
- `returns` type annotation required

### Insert (Covenant types only)
```
step id="s7" kind="insert"
  into="project.data_nodes"
  set field="name" from="name"
  set field="content" from="content"
  as="new_node"
end
```

### Update (Covenant types only)
```
step id="s8" kind="update"
  target="project.data_nodes"
  set field="content" from="updated_content"
  where
    equals field="id" var="node_id"
  end
  as="updated"
end
```

### Delete (Covenant types only)
```
step id="s9" kind="delete"
  from="project.data_nodes"
  where
    equals field="id" var="node_id"
  end
  as="_"
end
```

### Bind
```
step id="s10" kind="bind"
  from="user_id"
  as="id_copy"
end

step id="s11" kind="bind"
  lit="default_value"
  as="default"
end

step id="s12" kind="bind"
  mut
  from="counter"
  as="mutable_counter"
end
```

### Return
```
step id="s13" kind="return"
  from="result"
  as="_"
end

step id="s14" kind="return"
  lit=42
  as="_"
end

step id="s15" kind="return"
  variant type="Ok<User>"
    field name="value" from="user"
  end
  as="_"
end
```

### If
```
step id="s16" kind="if"
  condition="is_valid"
  then
    step id="s16a" kind="return" from="success" as="_" end
  end
  else
    step id="s16b" kind="return" from="failure" as="_" end
  end
  as="result"
end
```

### Match
```
step id="s17" kind="match"
  on="result"
  case variant type="Ok" bindings=("value")
    step id="s17a" kind="return" from="value" as="_" end
  end
  case variant type="Err" bindings=("error")
    step id="s17b" kind="return" from="default" as="_" end
  end
  as="final"
end
```

**Pattern types:**
- `wildcard` — Matches anything
- `binding="x"` — Bind to variable
- `literal=<value>` — Match literal
- `variant type="<Type>" bindings=("<var1>", "<var2>")` — Destructure variant
- `struct type="<Type>" fields=("<field1>", "<field2>")` — Destructure struct

### For
```
step id="s18" kind="for"
  var="item"
  in="items"
  step id="s18a" kind="call"
    fn="process_item"
    arg name="item" from="item"
    as="processed"
  end
  as="results"
end
```

### Transaction
```
step id="s19" kind="transaction"
  isolation=serializable
  step id="s19a" kind="insert" into="accounts" set field="balance" from="initial" as="acc" end
  step id="s19b" kind="update" target="ledger" set field="total" from="new_total" as="_" end
  as="tx_result"
end
```

## Database Bindings

```
snippet id="db.app_db" kind="database"

metadata
  type="database"
  dialect="postgres"
  connection="env:APP_DB_URL"
end

schema
  table name="users"
    field name="id" type="Int" primary_key=true
    field name="email" type="String"
    field name="created_at" type="String"
  end

  table name="orders"
    field name="id" type="Int" primary_key=true
    field name="user_id" type="Int"
    field name="total" type="Float"
  end
end

end
```

## External Tool Bindings

```
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

## Null Handling

Use `none` literal for null/absent values:

```
step id="s20" kind="query"
  target="project"
  select all
  from="users"
  where
    equals field="deleted_at" lit=none    // Check for null
  end
  as="active_users"
end
```

## Common Patterns

### CRUD with Error Handling
```
snippet id="user.create" kind="fn"

effects
  effect database
end

signature
  fn name="create_user"
    param name="email" type="String"
    returns union
      type="User"
      type="DbError"
    end
  end
end

body
  step id="s1" kind="insert"
    into="project.users"
    set field="email" from="email"
    set field="created_at" from="now"
    as="user"
  end

  step id="s2" kind="return"
    variant type="Ok<User>"
      field name="value" from="user"
    end
    as="_"
  end
end

end
```

### Pattern Matching
```
step id="s1" kind="call"
  fn="user.get_by_id"
  arg name="id" from="user_id"
  as="result"
end

step id="s2" kind="match"
  on="result"
  case variant type="Ok" bindings=("user")
    step id="s2a" kind="return" from="user" as="_" end
  end
  case variant type="Err" bindings=("error")
    step id="s2b" kind="return"
      variant type="Err<NotFound>"
        field name="message" lit="User not found"
      end
      as="_"
    end
  end
  as="final"
end
```

### Iterating Collections
```
step id="s1" kind="for"
  var="user"
  in="users"
  step id="s1a" kind="compute"
    op=add
    input var="total"
    input field="balance" of="user"
    as="new_total"
  end
  as="totals"
end
```

## Common Generation Errors

### 1. Effect Transitivity
**Error:** Calling function with effects without declaring them
```
// WRONG
snippet id="app.main" kind="fn"
// Missing: effects section
signature
  fn name="main"
    returns type="Unit" end
  end
end
body
  step id="s1" kind="call"
    fn="user.get_by_id"  // This has effect database
    arg name="id" lit=1
    as="user"
  end
end
end

// CORRECT
snippet id="app.main" kind="fn"
effects
  effect database  // Declare transitive effect
end
// ... rest
```

### 2. Pattern Match Exhaustiveness
**Error:** Missing match cases for union variants
```
// WRONG - Missing Err case
step id="s1" kind="match"
  on="result"  // Result is union of Ok, Err
  case variant type="Ok" bindings=("value")
    step id="s1a" kind="return" from="value" as="_" end
  end
  as="final"
end

// CORRECT
step id="s1" kind="match"
  on="result"
  case variant type="Ok" bindings=("value")
    step id="s1a" kind="return" from="value" as="_" end
  end
  case variant type="Err" bindings=("error")
    step id="s1b" kind="return" from="default" as="_" end
  end
  as="final"
end
```

### 3. Canonical Ordering
**Error:** Sections in wrong order
```
// WRONG
snippet id="foo" kind="fn"
body
  // ...
end
signature  // Signature must come before body
  // ...
end
end

// CORRECT - Canonical order:
// 1. effects
// 2. requires
// 3. types
// 4. tools
// 5. signature
// 6. body
// 7. tests
// 8. metadata
```

### 4. SSA Violations
**Error:** Reusing variable names
```
// WRONG
step id="s1" kind="bind" from="x" as="result" end
step id="s2" kind="bind" from="y" as="result" end  // Duplicate name!

// CORRECT
step id="s1" kind="bind" from="x" as="result1" end
step id="s2" kind="bind" from="y" as="result2" end
```

## Generation Checklist

Before returning generated code, verify:

- [ ] All effects transitively declared
- [ ] All match cases exhaustive
- [ ] Sections in canonical order (effects → requires → types → tools → signature → body → tests → metadata)
- [ ] No duplicate variable names (SSA form)
- [ ] All step IDs unique within snippet
- [ ] No operators used (use keywords: `add`, `equals`, etc.)
- [ ] All strings use double quotes
- [ ] Every `snippet`, `step`, `req`, `test` has unique ID
- [ ] Return types match function signature
- [ ] SQL dialect queries have `returns` annotation
- [ ] CRUD operations (insert/update/delete) only target Covenant types

## Grammar Quick Reference

**Keywords (~50 total):**
snippet, end, effects, requires, types, tools, signature, body, tests, metadata, fn, struct, enum, module, database, extern, test, data, effect, req, priority, text, step, kind, as, op, input, var, lit, from, of, into, set, param, returns, union, collection, type, optional, add, sub, mul, div, mod, equals, not_equals, less, greater, and, or, not, if, then, else, match, case, for, in, return, bind, mut, query, target, select, all, from, join, where, order, by, limit, insert, update, delete, transaction, handle, variant, field, wildcard, binding, literal, true, false, none

**No operators. No semicolons. No expression nesting.**

---

**Token count target: ~2,800 tokens**
