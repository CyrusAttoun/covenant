# Covenant Syntax Examples

Minimal examples of every construct. Use this as a quick reference cheat sheet.

---

## Snippet Kinds

### Function
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

### Struct
```
snippet id="types.Point" kind="struct"
  signature
    struct name="Point"
      field name="x" type="Int"
      field name="y" type="Int"
    end
  end
end
```

### Enum
```
snippet id="types.Status" kind="enum"
  signature
    enum name="Status"
      variant name="Pending"
      variant name="Active"
      variant name="Completed"
    end
  end
end
```

### Enum with Data
```
snippet id="types.Result" kind="enum"
  signature
    enum name="Result"
      variant name="Ok"
        field name="value" type="Int"
      end
      variant name="Err"
        field name="message" type="String"
      end
    end
  end
end
```

### Database Binding
```
snippet id="db.main" kind="database"
  metadata
    dialect="postgres"
    connection="env:DATABASE_URL"
  end
  schema
    table name="users"
      field name="id" type="Int" primary_key=true
      field name="email" type="String"
    end
  end
end
```

### External Binding
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
  end
end
```

### Data Node
```
snippet id="docs.intro" kind="data"
  content
    """
    Introduction to the system.
    """
  end
  relations
    rel to="auth.login" type=describes
  end
end
```

---

## Sections

### Effects
```
effects
  effect database
  effect network
  effect console
end
```

### Effects with Parameters
```
effects
  effect filesystem(path="/data")
  effect network(host="api.example.com")
end
```

### Requirements
```
requires
  req id="R-001"
    text "Must validate input"
    priority high
    status approved
  end
end
```

### Signature - Function
```
signature
  fn name="process"
    param name="input" type="String"
    param name="options" type="Options" optional
    returns type="Result"
  end
end
```

### Signature - Union Return
```
signature
  fn name="find"
    param name="id" type="Int"
    returns union
      type="User"
      type="NotFoundError"
      type="DbError"
    end
  end
end
```

### Signature - Collection Return
```
signature
  fn name="list_all"
    returns collection of="User"
  end
end
```

### Tests
```
tests
  test id="T-001" kind="unit" covers="R-001"
    step id="t1" kind="call"
      fn="validate"
      arg name="input" lit="test"
      as="result"
    end
    step id="t2" kind="call"
      fn="assert_true"
      arg name="value" from="result"
      as="_"
    end
  end
end
```

### Metadata
```
metadata
  author="system"
  created="2024-01-15"
  confidence=0.95
  cost_hint=moderate
  latency_hint=fast
  tags=["auth", "core"]
end
```

### Relations
```
relations
  rel to="docs.overview" type=describes
  rel from="auth.login" type=implemented_by
  rel to="types.User" type=depends_on
end
```

---

## Step Kinds

### Compute - Binary
```
step id="s1" kind="compute"
  op=add
  input var="x"
  input var="y"
  as="sum"
end
```

### Compute - Unary
```
step id="s1" kind="compute"
  op=not
  input var="flag"
  as="negated"
end
```

### Call - Function
```
step id="s1" kind="call"
  fn="validate"
  arg name="input" from="data"
  as="is_valid"
end
```

### Call - Tool
```
step id="s1" kind="call"
  tool="http_client"
  arg name="url" from="endpoint"
  as="response"
end
```

### Call - With Error Handler
```
step id="s1" kind="call"
  fn="parse"
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
```

### Bind - From Variable
```
step id="s1" kind="bind"
  from="source"
  as="alias"
end
```

### Bind - Literal
```
step id="s1" kind="bind"
  lit=42
  as="constant"
end
```

### Bind - Mutable
```
step id="s1" kind="bind"
  mut
  lit=0
  as="counter"
end
```

### Return - From Variable
```
step id="s1" kind="return"
  from="result"
  as="_"
end
```

### Return - Literal
```
step id="s1" kind="return"
  lit="success"
  as="_"
end
```

### Return - Struct
```
step id="s1" kind="return"
  struct type="Point"
    field name="x" from="x_val"
    field name="y" lit=0
  end
  as="_"
end
```

### Return - Variant
```
step id="s1" kind="return"
  variant type="Result::Err"
    field name="message" lit="failed"
  end
  as="_"
end
```

### If - Simple
```
step id="s1" kind="if"
  condition="is_valid"
  then
    step id="s1a" kind="return"
      lit="valid"
      as="_"
    end
  end
  as="_"
end
```

### If - With Else
```
step id="s1" kind="if"
  condition="flag"
  then
    step id="s1a" kind="bind"
      lit="yes"
      as="result"
    end
  end
  else
    step id="s1b" kind="bind"
      lit="no"
      as="result"
    end
  end
  as="_"
end
```

### Match - On Variant
```
step id="s1" kind="match"
  on="option"
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

