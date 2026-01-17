# Covenant Error Codes

Comprehensive catalog of all compiler error codes with examples and auto-fix strategies.

---

## Table of Contents

- [Parse Errors (E-PARSE-xxx)](#parse-errors)
- [Type Errors (E-TYPE-xxx)](#type-errors)
- [Effect Errors (E-EFFECT-xxx)](#effect-errors)
- [Requirement Errors (E-REQ-xxx)](#requirement-errors)
- [Symbol Errors (E-SYMBOL-xxx)](#symbol-errors)
- [Query Errors (E-QUERY-xxx)](#query-errors)
  - [E-QUERY-001: Query Exceeds Cost Budget](#e-query-001-query-exceeds-cost-budget)
  - [E-QUERY-002: Invalid Query Target](#e-query-002-invalid-query-target)
  - [E-QUERY-012: SQL Runtime Error](#e-query-012-sql-runtime-error)
  - [E-QUERY-013: Return Type Mismatch](#e-query-013-return-type-mismatch)
  - [E-QUERY-020: Unmatched Placeholder](#e-query-020-unmatched-placeholder)
  - [E-QUERY-021: Missing Placeholder](#e-query-021-missing-placeholder)
  - [E-QUERY-022: Missing Returns Annotation](#e-query-022-missing-returns-annotation)
- [Kind Errors (E-KIND-xxx)](#kind-errors)
  - [E-KIND-001: Unknown Kind](#e-kind-001-unknown-kind)
  - [E-KIND-002: Missing Effect Import](#e-kind-002-missing-effect-import)
  - [E-KIND-003: Invalid Kind Structure](#e-kind-003-invalid-kind-structure)
  - [E-KIND-004: Missing Required Section](#e-kind-004-missing-required-section)
  - [E-KIND-005: Invalid Field Value](#e-kind-005-invalid-field-value)
- [Warnings (W-xxx)](#warnings)

---

## Parse Errors (E-PARSE-xxx)

### E-PARSE-001: Unexpected Token

**Description:** Parser encountered a token that doesn't match the expected grammar.

**Example:**
```
snippet id="foo" kind="fn"
  signature
    fn name="bar"
      param name="x" type="Int"  // Missing closing for fn block
  end  // This 'end' closes signature, not fn
end
```

**Auto-fix:**
```json
{
  "kind": "canonical",
  "confidence": 1.0,
  "description": "Insert missing 'end' keyword",
  "edits": [{
    "operation": "insert_after",
    "target": "snippet[@id='foo']/signature/fn",
    "content": "    end"
  }]
}
```

---

### E-PARSE-002: Missing Required Field

**Description:** A block is missing a required field according to canonical ordering.

**Example:**
```
snippet id="foo" kind="fn"
  signature
    fn name="bar"
      // Missing 'param' or 'returns'
    end
  end
end
```

**Auto-fix:**
```json
{
  "kind": "interactive",
  "confidence": 0.7,
  "description": "Add missing function signature components",
  "suggestions": [
    {"label": "Add returns type", "confidence": 0.8},
    {"label": "Function is valid without params or returns", "confidence": 0.2}
  ]
}
```

---

### E-PARSE-003: Invalid Canonical Ordering

**Description:** Fields appear in wrong order within a block.

**Example:**
```
snippet id="foo" kind="fn"
  body
    step id="s1" kind="return"
      from="x"
      as="_"
    end
  end

  signature  // signature must come before body
    fn name="bar"
      returns type="Int"
    end
  end
end
```

**Auto-fix:**
```json
{
  "kind": "canonical",
  "confidence": 1.0,
  "description": "Reorder sections to canonical order",
  "edits": [{
    "operation": "reorder_sections",
    "target": "snippet[@id='foo']",
    "order": ["signature", "body"]
  }]
}
```

---

### E-PARSE-004: Unterminated Block

**Description:** Block opened but never closed with `end`.

**Example:**
```
snippet id="foo" kind="fn"
  body
    step id="s1" kind="return"
      as="_"
    // Missing 'end' for step
  end
end
```

**Auto-fix:**
```json
{
  "kind": "canonical",
  "confidence": 0.95,
  "description": "Add missing 'end' keyword",
  "edits": [{
    "operation": "insert_before",
    "target": "snippet[@id='foo']/body[end]",
    "content": "    end"
  }]
}
```

---

## Type Errors (E-TYPE-xxx)

### E-TYPE-001: Type Mismatch

**Description:** Expression produces type that doesn't match expected type.

**Example:**
```
step id="s1" kind="compute"
  op=add
  input var="name"  // name: String
  input lit=5       // Int
  as="result"
end
```

**Error Context:**
```json
{
  "expected": "Int",
  "actual": "String",
  "operation": "add",
  "input_index": 0
}
```

**Auto-fix:**
```json
{
  "kind": "interactive",
  "confidence": 0.6,
  "description": "Cannot add String and Int",
  "suggestions": [
    {
      "type": "query",
      "description": "Find functions that convert String to Int",
      "query": "target=project select all from=functions where and equals field=param[0].type lit=String equals field=returns.type lit=Int end end"
    }
  ]
}
```

---

### E-TYPE-002: Undefined Type

**Description:** Reference to a type that doesn't exist in symbol table.

**Example:**
```
signature
  fn name="foo"
    param name="u" type="Usr"  // Typo: should be "User"
    returns type="Bool"
  end
end
```

**Auto-fix:**
```json
{
  "kind": "ranked",
  "confidence": 0.85,
  "description": "Did you mean 'User'?",
  "edits": [{
    "operation": "replace",
    "target": "snippet[@id='foo']/signature/fn/param[@name='u']/@type",
    "content": "User"
  }],
  "alternatives": [
    {"suggestion": "User", "confidence": 0.85, "reason": "Levenshtein distance: 1"},
    {"suggestion": "UsrRole", "confidence": 0.4, "reason": "Contains 'Usr' prefix"}
  ]
}
```

---

### E-TYPE-003: Incompatible Union Members

**Description:** Union contains types that cannot be discriminated at runtime.

**Example:**
```
returns union
  type="String"
  type="String"  // Duplicate type in union
end
```

**Auto-fix:**
```json
{
  "kind": "canonical",
  "confidence": 1.0,
  "description": "Remove duplicate type from union",
  "edits": [{
    "operation": "delete",
    "target": "snippet[@id='foo']/signature/fn/returns/union/type[2]"
  }]
}
```

---

### E-TYPE-004: Non-Exhaustive Pattern Match

**Description:** Match statement doesn't cover all possible variants.

**Example:**
```
step id="s1" kind="match"
  on="result"  // result: union { User | DbError | NetworkError }
  case variant type="User" bindings=("u")
    // handle user
  end
  case variant type="DbError" bindings=("e")
    // handle db error
  end
  // Missing: NetworkError case
  as="handled"
end
```

**Auto-fix:**
```json
{
  "kind": "canonical",
  "confidence": 0.9,
  "description": "Add missing match case for NetworkError",
  "edits": [{
    "operation": "insert_before",
    "target": "snippet[@id='foo']/body/step[@id='s1'][as]",
    "content": "  case variant type=\"NetworkError\" bindings=(\"e\")\n    step id=\"s1c\" kind=\"return\"\n      variant type=\"NetworkError\"\n      from=\"e\"\n      as=\"_\"\n    end\n  end"
  }]
}
```

---

## Effect Errors (E-EFFECT-xxx)

### E-EFFECT-001: Pure Function Calls Effectful Code

**Description:** Function with no declared effects calls a function with effects.

**Example:**
```
snippet id="math.compute" kind="fn"
  // No effects declared (pure function)

  signature
    fn name="compute"
      param name="x" type="Int"
      returns type="Int"
    end
  end

  body
    step id="s1" kind="call"
      fn="db.query_count"  // This function has 'database' effect
      arg name="table" lit="users"
      as="count"
    end
  end
end
```

**Error Context:**
```json
{
  "caller": "math.compute",
  "caller_effects": [],
  "callee": "db.query_count",
  "callee_effects": ["database"]
}
```

**Auto-fix:**
```json
{
  "kind": "canonical",
  "confidence": 1.0,
  "description": "Add missing effect declarations to caller",
  "edits": [{
    "operation": "insert_after",
    "target": "snippet[@id='math.compute'][snippet_header]",
    "content": "\neffects\n  effect database\nend"
  }]
}
```

---

### E-EFFECT-002: Missing Effect Declaration

**Description:** Function performs effectful operation but doesn't declare the effect.

**Example:**
```
snippet id="fetch_user" kind="fn"
  // Missing: effects section

  body
    step id="s1" kind="query"
      target="app_db"  // Database effect required
      select all
      from="users"
      as="users"
    end
  end
end
```

**Auto-fix:**
```json
{
  "kind": "canonical",
  "confidence": 1.0,
  "description": "Add database effect declaration",
  "edits": [{
    "operation": "insert_after",
    "target": "snippet[@id='fetch_user'][snippet_header]",
    "content": "\neffects\n  effect database\nend"
  }]
}
```

---

### E-EFFECT-003: Effect Transitivity Violation

**Description:** Function A calls B, B calls C with effect E, but A doesn't declare E.

**Example:**
```
// Function C has 'filesystem' effect
snippet id="utils.log" kind="fn"
  effects
    effect filesystem
  end
end

// Function B calls C but doesn't declare effects
snippet id="middleware.audit" kind="fn"
  body
    step id="s1" kind="call"
      fn="utils.log"
      as="_"
    end
  end
end

// Function A calls B
snippet id="main.process" kind="fn"
  effects
    effect network
  end
  body
    step id="s1" kind="call"
      fn="middleware.audit"  // Transitively requires filesystem effect
      as="_"
    end
  end
end
```

**Error Context:**
```json
{
  "caller": "main.process",
  "effect_closure": ["network", "filesystem"],
  "declared_effects": ["network"],
  "missing_effects": ["filesystem"],
  "call_chain": ["main.process", "middleware.audit", "utils.log"]
}
```

**Auto-fix:**
```json
{
  "kind": "ranked",
  "confidence": 0.8,
  "description": "Add missing effect to effect closure",
  "edits": [
    {
      "description": "Add filesystem effect to main.process",
      "confidence": 0.5,
      "operation": "insert_after",
      "target": "snippet[@id='main.process']/effects/effect[last()]",
      "content": "\n  effect filesystem"
    },
    {
      "description": "Add filesystem effect to middleware.audit (source of transitivity)",
      "confidence": 0.8,
      "operation": "insert_after",
      "target": "snippet[@id='middleware.audit'][snippet_header]",
      "content": "\neffects\n  effect filesystem\nend"
    }
  ]
}
```

---

## Requirement Errors (E-REQ-xxx)

### E-REQ-001: Uncovered Requirement

**Description:** Requirement exists but no test declares `covers="R-xxx"`.

**Example:**
```
snippet id="auth.login" kind="fn"
  requires
    req id="R-AUTH-001"
      text "Users must authenticate with email and password"
      priority critical
      status approved
    end
  end

  // No tests section covering R-AUTH-001
end
```

**Auto-fix:**
```json
{
  "kind": "interactive",
  "confidence": 0.5,
  "description": "Add test to cover requirement R-AUTH-001",
  "edits": [{
    "operation": "insert_before",
    "target": "snippet[@id='auth.login'][end]",
    "content": "\ntests\n  test id=\"T-AUTH-001\" kind=\"unit\" covers=\"R-AUTH-001\"\n    // TODO: Implement test\n  end\nend"
  }]
}
```

---

### E-REQ-002: Test References Nonexistent Requirement

**Description:** Test declares `covers="R-xxx"` but requirement doesn't exist.

**Example:**
```
tests
  test id="T-001" kind="unit" covers="R-FOO-999"  // R-FOO-999 not defined
    // test body
  end
end
```

**Auto-fix:**
```json
{
  "kind": "ranked",
  "confidence": 0.7,
  "description": "Requirement R-FOO-999 not found",
  "suggestions": [
    {
      "type": "query",
      "description": "Find similar requirement IDs",
      "query": "target=project select all from=requirements where matches field=id pattern=^R-FOO"
    },
    {
      "type": "auto_fix",
      "description": "Remove invalid covers reference",
      "confidence": 0.4,
      "edits": [{
        "operation": "delete_attribute",
        "target": "snippet[@id='foo']/tests/test[@id='T-001']/@covers"
      }]
    }
  ]
}
```

---

## Symbol Errors (E-SYMBOL-xxx)

### E-SYMBOL-001: Undefined Reference

**Description:** Reference to a symbol that doesn't exist in symbol table.

**Example:**
```
step id="s1" kind="call"
  fn="validate_emai"  // Typo: should be validate_email
  arg name="email" from="user_email"
  as="is_valid"
end
```

**Auto-fix:**
```json
{
  "kind": "ranked",
  "confidence": 0.9,
  "description": "Did you mean 'validate_email'?",
  "edits": [{
    "operation": "replace",
    "target": "snippet[@id='foo']/body/step[@id='s1']/@fn",
    "content": "validate_email"
  }],
  "alternatives": [
    {"suggestion": "validate_email", "confidence": 0.9, "reason": "Levenshtein distance: 1"},
    {"suggestion": "validate_username", "confidence": 0.3, "reason": "Similar prefix"}
  ]
}
```

---

### E-SYMBOL-002: Duplicate Symbol ID

**Description:** Two snippets have the same ID.

**Example:**
```
snippet id="utils.helper" kind="fn"
  // ...
end

snippet id="utils.helper" kind="fn"  // Duplicate ID
  // ...
end
```

**Auto-fix:**
```json
{
  "kind": "interactive",
  "confidence": 0.6,
  "description": "Duplicate snippet ID 'utils.helper'",
  "suggestions": [
    {
      "description": "Rename second occurrence to 'utils.helper2'",
      "edits": [{
        "operation": "replace",
        "target": "snippet[@id='utils.helper'][2]/@id",
        "content": "utils.helper2"
      }]
    },
    {
      "type": "query",
      "description": "Show both definitions to resolve manually",
      "query": "target=project select all from=snippets where equals field=id lit=utils.helper"
    }
  ]
}
```

---

### E-SYMBOL-003: Circular Import

**Description:** Import graph contains a cycle (violates invariant I4).

**Example:**
```
// File: module_a.cov
snippet id="a.func" kind="fn"
  body
    step id="s1" kind="call"
      fn="b.func"  // A calls B
      as="_"
    end
  end
end

// File: module_b.cov
snippet id="b.func" kind="fn"
  body
    step id="s1" kind="call"
      fn="a.func"  // B calls A → cycle!
      as="_"
    end
  end
end
```

**Error Context:**
```json
{
  "cycle": ["a.func", "b.func", "a.func"],
  "cycle_length": 2
}
```

**Auto-fix:**
```json
{
  "kind": "interactive",
  "confidence": 0.3,
  "description": "Circular dependency detected",
  "suggestions": [
    {
      "type": "refactor",
      "description": "Extract common logic into new module",
      "confidence": 0.5
    },
    {
      "type": "query",
      "description": "Show call graph",
      "query": "target=project select field=calls from=functions where contains field=id any=[a.func, b.func]"
    }
  ]
}
```

---

## Query Errors (E-QUERY-xxx)

### E-QUERY-001: Query Exceeds Cost Budget

**Description:** Static analysis determines query will exceed declared cost hint.

**Example:**
```
snippet id="find_all_references" kind="fn"
  metadata
    cost_hint=cheap  // Claims <10ms
  end

  body
    step id="s1" kind="query"
      target="project"
      select all
      from="functions"
      join type="inner" table="symbols" on
        equals field="functions.id" field="symbols.referenced_by"
      end  // O(N²) join - will exceed 10ms on large projects
      as="refs"
    end
  end
end
```

**Error Context:**
```json
{
  "declared_cost": "cheap",
  "estimated_cost": "expensive",
  "complexity": "O(N²)",
  "reason": "Nested loop join without indexes"
}
```

**Auto-fix:**
```json
{
  "kind": "ranked",
  "confidence": 0.7,
  "description": "Query too expensive for cost_hint=cheap",
  "suggestions": [
    {
      "description": "Change cost_hint to 'expensive'",
      "confidence": 0.8,
      "edits": [{
        "operation": "replace",
        "target": "snippet[@id='find_all_references']/metadata/cost_hint",
        "content": "expensive"
      }]
    },
    {
      "description": "Optimize query to avoid join",
      "confidence": 0.4,
      "note": "Use bidirectional references instead"
    }
  ]
}
```

---

### E-QUERY-002: Invalid Query Target

**Description:** Query target doesn't exist or isn't queryable.

**Example:**
```
step id="s1" kind="query"
  target="unknown_db"  // Database not declared
  select all
  from="users"
  as="users"
end
```

**Auto-fix:**
```json
{
  "kind": "ranked",
  "confidence": 0.6,
  "description": "Query target 'unknown_db' not found",
  "suggestions": [
    {
      "type": "query",
      "description": "List available database targets",
      "query": "target=project select all from=snippets where equals field=kind lit=database"
    }
  ]
}
```

---

### E-QUERY-020: Unmatched Placeholder

**Description:** A placeholder in the SQL body has no matching `param` declaration.

**Example:**
```
step id="s1" kind="query"
  dialect="postgres"
  target="app_db"
  body
    SELECT * FROM users WHERE id = :user_id AND status = :status
  end
  params
    param name="user_id" from="uid"
    // Missing: param name="status" from="..."
  end
  returns collection of="User"
  as="result"
end
```

**Error Context:**
```json
{
  "placeholder": "status",
  "dialect": "postgres",
  "step_id": "s1",
  "declared_params": ["user_id"]
}
```

**Auto-fix:**
```json
{
  "kind": "ranked",
  "confidence": 0.8,
  "description": "Placeholder ':status' has no matching param declaration",
  "edits": [
    {
      "description": "Add param declaration for 'status'",
      "confidence": 0.8,
      "operation": "insert",
      "target": "step[@id='s1']/params",
      "content": "param name=\"status\" from=\"status_value\""
    }
  ]
}
```

---

### E-QUERY-021: Missing Placeholder

**Description:** A `param` is declared but has no matching placeholder in the SQL body.

**Example:**
```
step id="s1" kind="query"
  dialect="postgres"
  target="app_db"
  body
    SELECT * FROM users WHERE id = :user_id
  end
  params
    param name="user_id" from="uid"
    param name="status" from="status_value"  // ← No :status in body
  end
  returns collection of="User"
  as="result"
end
```

**Error Context:**
```json
{
  "param_name": "status",
  "dialect": "postgres",
  "step_id": "s1",
  "body_placeholders": ["user_id"]
}
```

**Auto-fix:**
```json
{
  "kind": "ranked",
  "confidence": 0.9,
  "description": "Param 'status' has no matching placeholder in body",
  "edits": [
    {
      "description": "Remove unused param declaration",
      "confidence": 0.9,
      "operation": "delete",
      "target": "step[@id='s1']/params/param[@name='status']"
    }
  ]
}
```

---

### E-QUERY-022: Missing Returns Annotation

**Description:** SQL dialect queries require a `returns` type annotation.

**Example:**
```
step id="s1" kind="query"
  dialect="postgres"
  target="app_db"
  body
    SELECT * FROM users WHERE id = :user_id
  end
  params
    param name="user_id" from="uid"
  end
  // Missing: returns type="User" or returns collection of="User"
  as="result"
end
```

**Error Context:**
```json
{
  "dialect": "postgres",
  "step_id": "s1"
}
```

**Auto-fix:**
```json
{
  "kind": "interactive",
  "confidence": 0.0,
  "description": "SQL dialect queries require 'returns' type annotation",
  "suggestions": [
    {
      "type": "interactive",
      "description": "Add returns annotation",
      "note": "Add 'returns type=\"TypeName\"' or 'returns collection of=\"TypeName\"' after params section"
    }
  ]
}
```

---

### E-QUERY-012: SQL Runtime Error

**Description:** The SQL query was rejected by the database at runtime. This occurs when the database parser cannot understand the SQL string.

**Example:**
```
step id="s1" kind="query"
  dialect="postgres"
  target="app_db"
  body
    SELECT * FORM users WHERE id = :id  // ← Typo: FORM instead of FROM
  end
  params
    param name="id" from="user_id"
  end
  returns collection of="User"
  as="users"
end
```

**Error Context:**
```json
{
  "dialect": "postgres",
  "error_message": "syntax error at or near \"FORM\"",
  "error_position": 10,
  "sql_snippet": "SELECT * FORM users",
  "step_id": "s1"
}
```

**Auto-fix:**
```json
{
  "kind": "interactive",
  "confidence": 0.0,
  "description": "SQL syntax error - database rejected query",
  "suggestions": [
    {
      "type": "interactive",
      "description": "Review and fix SQL syntax",
      "note": "Check SQL syntax for dialect 'postgres'. Error at position 10."
    }
  ]
}
```

---

### E-QUERY-013: Return Type Mismatch

**Description:** SQL query returned data incompatible with the declared return type. The actual columns don't match the expected type schema. This is a runtime error.

**Example:**
```
step id="s1" kind="query"
  dialect="sqlserver"
  target="app_db"
  body
    SELECT user_id, name FROM users
  end
  params
  end
  returns collection of="UserEmail"    // ← Expects id, email fields
  as="result"
end
```

**Error Context:**
```json
{
  "expected_type": "UserEmail",
  "expected_fields": [{"name": "id", "type": "Int"}, {"name": "email", "type": "String"}],
  "actual_fields": [{"name": "user_id", "type": "Int"}, {"name": "name", "type": "String"}],
  "missing_fields": ["email"],
  "extra_fields": ["user_id", "name"],
  "step_id": "s1"
}
```

**Auto-fix:**
```json
{
  "kind": "ranked",
  "confidence": 0.6,
  "description": "Return type mismatch: query returns different fields than declared",
  "suggestions": [
    {
      "description": "Update SQL to match declared type",
      "confidence": 0.5,
      "note": "Change SELECT to: SELECT id, email FROM users"
    },
    {
      "description": "Create new type matching actual query result",
      "confidence": 0.4,
      "note": "Create type with fields: user_id (Int), name (String)"
    }
  ]
}

---

## Kind Errors (E-KIND-xxx)

Errors related to extensible kinds imported via the effects system.

### E-KIND-001: Unknown Kind

**Description:** Step uses a kind that is neither a core kind nor an imported extended kind.

**Example:**
```
snippet id="app.fetch" kind="fn"

effects
  effect network
end

body
  step id="s1" kind="std.concurrent.parallel"  // Error: std.concurrent not imported
    branch id="b1"
      step id="b1.1" kind="call"
        fn="http.get"
        arg name="url" lit="https://api.example.com"
        as="response"
      end
    end
    as="result"
  end
end

end
```

**Error Output:**
```
Error E-KIND-001: Unknown kind 'std.concurrent.parallel'
  --> app.cov:9:3
   |
 9 |   step id="s1" kind="std.concurrent.parallel"
   |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = hint: Add 'effect std.concurrent' to snippet effects section
```

**Auto-fix:**
```json
{
  "kind": "canonical",
  "confidence": 1.0,
  "description": "Add missing effect import for kind",
  "edits": [{
    "operation": "insert_after",
    "target": "snippet[@id='app.fetch']/effects/effect[last()]",
    "content": "\n  effect std.concurrent"
  }]
}
```

---

### E-KIND-002: Missing Effect Import

**Description:** Snippet uses an extended kind but doesn't declare the required effect.

**Example:**
```
snippet id="dashboard.load" kind="fn"

// No effects section at all

body
  step id="s1" kind="std.concurrent.parallel"
    branch id="b1" ... end
    as="result"
  end
end

end
```

**Error Output:**
```
Error E-KIND-002: Kind 'std.concurrent.parallel' requires effect 'std.concurrent'
  --> dashboard.cov:6:3
   |
 6 |   step id="s1" kind="std.concurrent.parallel"
   |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: Extended kinds must be imported via effects
   = hint: Add effects section with 'effect std.concurrent'
```

**Auto-fix:**
```json
{
  "kind": "canonical",
  "confidence": 1.0,
  "description": "Add effects section with required effect",
  "edits": [{
    "operation": "insert_after",
    "target": "snippet[@id='dashboard.load'][snippet_header]",
    "content": "\neffects\n  effect std.concurrent\nend"
  }]
}
```

---

### E-KIND-003: Invalid Kind Structure

**Description:** Extended step body doesn't match the structure defined in the kind definition.

**Example:**
```
snippet id="app.fetch" kind="fn"

effects
  effect std.concurrent
end

body
  step id="s1" kind="std.concurrent.parallel"
    // Missing required 'branch' sections
    as="result"
  end
end

end
```

**Error Output:**
```
Error E-KIND-003: Kind 'std.concurrent.parallel' requires at least one 'branch' section
  --> app.cov:8:3
   |
 8 |   step id="s1" kind="std.concurrent.parallel"
   |   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: Kind definition requires: section 'branch' (multiple=true, required=true)
   = hint: Add branch blocks containing steps
```

**Auto-fix:**
```json
{
  "kind": "interactive",
  "confidence": 0.5,
  "description": "Add branch section to parallel block",
  "edits": [{
    "operation": "insert_before",
    "target": "snippet[@id='app.fetch']/body/step[@id='s1']/as",
    "content": "    branch id=\"b1\"\n      // TODO: Add steps\n    end\n"
  }]
}
```

---

### E-KIND-004: Missing Required Section

**Description:** Kind definition is missing a required section in its structure.

**Example:**
```
snippet id="myorg.custom" kind="effect-kind"

kinds
  kind name="my_step"
    structure
      // Missing required field or section definitions
    end
    // Missing compile_to
  end
end

end
```

**Error Output:**
```
Error E-KIND-004: Kind definition 'my_step' missing required field 'compile_to'
  --> myorg.cov:4:3
   |
 4 |   kind name="my_step"
   |   ^^^^^^^^^^^^^^^^^^^^
   |
   = note: Kind definitions must specify 'compile_to' for code generation
   = hint: Add compile_to="your_runtime_handler"
```

**Auto-fix:**
```json
{
  "kind": "interactive",
  "confidence": 0.3,
  "description": "Add compile_to field to kind definition",
  "suggestions": [
    {"label": "compile_to=\"host_custom\"", "confidence": 0.5},
    {"label": "compile_to=\"wasm_custom\"", "confidence": 0.3}
  ]
}
```

---

### E-KIND-005: Invalid Field Value

**Description:** Field value doesn't match the expected type or allowed values in the kind definition.

**Example:**
```
snippet id="app.fetch" kind="fn"

effects
  effect std.concurrent
end

body
  step id="s1" kind="std.concurrent.parallel"
    on_error="abort"  // Invalid: allowed values are fail_fast, collect_all, ignore_errors

    branch id="b1"
      step id="b1.1" kind="call"
        fn="http.get"
        arg name="url" lit="https://api.example.com"
        as="response"
      end
    end
    as="result"
  end
end

end
```

**Error Output:**
```
Error E-KIND-005: Invalid value 'abort' for field 'on_error'
  --> app.cov:9:5
   |
 9 |     on_error="abort"
   |              ^^^^^^^
   |
   = note: Allowed values: fail_fast, collect_all, ignore_errors
   = hint: Did you mean 'fail_fast'?
```

**Auto-fix:**
```json
{
  "kind": "ranked",
  "confidence": 0.9,
  "description": "Fix invalid field value",
  "edits": [
    {
      "description": "Change to 'fail_fast' (most similar)",
      "confidence": 0.9,
      "operation": "replace",
      "target": "snippet[@id='app.fetch']/body/step[@id='s1']/@on_error",
      "content": "\"fail_fast\""
    },
    {
      "description": "Change to 'collect_all'",
      "confidence": 0.5,
      "operation": "replace",
      "target": "snippet[@id='app.fetch']/body/step[@id='s1']/@on_error",
      "content": "\"collect_all\""
    }
  ]
}
```

---

## Warnings (W-xxx)

### W-DEAD-001: Unused Binding

**Description:** Variable bound but never referenced.

**Example:**
```
step id="s1" kind="compute"
  op=add
  input lit=5
  input lit=3
  as="result"  // 'result' never used
end

step id="s2" kind="return"
  lit=42
  as="_"
end
```

**Auto-fix:**
```json
{
  "kind": "canonical",
  "confidence": 0.9,
  "description": "Remove unused binding 'result'",
  "edits": [{
    "operation": "delete",
    "target": "snippet[@id='foo']/body/step[@id='s1']"
  }]
}
```

---

### W-DEAD-002: Unreachable Code

**Description:** Code that can never be executed.

**Example:**
```
step id="s1" kind="return"
  lit=42
  as="_"
end

step id="s2" kind="compute"  // Unreachable - after return
  op=add
  input lit=1
  input lit=2
  as="dead"
end
```

**Auto-fix:**
```json
{
  "kind": "canonical",
  "confidence": 1.0,
  "description": "Remove unreachable code",
  "edits": [{
    "operation": "delete",
    "target": "snippet[@id='foo']/body/step[@id='s2']"
  }]
}
```

---

### W-DEAD-003: Uncalled Function

**Description:** Function is not called by any other function and is not an entry point.

**Example:**
```
snippet id="utils.old_helper" kind="fn"
  // Function never called, not exported, not entry point
  // symbol_metadata.called_by = []
end
```

**Error Context:**
```json
{
  "symbol_id": "utils.old_helper",
  "called_by": [],
  "is_exported": false,
  "is_entry_point": false
}
```

**Auto-fix:**
```json
{
  "kind": "interactive",
  "confidence": 0.6,
  "description": "Function 'utils.old_helper' is never called",
  "suggestions": [
    {
      "description": "Delete unused function",
      "confidence": 0.7,
      "edits": [{
        "operation": "delete",
        "target": "snippet[@id='utils.old_helper']"
      }]
    },
    {
      "description": "Mark as exported (keep for external use)",
      "confidence": 0.3,
      "note": "Add metadata: is_exported=true"
    }
  ]
}
```

---

### W-PERF-001: Inefficient Query Pattern

**Description:** Query uses pattern that could be optimized.

**Example:**
```
step id="s1" kind="query"
  target="project"
  select all
  from="functions"
  as="all_funcs"
end

step id="s2" kind="for"
  var="f"
  in="all_funcs"
  // Filter in loop instead of WHERE clause
  step id="s2a" kind="if"
    condition="has_db_effect"
    then
      // process
    end
  end
end
```

**Auto-fix:**
```json
{
  "kind": "ranked",
  "confidence": 0.8,
  "description": "Move filter into WHERE clause for better performance",
  "edits": [{
    "operation": "replace",
    "target": "snippet[@id='foo']/body/step[@id='s1']",
    "content": "step id=\"s1\" kind=\"query\"\n  target=\"project\"\n  select all\n  from=\"functions\"\n  where\n    contains field=\"effects\" lit=\"database\"\n  end\n  as=\"db_funcs\"\nend"
  }]
}
```

---

## JSON Schema for Error Messages

All errors conform to this schema for machine parsing:

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["code", "severity", "message", "source_location"],
  "properties": {
    "code": {
      "type": "string",
      "pattern": "^(E|W)-[A-Z]+-\\d{3}$"
    },
    "severity": {
      "enum": ["error", "warning", "info"]
    },
    "message": {
      "type": "string"
    },
    "source_location": {
      "type": "object",
      "required": ["file"],
      "properties": {
        "file": {"type": "string"},
        "snippet_id": {"type": "string"},
        "step_id": {"type": "string"},
        "line": {"type": "integer"},
        "column": {"type": "integer"}
      }
    },
    "context": {
      "type": "object",
      "description": "Error-specific context (varies by error code)"
    },
    "suggestions": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["type"],
        "properties": {
          "type": {
            "enum": ["auto_fix", "query", "refactor"]
          },
          "description": {"type": "string"},
          "confidence": {
            "type": "number",
            "minimum": 0,
            "maximum": 1
          },
          "edits": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["operation", "target"],
              "properties": {
                "operation": {
                  "enum": ["insert_before", "insert_after", "replace", "delete", "reorder_sections"]
                },
                "target": {"type": "string", "description": "XPath-like selector"},
                "content": {"type": "string"}
              }
            }
          },
          "query": {"type": "string"},
          "alternatives": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "suggestion": {"type": "string"},
                "confidence": {"type": "number"},
                "reason": {"type": "string"}
              }
            }
          }
        }
      }
    }
  }
}
```

---

## Implementation Notes

### Error Recovery Strategy

1. **Parse Phase:**
   - On syntax error, attempt recovery at next block boundary (`end` keyword)
   - Insert synthetic `end` if EOF reached with open blocks
   - Continue parsing to collect all parse errors

2. **Symbol Phase:**
   - On undefined reference, insert placeholder symbol to allow type checking
   - Mark placeholder for later resolution

3. **Type Phase:**
   - On type error, insert `unknown` type to continue checking
   - Propagate `unknown` to avoid cascading errors

4. **Effect Phase:**
   - On effect violation, insert missing effect declarations temporarily
   - Mark as auto-generated for user review

### Auto-Fix Confidence Levels

- **1.0:** Deterministic, always correct (e.g., missing `end` keyword)
- **0.9-0.99:** High confidence, usually correct (e.g., Levenshtein distance 1 typo fix)
- **0.8-0.89:** Good confidence, often correct (e.g., inferred type from usage)
- **0.5-0.79:** Moderate confidence, requires review (e.g., multiple alternatives)
- **<0.5:** Low confidence, interactive choice required (e.g., ambiguous refactoring)

### Edit Operations

All edit operations use XPath-like selectors for precise targeting:

- `snippet[@id='foo']` - Select snippet by ID
- `snippet[@id='foo']/body/step[@id='s1']` - Select specific step
- `snippet[@id='foo']/signature/fn/param[@name='x']/@type` - Select attribute
- `snippet[@id='foo']/effects/effect[last()]` - Select last effect
- `snippet[@id='foo'][end]` - Select position just before closing `end`

---

## Related Documents

- [DESIGN.md](DESIGN.md) - Section 11: Error Diagnostics
- [DESIGN.md](DESIGN.md) - Section 12: Auto-Fix Protocol
- [COMPILER.md](COMPILER.md) - Compilation phase error handling
