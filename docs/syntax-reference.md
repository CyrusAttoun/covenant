# Covenant Syntax Reference

A comprehensive reference of all keywords and constructs in the Covenant IR language.

---

## Table of Contents

1. [Structure Keywords](#structure-keywords)
2. [Section Keywords](#section-keywords)
3. [Step Kinds](#step-kinds)
4. [Binary Operators](#binary-operators)
5. [Unary Operators](#unary-operators)
6. [Query Keywords](#query-keywords)
7. [CRUD Keywords](#crud-keywords)
8. [Type Keywords](#type-keywords)
9. [Attribute Keywords](#attribute-keywords)
10. [Control Flow Keywords](#control-flow-keywords)
11. [Literal Values](#literal-values)
12. [Relation Types](#relation-types)

---

## Structure Keywords

These keywords define the top-level structure of Covenant programs.

### `snippet`

Declares a code unit. Every snippet has an ID and kind.

```
snippet id="module.function_name" kind="fn"
  // sections go here
end
```

**Attributes:**
- `id` (required): Unique identifier in `module.name` format
- `kind` (required): Type of snippet

**Valid kinds:**
| Kind | Purpose |
|------|---------|
| `fn` | Function definition |
| `struct` | Data structure |
| `enum` | Enumeration with variants |
| `module` | Module grouping |
| `database` | External database binding |
| `extern` | External tool/library binding |
| `test` | Standalone test snippet |
| `data` | Structured or unstructured content |

### `end`

Closes any block. Every `snippet`, section, step, and nested construct ends with `end`.

```
snippet id="foo" kind="fn"
  body
    step id="s1" kind="return"
      lit=42
      as="_"
    end       // closes step
  end         // closes body
end           // closes snippet
```

### `refactor`

Multi-snippet transformation block with transactional semantics.

```
refactor id="rename_function"
  step id="r1" kind="update_snippet"
    target="module.old_name"
    set field="signature.fn.name" lit="new_name"
    as="_"
  end
end
```

---

## Section Keywords

Sections appear inside snippets in canonical order.

### `effects`

Declares capabilities/side effects this snippet requires.

```
effects
  effect database
  effect network
  effect filesystem(path="/data")
end
```

**Effect parameters:** Some effects accept parameters in parentheses.

### `requires`

First-class requirements linked to implementation and tests.

```
requires
  req id="R-001"
    text "Users must be retrievable by ID"
    priority high
    status approved
  end
end
```

**Requirement fields:**
| Field | Values |
|-------|--------|
| `text` | Description string |
| `priority` | `critical`, `high`, `medium`, `low` |
| `status` | `draft`, `approved`, `implemented`, `tested` |

### `types`

Local type definitions (struct, enum, alias).

```
types
  struct name="Point"
    field name="x" type="Int"
    field name="y" type="Int"
  end
end
```

### `tools`

References to external tool contracts.

```
tools
  tool id="t1" contract="payments.charge@1"
    idempotent=key
    timeout=30s
    retry=(max=3 backoff=exponential)
  end
end
```

**Tool attributes:**
| Attribute | Purpose |
|-----------|---------|
| `idempotent` | Which argument provides idempotency key |
| `timeout` | Maximum execution time (`30s`, `5m`) |
| `retry` | Retry policy with max count and backoff |
| `auth` | Required permission scope |

### `signature`

Public interface: function signature, struct fields, or enum variants.

```
signature
  fn name="get_user"
    param name="id" type="Int"
    returns union
      type="User"
      type="DbError"
    end
  end
end
```

### `body`

Implementation as a sequence of steps in SSA form.

```
body
  step id="s1" kind="compute"
    op=add
    input var="x"
    input lit=1
    as="result"
  end
end
```

### `tests`

Test definitions linked to requirements.

```
tests
  test id="T-001" kind="unit" covers="R-001"
    // test steps
  end
end
```

**Test attributes:**
| Attribute | Values |
|-----------|--------|
| `kind` | `unit`, `property`, `integration`, `golden` |
| `covers` | Requirement ID this test covers |
| `property` | Property description for property-based tests |

### `metadata`

Additional metadata for tooling and AI planning.

```
metadata
  author="system"
  created="2024-01-15"
  confidence=0.95
  cost_hint=moderate
  latency_hint=slow
  tags=["auth", "security"]
end
```

**Common metadata fields:**
| Field | Purpose |
|-------|---------|
| `author` | Creator identifier |
| `created`, `modified` | Timestamps |
| `confidence` | AI generation confidence (0.0-1.0) |
| `cost_hint` | `cheap`, `moderate`, `expensive` |
| `latency_hint` | `fast`, `medium`, `slow` |
| `tags` | Array of string tags |
| `generated_by` | AI model that generated this |
| `human_reviewed` | `true` or `false` |

### `relations`

Semantic relationships to other snippets.

```
relations
  rel to="docs.auth_flow" type=describes
  rel from="auth.login" type=described_by
end
```

### `content`

Data content for `kind="data"` snippets.

```
content
  """
  Multi-line content goes here.
  """
end
```

Or structured fields:

```
content
  display_name "Alice"
  email "alice@example.com"
end
```

### `schema`

Schema definition for data snippets and database bindings.

```
schema
  table name="users"
    field name="id" type="Int" primary_key=true
    field name="email" type="String"
  end
end
```

---

## Step Kinds

Steps are the atomic operations in a `body` section. Every step has:
- `id`: Unique identifier within the snippet
- `kind`: The operation type
- `as`: Output binding name

### `compute`

Arithmetic and logic operations.

```
step id="s1" kind="compute"
  op=add
  input var="x"
  input lit=5
  as="sum"
end
```

### `call`

Function or tool invocation.

```
step id="s1" kind="call"
  fn="validate_email"
  arg name="email" from="user_email"
  as="is_valid"
end

step id="s2" kind="call"
  tool="t1"
  arg name="amount" from="total"
  as="result"
end
```

**Error handling:**
```
step id="s1" kind="call"
  fn="parse_int"
  arg name="s" from="input"
  as="parsed"
  handle
    case type="ParseError"
      step id="s1a" kind="return"
        lit=0
        as="_"
      end
    end
  end
end
```

### `query`

Data retrieval (Covenant types or external databases).

```
// Covenant query
step id="s1" kind="query"
  target="project"
  select all
  from="functions"
  where
    equals field="kind" lit="fn"
  end
  as="fns"
end

// SQL dialect query
step id="s2" kind="query"
  dialect="postgres"
  target="app_db"
  body
    SELECT * FROM users WHERE id = :user_id
  end
  params
    param name="user_id" from="id"
  end
  returns type="User"
  as="user"
end
```

### `bind`

Variable binding (simple assignment).

```
step id="s1" kind="bind"
  from="some_var"
  as="alias"
end

step id="s2" kind="bind"
  lit=42
  as="constant"
end

step id="s3" kind="bind"
  mut
  from="initial_value"
  as="mutable_var"
end
```

### `return`

Function return.

```
step id="s1" kind="return"
  from="result"
  as="_"
end

step id="s2" kind="return"
  lit="default value"
  as="_"
end

step id="s3" kind="return"
  struct type="User"
    field name="id" from="user_id"
    field name="name" lit="Anonymous"
  end
  as="_"
end
```

### `if`

Conditional execution.

```
step id="s1" kind="if"
  condition="is_valid"
  then
    step id="s1a" kind="return"
      lit="valid"
      as="_"
    end
  end
  else
    step id="s1b" kind="return"
      lit="invalid"
      as="_"
    end
  end
  as="_"
end
```

### `match`

Pattern matching on variants.

```
step id="s1" kind="match"
  on="result"
  case variant type="Some" bindings=("value")
    step id="s1a" kind="return"
      from="value"
      as="_"
    end
  end
  case variant type="None"
    step id="s1b" kind="return"
      lit=0
      as="_"
    end
  end
  as="_"
end
```

**Pattern types:**
| Pattern | Example |
|---------|---------|
| `wildcard` | Matches anything |
| `binding="x"` | Binds value to name |
| `literal=42` | Matches specific value |
| `variant type="Some"` | Matches enum variant |
| `struct type="Point"` | Matches struct type |

### `for`

Loop over collection.

```
step id="s1" kind="for"
  var="item" in="items"
  step id="s1a" kind="call"
    fn="process"
    arg name="x" from="item"
    as="_"
  end
  as="processed"
end
```

### `insert`

Insert into Covenant-managed collection.

```
step id="s1" kind="insert"
  into="project.data_nodes"
  set field="name" from="name"
  set field="content" lit="initial"
  as="new_node"
end
```

### `update`

Update Covenant-managed records.

```
step id="s1" kind="update"
  target="project.data_nodes"
  set field="content" from="new_content"
  where
    equals field="id" var="node_id"
  end
  as="updated"
end
```

### `delete`

Delete from Covenant-managed collection.

```
step id="s1" kind="delete"
  from="project.data_nodes"
  where
    equals field="id" var="node_id"
  end
  as="_"
end
```

### `transaction`

Wrap steps in atomic transaction.

```
step id="s1" kind="transaction"
  isolation=serializable
  step id="s1a" kind="insert"
    // ...
  end
  step id="s1b" kind="update"
    // ...
  end
  as="tx_result"
end
```

**Isolation levels:** `read_uncommitted`, `read_committed`, `repeatable_read`, `serializable`

### `traverse`

Graph traversal following relations.

```
step id="s1" kind="traverse"
  target="project"
  from="docs.overview"
  follow type=contains
  depth=unbounded
  direction=outgoing
  as="all_children"
end
```

---

## Binary Operators

Used with `kind="compute"`. All take two inputs.

### Arithmetic

| Operator | Description | Example |
|----------|-------------|---------|
| `add` | Addition | `op=add input var="x" input var="y"` |
| `sub` | Subtraction | `op=sub input var="x" input lit=1` |
| `mul` | Multiplication | `op=mul input var="x" input lit=2` |
| `div` | Division | `op=div input var="x" input var="y"` |
| `mod` | Modulo | `op=mod input var="x" input lit=10` |

### Comparison

| Operator | Description | Example |
|----------|-------------|---------|
| `equals` | Equality | `op=equals input var="x" input var="y"` |
| `not_equals` | Inequality | `op=not_equals input var="x" input lit=0` |
| `less` | Less than | `op=less input var="x" input lit=10` |
| `greater` | Greater than | `op=greater input var="x" input lit=0` |
| `less_eq` | Less or equal | `op=less_eq input var="x" input var="max"` |
| `greater_eq` | Greater or equal | `op=greater_eq input var="x" input lit=1` |

### Logic

| Operator | Description | Example |
|----------|-------------|---------|
| `and` | Logical AND | `op=and input var="a" input var="b"` |
| `or` | Logical OR | `op=or input var="a" input var="b"` |

### String

| Operator | Description | Example |
|----------|-------------|---------|
| `concat` | String concatenation | `op=concat input var="first" input var="last"` |
| `contains` | Substring check | `op=contains input var="haystack" input var="needle"` |

---

## Unary Operators

Take a single input.

| Operator | Description | Example |
|----------|-------------|---------|
| `not` | Logical negation | `op=not input var="flag"` |
| `neg` | Numeric negation | `op=neg input var="x"` |

---

## Query Keywords

Used within `kind="query"` steps.

### Selection

| Keyword | Purpose | Example |
|---------|---------|---------|
| `select all` | Select all fields | `select all` |
| `select field` | Select specific fields | `select field="id" field="name"` |

### Source

| Keyword | Purpose | Example |
|---------|---------|---------|
| `target` | Data source | `target="project"` or `target="app_db"` |
| `from` | Collection/table | `from="functions"` |

### Filtering

| Keyword | Purpose | Example |
|---------|---------|---------|
| `where ... end` | Filter conditions | `where equals field="id" var="x" end` |

### Condition operators

| Operator | Description |
|----------|-------------|
| `equals` | Field equals value |
| `not_equals` | Field not equals value |
| `less`, `greater` | Numeric comparison |
| `less_eq`, `greater_eq` | Inclusive comparison |
| `contains` | Collection/string contains |

### Compound conditions

| Keyword | Purpose |
|---------|---------|
| `and ... end` | All conditions must match |
| `or ... end` | Any condition must match |
| `not` | Negate condition |

### Joins

| Keyword | Purpose | Example |
|---------|---------|---------|
| `join to="X" on ... end` | Explicit field join | `join to="tests" on equals field="fn_id" field="tests.fn_id" end` |
| `follow rel="X"` | Follow declared relation | `follow rel="covered_by"` |

### Ordering and limiting

| Keyword | Purpose | Example |
|---------|---------|---------|
| `order by="X" dir="Y"` | Sort results | `order by="name" dir="asc"` |
| `limit=N` | Maximum results | `limit=10` |
| `offset=N` | Skip first N results | `offset=20` |

### SQL Dialects

| Keyword | Purpose | Example |
|---------|---------|---------|
| `dialect` | Specify SQL flavor | `dialect="postgres"` |
| `body ... end` | Raw SQL content | `body SELECT * FROM users end` |
| `params ... end` | Parameter bindings | `params param name="id" from="user_id" end` |
| `returns` | Return type annotation | `returns type="User"` |

---

## CRUD Keywords

For Covenant-managed types (not SQL databases).

| Keyword | Step Kind | Purpose |
|---------|-----------|---------|
| `insert` | `insert` | Create new record |
| `into` | `insert` | Target collection |
| `update` | `update` | Modify records |
| `delete` | `delete` | Remove records |
| `set field="X"` | `insert`, `update` | Field assignment |

---

## Type Keywords

### Type definitions

| Keyword | Purpose | Example |
|---------|---------|---------|
| `struct` | Data structure | `struct name="Point" ... end` |
| `enum` | Enumeration | `enum name="Status" ... end` |
| `alias` | Type alias | `alias name="UserId" type="Int"` |
| `field` | Struct field | `field name="x" type="Int"` |
| `variant` | Enum variant | `variant name="Success"` |

### Function signatures

| Keyword | Purpose | Example |
|---------|---------|---------|
| `fn` | Function signature | `fn name="foo" ... end` |
| `param` | Parameter | `param name="x" type="Int"` |
| `returns` | Return type | `returns type="Int"` |
| `generic` | Generic type param | `generic name="T"` |

### Return types

| Form | Example |
|------|---------|
| Simple | `returns type="Int"` |
| Optional | `returns type="User" optional` |
| Collection | `returns collection of="User"` |
| Union | `returns union type="User" type="Error" end` |

### Type modifiers

| Modifier | Meaning | Example |
|----------|---------|---------|
| `?` | Optional | `type="User?"` |
| `[]` | Array | `type="Int[]"` |
| `<T>` | Generic | `type="List<User>"` |

---

## Attribute Keywords

Used to specify values in various contexts.

| Keyword | Purpose | Example |
|---------|---------|---------|
| `var="X"` | Reference variable | `input var="x"` |
| `lit=X` | Literal value | `input lit=42` |
| `field="X"` | Reference field | `field="user.name"` |
| `from="X"` | Source binding | `arg name="x" from="value"` |
| `as="X"` | Output binding | `as="result"` |
| `name="X"` | Named element | `param name="id"` |
| `type="X"` | Type annotation | `type="Int"` |
| `id="X"` | Identifier | `id="s1"` |
| `kind="X"` | Kind specifier | `kind="fn"` |

---

## Control Flow Keywords

| Keyword | Context | Purpose |
|---------|---------|---------|
| `if` | Step kind | Conditional |
| `then` | Inside `if` | True branch |
| `else` | Inside `if` | False branch |
| `condition` | Inside `if` | Condition variable |
| `match` | Step kind | Pattern matching |
| `on` | Inside `match` | Value to match |
| `case` | Inside `match` | Match arm |
| `for` | Step kind | Iteration |
| `in` | Inside `for` | Collection to iterate |
| `return` | Step kind | Return from function |

---

## Literal Values

| Type | Examples |
|------|----------|
| Integer | `42`, `-1`, `0` |
| Float | `3.14`, `-0.5` |
| String | `"hello"`, `"with \"escapes\""` |
| Multi-line string | `"""..."""` |
| Boolean | `true`, `false` |
| Null | `none` |
| Array | `[1, 2, 3]`, `["a", "b"]` |
| Struct | `{"key": "value"}` |

---

## Relation Types

Used in `relations` section with `rel to=` or `rel from=`.

### Structural

| Type | Inverse | Purpose |
|------|---------|---------|
| `contains` | `contained_by` | Parent-child |
| `next` | `previous` | Sequence |

### Semantic

| Type | Inverse | Purpose |
|------|---------|---------|
| `describes` | `described_by` | Documentation links |
| `elaborates_on` | - | Expands on topic |
| `contrasts_with` | - | Compares/contrasts |
| `example_of` | - | Provides example |

### Temporal

| Type | Inverse | Purpose |
|------|---------|---------|
| `supersedes` | `precedes` | Version replacement |
| `version_of` | - | Version relationship |

### Causal

| Type | Inverse | Purpose |
|------|---------|---------|
| `causes` | `caused_by` | Cause-effect |
| `motivates` | `enables` | Motivation |

### Reference

| Type | Inverse | Purpose |
|------|---------|---------|
| `related_to` | - | General relation |
| `depends_on` | - | Dependency |
| `implements` | `implemented_by` | Implementation link |

---

## Comments

```
// Single-line comment (ignored by parser)

note "Queryable annotation"

note lang="pseudo"
  """
  Multi-line queryable note
  """
end
```

**Key distinction:**
- `//` comments are discarded during parsing
- `note` keywords become part of the AST and are queryable