### Match - On Literal
```
step id="s1" kind="match"
  on="code"
  case literal=200
    step id="s1a" kind="bind"
      lit="ok"
      as="status"
    end
  end
  case literal=404
    step id="s1b" kind="bind"
      lit="not found"
      as="status"
    end
  end
  case wildcard
    step id="s1c" kind="bind"
      lit="error"
      as="status"
    end
  end
  as="_"
end
```

### For Loop
```
step id="s1" kind="for"
  var="item" in="items"
  step id="s1a" kind="call"
    fn="process"
    arg name="x" from="item"
    as="processed"
  end
  as="results"
end
```

### Query - Covenant
```
step id="s1" kind="query"
  target="project"
  select all
  from="functions"
  where
    equals field="module" lit="auth"
  end
  as="auth_fns"
end
```

### Query - SQL Dialect
```
step id="s1" kind="query"
  dialect="postgres"
  target="app_db"
  body
    SELECT * FROM users WHERE id = :id
  end
  params
    param name="id" from="user_id"
  end
  returns type="User"
  as="user"
end
```

### Insert
```
step id="s1" kind="insert"
  into="project.nodes"
  set field="name" from="name"
  set field="value" lit=0
  as="new_node"
end
```

### Update
```
step id="s1" kind="update"
  target="project.nodes"
  set field="value" from="new_value"
  where
    equals field="id" var="node_id"
  end
  as="updated"
end
```

### Delete
```
step id="s1" kind="delete"
  from="project.nodes"
  where
    equals field="id" var="node_id"
  end
  as="_"
end
```

### Transaction
```
step id="s1" kind="transaction"
  isolation=serializable
  step id="s1a" kind="insert"
    into="orders"
    set field="total" from="total"
    as="order"
  end
  step id="s1b" kind="update"
    target="inventory"
    set field="quantity" from="new_qty"
    where
      equals field="product_id" var="pid"
    end
    as="_"
  end
  as="tx_result"
end
```

### Traverse
```
step id="s1" kind="traverse"
  target="project"
  from="docs.root"
  follow type=contains
  depth=unbounded
  direction=outgoing
  as="all_children"
end
```

---

## Operators

### Arithmetic
```
op=add input var="a" input var="b"    // a + b
op=sub input var="a" input var="b"    // a - b
op=mul input var="a" input var="b"    // a * b
op=div input var="a" input var="b"    // a / b
op=mod input var="a" input var="b"    // a % b
```

### Comparison
```
op=equals input var="a" input var="b"      // a == b
op=not_equals input var="a" input var="b"  // a != b
op=less input var="a" input var="b"        // a < b
op=greater input var="a" input var="b"     // a > b
op=less_eq input var="a" input var="b"     // a <= b
op=greater_eq input var="a" input var="b"  // a >= b
```

### Logic
```
op=and input var="a" input var="b"   // a && b
op=or input var="a" input var="b"    // a || b
op=not input var="a"                 // !a
```

### String
```
op=concat input var="a" input var="b"    // a + b (string)
op=contains input var="s" input var="sub" // s.contains(sub)
```

### Numeric
```
op=neg input var="x"   // -x
```

---

## Input Sources

### Variable Reference
```
input var="x"
```

### Literal Value
```
input lit=42
input lit="hello"
input lit=true
input lit=none
```

### Field Access
```
input field="name" of="user"
```

---

## Argument Passing

### From Variable
```
arg name="id" from="user_id"
```

### Literal Value
```
arg name="limit" lit=10
```

---

## Where Clause Conditions

### Simple Comparison
```
where
  equals field="status" lit="active"
end
```

### Variable Comparison
```
where
  equals field="id" var="target_id"
end
```

### And Condition
```
where
  and
    equals field="active" lit=true
    greater field="age" lit=18
  end
end
```

### Or Condition
```
where
  or
    equals field="role" lit="admin"
    equals field="role" lit="moderator"
  end
end
```

### Not Condition
```
where
  not equals field="deleted" lit=true
end
```

### Null Check
```
where
  equals field="deleted_at" lit=none
end
```

---

## Type References

### Simple Type
```
type="Int"
type="String"
type="Bool"
```

### Optional Type
```
type="User" optional
type="Int?"
```

### Collection Type
```
collection of="User"
type="Int[]"
```

### Generic Type
```
type="List<User>"
type="Map<String, Int>"
```

### Qualified Type
```
type="module::TypeName"
```

---

## Literals

### Numbers
```
lit=42
lit=-1
lit=3.14
```

### Strings
```
lit="hello"
lit="with \"escapes\""
```

### Booleans
```
lit=true
lit=false
```

### Null
```
lit=none
```

### Arrays
```
lit=[1, 2, 3]
lit=["a", "b", "c"]
```

### Structs
```
lit={"key": "value", "count": 42}
```
