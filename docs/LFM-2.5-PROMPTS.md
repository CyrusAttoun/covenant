# LFM-2.5 Prompts for Covenant Code Generation

Test prompts for evaluating small local LLMs (LFM2.5-1.2B-Instruct, Llama 3.2 1B) on Covenant syntax generation.

Each prompt includes the complete grammar and sufficient context to generate the target example.

## Prompt Design Principles

Based on testing, these techniques improve accuracy:

1. **Explicit `end` tracking** - Add a rule stating every block requires matching `end`
2. **Template skeleton** - Show the nesting structure before the task
3. **One snippet per prompt** - Reduce context accumulation errors
4. **Explicit return step rule** - Functions must end with explicit return
5. **`as="..."` syntax reminder** - Always use equals sign, not space
6. **Forced terminator** - Require `// END` comment after final `end` to prevent truncation

---

## Prompt 1: Hello World

**Target**: Simple effectful function with console output

```
You are a code generator for Covenant, a machine-first programming language. Generate valid Covenant code following the grammar and rules below.

=== COVENANT GRAMMAR (EBNF) ===

(* Top Level *)
program        = { snippet } ;
snippet        = "snippet" "id" "=" STRING "kind" "=" snippet_kind { section } "end" ;
snippet_kind   = "fn" | "struct" | "enum" | "module" | "database" | "extern" | "test" | "data" ;

(* Sections - must appear in this canonical order *)
section        = effects_section | requires_section | signature_section | body_section | tests_section | metadata_section ;

(* Effects Section *)
effects_section = "effects" { effect_decl } "end" ;
effect_decl     = "effect" IDENT ;

(* Signature Section *)
signature_section = "signature" fn_signature "end" ;
fn_signature   = "fn" "name" "=" STRING { param_decl } [ returns_decl ] "end" ;
param_decl     = "param" "name" "=" STRING "type" "=" type_ref ;
returns_decl   = "returns" ( "type" "=" type_ref | "collection" "of" "=" type_ref | "union" { union_member } "end" ) ;
union_member   = "type" "=" type_ref [ "optional" ] ;

(* Body Section - SSA form, one operation per step *)
body_section   = "body" { step } "end" ;
step           = "step" "id" "=" STRING "kind" "=" step_kind step_body "as" "=" STRING "end" ;
step_kind      = "compute" | "call" | "query" | "bind" | "return" | "if" | "match" | "for" ;

(* Call Step *)
call_body      = "fn" "=" STRING { call_arg } ;
call_arg       = "arg" "name" "=" STRING ( "from" "=" STRING | "lit" "=" literal ) ;

(* Return Step *)
return_body    = "from" "=" STRING | "lit" "=" literal ;

(* Types and Literals *)
type_ref       = IDENT [ "?" | "[]" ] ;
literal        = NUMBER | STRING | "true" | "false" | "none" ;
STRING         = '"' { any_char } '"' ;

=== CRITICAL RULES ===

1. Every snippet starts with `snippet id="..." kind="..."` and ends with `end`
2. Sections must appear in canonical order: effects, requires, signature, body, tests, metadata
3. Every step has: id, kind, body content, and `as="..."` for output binding
4. Use `as="_"` when discarding the return value (like void/Unit)
5. Function calls use fully-qualified snippet IDs: `fn="module.function_name"`
6. Effects must be declared before using effectful functions
7. No operators - use keywords (add, equals, and, or, not)
8. Double quotes only for strings
9. MUST end snippet with `// END` comment on the line after the final `end`

=== TASK ===

Generate a Covenant function that:
- Has snippet id "main.hello" and kind "fn"
- Declares the "console" effect (required for printing)
- Has a signature with function name "main", no parameters, returns type "Unit"
- Body calls "console.println" with argument name="message" and literal value "Hello, world!"
- Discards the return value with as="_"
- End with `// END` comment after the final `end`

Generate only the Covenant code, no explanations.
```

**Expected Output**:
```covenant
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
    fn="console.println"
    arg name="message" lit="Hello, world!"
    as="_"
  end
end

end
// END
```

---

## Prompt 2: Project Queries

**Target**: Meta-programming functions that query the AST/symbol graph

```
You are a code generator for Covenant, a machine-first programming language. Generate valid Covenant code following the grammar and rules below.

=== COVENANT GRAMMAR (EBNF) ===

(* Top Level *)
program        = { snippet } ;
snippet        = "snippet" "id" "=" STRING "kind" "=" snippet_kind { section } "end" ;
snippet_kind   = "fn" | "struct" | "enum" | "module" | "database" | "extern" | "test" | "data" ;

(* Sections - canonical order *)
section        = effects_section | requires_section | signature_section | body_section | tests_section | metadata_section ;

(* Effects Section *)
effects_section = "effects" { effect_decl } "end" ;
effect_decl     = "effect" IDENT ;

(* Signature Section *)
signature_section = "signature" fn_signature "end" ;
fn_signature   = "fn" "name" "=" STRING { param_decl } [ returns_decl ] "end" ;
param_decl     = "param" "name" "=" STRING "type" "=" type_ref ;
returns_decl   = "returns" ( "type" "=" type_ref | "collection" "of" "=" type_ref | "union" { union_member } "end" ) ;

(* Body Section *)
body_section   = "body" { step } "end" ;
step           = "step" "id" "=" STRING "kind" "=" step_kind step_body "as" "=" STRING "end" ;
step_kind      = "compute" | "call" | "query" | "bind" | "return" | "if" | "match" | "for" ;

(* Query Step - Covenant dialect for querying project/AST *)
query_body     = "target" "=" STRING select_clause from_clause [ where_clause ] [ order_clause ] [ limit_clause ] ;
select_clause  = "select" ( "all" | { "field" "=" STRING } ) ;
from_clause    = "from" "=" STRING ;
where_clause   = "where" condition "end" ;
condition      = simple_condition | compound_condition ;
simple_condition = compare_op "field" "=" STRING ( "var" "=" STRING | "lit" "=" literal ) ;
compare_op     = "equals" | "not_equals" | "less" | "greater" | "contains" | "matches" ;
compound_condition = ( "and" | "or" ) { condition } "end" ;
order_clause   = "order" "by" "=" STRING "dir" "=" ( "asc" | "desc" ) ;
limit_clause   = "limit" "=" NUMBER ;

(* Return Step *)
return_body    = "from" "=" STRING ;

(* Types *)
type_ref       = IDENT [ "?" | "[]" ] ;
literal        = NUMBER | STRING | "true" | "false" | "none" | "[" [ literal { "," literal } ] "]" ;

=== CRITICAL RULES ===

1. EVERY BLOCK NEEDS MATCHING `end`: snippet...end, effects...end, signature...end, fn...end, body...end, step...end, where...end, and...end
2. Sections in order: effects, requires, signature, body, tests, metadata
3. EVERY step has: id, kind, body content, and `as="..."` (with equals sign, not space)
4. EVERY function body MUST end with an explicit return step
5. The "meta" effect allows querying the project's symbol graph
6. target="project" queries the current project's AST
7. Compound conditions use `and ... end` or `or ... end` with nested conditions
8. Array literals use brackets: lit=[] for empty
9. MUST end EVERY snippet with `// END` comment on the line after the final `end`

=== TEMPLATE (follow this structure exactly) ===

snippet id="..." kind="fn"

effects
  effect ...
end

signature
  fn name="..."
    ...
  end
end

body
  step id="s1" kind="query"
    ...
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
// END

=== TASK ===

Generate THREE Covenant functions. Each function MUST have a query step followed by a return step. Each snippet MUST end with `// END` comment.

FUNCTION 1: meta.find_db_functions
- Effect: meta
- Returns: collection of="FunctionInfo"
- Query target="project", select all from="functions"
- Where: contains field="effects" lit="database"
- Step s1: query, Step s2: return from="result"

FUNCTION 2: meta.find_callers
- Effect: meta
- Parameter: fn_name of type String
- Returns: collection of="FunctionInfo"
- Query target="project", select field="called_by" from="functions"
- Where: equals field="name" var="fn_name"
- Step s1: query, Step s2: return from="result"

FUNCTION 3: meta.find_dead_code
- Effect: meta
- Returns: collection of="FunctionInfo"
- Query target="project", select all from="functions"
- Where: compound AND with THREE conditions inside:
  - equals field="called_by" lit=[]
  - equals field="is_exported" lit=false
  - equals field="is_entry_point" lit=false
- Step s1: query, Step s2: return from="result"

Generate only the Covenant code for all three functions, no explanations.
```

**Expected Output**:
```covenant
snippet id="meta.find_db_functions" kind="fn"

effects
  effect meta
end

signature
  fn name="find_db_functions"
    returns collection of="FunctionInfo"
  end
end

body
  step id="s1" kind="query"
    target="project"
    select all
    from="functions"
    where
      contains field="effects" lit="database"
    end
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
// END


snippet id="meta.find_callers" kind="fn"

effects
  effect meta
end

signature
  fn name="find_callers"
    param name="fn_name" type="String"
    returns collection of="FunctionInfo"
  end
end

body
  step id="s1" kind="query"
    target="project"
    select field="called_by"
    from="functions"
    where
      equals field="name" var="fn_name"
    end
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
// END


snippet id="meta.find_dead_code" kind="fn"

effects
  effect meta
end

signature
  fn name="find_dead_code"
    returns collection of="FunctionInfo"
  end
end

body
  step id="s1" kind="query"
    target="project"
    select all
    from="functions"
    where
      and
        equals field="called_by" lit=[]
        equals field="is_exported" lit=false
        equals field="is_entry_point" lit=false
      end
    end
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
// END
```

---

## Prompt 3: Advanced SQL

**Target**: Complex SQL queries with CTEs, window functions, and dialect-specific syntax

**Note**: This prompt is split into two separate prompts (3a and 3b) to reduce complexity and improve accuracy.

### Prompt 3a: PostgreSQL with CTE

```
You are a code generator for Covenant, a machine-first programming language. Generate valid Covenant code following the grammar and rules below.

=== COVENANT GRAMMAR (EBNF) ===

snippet        = "snippet" "id" "=" STRING "kind" "=" snippet_kind { section } "end" ;
section        = effects_section | requires_section | signature_section | body_section | tests_section ;

effects_section = "effects" { "effect" IDENT } "end" ;
requires_section = "requires" { requirement } "end" ;
requirement    = "req" "id" "=" STRING "text" STRING "priority" ( "critical" | "high" | "medium" | "low" ) "end" ;
signature_section = "signature" fn_signature "end" ;
fn_signature   = "fn" "name" "=" STRING { param_decl } returns_decl "end" ;
param_decl     = "param" "name" "=" STRING "type" "=" type_ref ;
returns_decl   = "returns" "collection" "of" "=" type_ref ;

body_section   = "body" { step } "end" ;
step           = "step" "id" "=" STRING "kind" "=" step_kind step_body "as" "=" STRING "end" ;

(* SQL Query Step *)
sql_query_step = "dialect" "=" STRING "target" "=" STRING "body" RAW_SQL "end" "params" { param_binding } "end" "returns" "collection" "of" "=" type_ref ;
param_binding  = "param" "name" "=" STRING "from" "=" STRING ;

return_step    = "from" "=" STRING ;

tests_section  = "tests" { test_def } "end" ;
test_def       = "test" "id" "=" STRING "kind" "=" "unit" "covers" "=" STRING "end" ;

=== CRITICAL RULES ===

1. EVERY BLOCK NEEDS `end`: snippet...end, effects...end, requires...end, req...end, signature...end, fn...end, body...end, step...end, params...end, tests...end, test...end
2. ALWAYS use `as="..."` with EQUALS SIGN (not `as "..."` with space)
3. Function body MUST have TWO steps: (1) query step with as="result", (2) return step with as="_"
4. SQL goes between `body` and `end` INSIDE the query step - do NOT add extra syntax
5. PostgreSQL parameters use :name syntax in SQL
6. MUST end snippet with `// END` comment on the line after the final `end`

=== TEMPLATE (copy this structure exactly) ===

snippet id="..." kind="fn"

effects
  effect database
end

requires
  req id="..."
    text "..."
    priority high
  end
end

signature
  fn name="..."
    param name="..." type="..."
    returns collection of="..."
  end
end

body
  step id="s1" kind="query"
    dialect="postgres"
    target="..."
    body
      SELECT ... FROM ... WHERE ... :param_name ...
    end
    params
      param name="..." from="..."
    end
    returns collection of="..."
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

tests
  test id="..." kind="unit" covers="..."
  end
end

end
// END

=== TASK ===

Generate ONE Covenant function: analytics.high_value_customers

- Snippet id: "analytics.high_value_customers"
- Kind: "fn"
- Effect: database
- Requirement: id="R-ANALYTICS-001", text="Identify customers with total spending above threshold", priority=high
- Function name: "get_high_value_customers"
- Parameters: min_revenue (Decimal), min_orders (Int)
- Returns: collection of="CustomerStats"
- Dialect: "postgres"
- Target: "app_db"
- SQL (copy exactly):
WITH customer_orders AS (
  SELECT customer_id, COUNT(*) as order_count, SUM(total) as total_revenue
  FROM orders GROUP BY customer_id
)
SELECT c.id, c.name, co.order_count, co.total_revenue
FROM customer_orders co JOIN customers c ON c.id = co.customer_id
WHERE co.total_revenue > :min_revenue AND co.order_count >= :min_orders
ORDER BY co.total_revenue DESC
- Params: min_revenue from="min_revenue", min_orders from="min_orders"
- Test: id="T-ANALYTICS-001", kind="unit", covers="R-ANALYTICS-001"
- Steps: s1 (query), s2 (return)
- End with `// END` comment after the final `end`

Generate only the Covenant code, no explanations.
```

### Prompt 3b: SQL Server with Window Functions

```
You are a code generator for Covenant, a machine-first programming language. Generate valid Covenant code following the grammar and rules below.

=== COVENANT GRAMMAR (EBNF) ===

snippet        = "snippet" "id" "=" STRING "kind" "=" snippet_kind { section } "end" ;
section        = effects_section | signature_section | body_section ;

effects_section = "effects" { "effect" IDENT } "end" ;
signature_section = "signature" fn_signature "end" ;
fn_signature   = "fn" "name" "=" STRING { param_decl } returns_decl "end" ;
param_decl     = "param" "name" "=" STRING "type" "=" type_ref ;
returns_decl   = "returns" "collection" "of" "=" type_ref ;

body_section   = "body" { step } "end" ;
step           = "step" "id" "=" STRING "kind" "=" step_kind step_body "as" "=" STRING "end" ;

(* SQL Query Step *)
sql_query_step = "dialect" "=" STRING "target" "=" STRING "body" RAW_SQL "end" "params" { param_binding } "end" "returns" "collection" "of" "=" type_ref ;
param_binding  = "param" "name" "=" STRING "from" "=" STRING ;

return_step    = "from" "=" STRING ;

=== CRITICAL RULES ===

1. EVERY BLOCK NEEDS `end`: snippet...end, effects...end, signature...end, fn...end, body...end, step...end, params...end
2. ALWAYS use `as="..."` with EQUALS SIGN (not `as "..."` with space)
3. Function body MUST have TWO steps: (1) query step with as="result", (2) return step with as="_"
4. SQL goes between `body` and `end` INSIDE the query step
5. SQL Server parameters use @name syntax in SQL
6. MUST end snippet with `// END` comment on the line after the final `end`

=== TEMPLATE (copy this structure exactly) ===

snippet id="..." kind="fn"

effects
  effect database
end

signature
  fn name="..."
    param name="..." type="..."
    returns collection of="..."
  end
end

body
  step id="s1" kind="query"
    dialect="sqlserver"
    target="..."
    body
      SELECT ... FROM ... WHERE ... @param_name ...
    end
    params
      param name="..." from="..."
    end
    returns collection of="..."
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
// END

=== TASK ===

Generate ONE Covenant function: analytics.sales_with_metrics

- Snippet id: "analytics.sales_with_metrics"
- Kind: "fn"
- Effect: database
- Function name: "calculate_sales_metrics"
- Parameter: user_id (Int)
- Returns: collection of="SalesMetrics"
- Dialect: "sqlserver"
- Target: "analytics_db"
- SQL (copy exactly):
SELECT order_id, order_date, amount,
  ROW_NUMBER() OVER (ORDER BY order_date) as order_sequence,
  SUM(amount) OVER (ORDER BY order_date ROWS UNBOUNDED PRECEDING) as running_total
FROM orders WHERE user_id = @user_id ORDER BY order_date
- Params: user_id from="user_id"
- Steps: s1 (query), s2 (return)
- End with `// END` comment after the final `end`

Generate only the Covenant code, no explanations.
```

**Expected Output for Prompt 3a**:
```covenant
snippet id="analytics.high_value_customers" kind="fn"

effects
  effect database
end

requires
  req id="R-ANALYTICS-001"
    text "Identify customers with total spending above threshold"
    priority high
  end
end

signature
  fn name="get_high_value_customers"
    param name="min_revenue" type="Decimal"
    param name="min_orders" type="Int"
    returns collection of="CustomerStats"
  end
end

body
  step id="s1" kind="query"
    dialect="postgres"
    target="app_db"
    body
      WITH customer_orders AS (
        SELECT customer_id, COUNT(*) as order_count, SUM(total) as total_revenue
        FROM orders GROUP BY customer_id
      )
      SELECT c.id, c.name, co.order_count, co.total_revenue
      FROM customer_orders co JOIN customers c ON c.id = co.customer_id
      WHERE co.total_revenue > :min_revenue AND co.order_count >= :min_orders
      ORDER BY co.total_revenue DESC
    end
    params
      param name="min_revenue" from="min_revenue"
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

tests
  test id="T-ANALYTICS-001" kind="unit" covers="R-ANALYTICS-001"
  end
end

end
// END
```

**Expected Output for Prompt 3b**:
```covenant
snippet id="analytics.sales_with_metrics" kind="fn"

effects
  effect database
end

signature
  fn name="calculate_sales_metrics"
    param name="user_id" type="Int"
    returns collection of="SalesMetrics"
  end
end

body
  step id="s1" kind="query"
    dialect="sqlserver"
    target="analytics_db"
    body
      SELECT order_id, order_date, amount,
        ROW_NUMBER() OVER (ORDER BY order_date) as order_sequence,
        SUM(amount) OVER (ORDER BY order_date ROWS UNBOUNDED PRECEDING) as running_total
      FROM orders WHERE user_id = @user_id ORDER BY order_date
    end
    params
      param name="user_id" from="user_id"
    end
    returns collection of="SalesMetrics"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
// END
```

---

## Prompt 4: Pattern Matching

**Target**: Enum definition and functions using match expressions with variant patterns and bindings

**Note**: Split into three separate prompts (4a, 4b, 4c) to improve accuracy.

### Prompt 4a: Enum Definition

```
You are a code generator for Covenant, a machine-first programming language. Generate valid Covenant code following the grammar and rules below.

=== COVENANT GRAMMAR (EBNF) ===

snippet        = "snippet" "id" "=" STRING "kind" "=" snippet_kind { section } "end" ;
snippet_kind   = "fn" | "struct" | "enum" ;
section        = signature_section ;

signature_section = "signature" enum_signature "end" ;
enum_signature = "enum" "name" "=" STRING { enum_variant } "end" ;
enum_variant   = "variant" "name" "=" STRING [ { field_decl } ] "end" ;
field_decl     = "field" "name" "=" STRING "type" "=" type_ref ;

type_ref       = IDENT [ "[]" | "<" type_ref "," type_ref ">" ] ;

=== CRITICAL RULES ===

1. EVERY BLOCK NEEDS `end`: snippet...end, signature...end, enum...end, variant...end
2. Enum snippets use kind="enum"
3. Each variant MUST have its own `end`
4. Variants without fields still need `end` immediately after the name
5. MUST end snippet with `// END` comment on the line after the final `end`

=== TEMPLATE ===

snippet id="..." kind="enum"

signature
  enum name="..."
    variant name="NoFields"
    end
    variant name="WithField"
      field name="..." type="..."
    end
  end
end

end
// END

=== TASK ===

Generate ONE enum snippet: json.Json

- Snippet id: "json.Json"
- Kind: "enum"
- Enum name: "Json"
- 6 variants (each needs its own `end`):
  1. variant name="Null" end (no fields)
  2. variant name="Bool" field name="value" type="Bool" end
  3. variant name="Number" field name="value" type="Float" end
  4. variant name="String" field name="value" type="String" end
  5. variant name="Array" field name="items" type="Json[]" end
  6. variant name="Object" field name="fields" type="Map<String, Json>" end
- End with `// END` comment after the final `end`

Generate only the Covenant code, no explanations.
```

### Prompt 4b: Match with Multiple Cases

```
You are a code generator for Covenant, a machine-first programming language. Generate valid Covenant code following the grammar and rules below.

=== COVENANT GRAMMAR (EBNF) ===

snippet        = "snippet" "id" "=" STRING "kind" "=" "fn" { section } "end" ;
section        = signature_section | body_section ;

signature_section = "signature" fn_signature "end" ;
fn_signature   = "fn" "name" "=" STRING param_decl returns_decl "end" ;
param_decl     = "param" "name" "=" STRING "type" "=" type_ref ;
returns_decl   = "returns" "type" "=" type_ref ;

body_section   = "body" { step } "end" ;
step           = "step" "id" "=" STRING "kind" "=" step_kind step_body "as" "=" STRING "end" ;

(* Match Step *)
match_step     = "on" "=" STRING { match_case } ;
match_case     = "case" "variant" "type" "=" STRING { step } "end" ;

(* Return Step *)
return_step    = "lit" "=" literal ;
literal        = STRING ;

=== CRITICAL RULES ===

1. EVERY BLOCK NEEDS `end`: snippet...end, signature...end, fn...end, body...end, step...end, case...end
2. ALWAYS use `as="..."` with EQUALS SIGN
3. Match step structure: on="var" then cases, then as="_" at the very end
4. Each case has: case variant type="..." then steps inside, then end
5. Steps inside cases use sub-IDs: s1a, s1b, s1c, s1d, s1e, s1f
6. Variant types use :: syntax: "EnumName::VariantName"
7. MUST end snippet with `// END` comment on the line after the final `end`

=== TEMPLATE ===

snippet id="..." kind="fn"

signature
  fn name="..."
    param name="..." type="..."
    returns type="..."
  end
end

body
  step id="s1" kind="match"
    on="..."
    case variant type="Type::Variant1"
      step id="s1a" kind="return"
        lit="..."
        as="_"
      end
    end
    case variant type="Type::Variant2"
      step id="s1b" kind="return"
        lit="..."
        as="_"
      end
    end
    as="_"
  end
end

end
// END

=== TASK ===

Generate ONE function snippet: json.type_name

- Snippet id: "json.type_name"
- Kind: "fn"
- Function name: "json_type_name"
- Parameter: name="value" type="Json"
- Returns: type="String"
- Body: ONE match step (id="s1") with 6 cases:
  - case variant type="Json::Null" -> step s1a return lit="null"
  - case variant type="Json::Bool" -> step s1b return lit="boolean"
  - case variant type="Json::Number" -> step s1c return lit="number"
  - case variant type="Json::String" -> step s1d return lit="string"
  - case variant type="Json::Array" -> step s1e return lit="array"
  - case variant type="Json::Object" -> step s1f return lit="object"
- Match step ends with as="_"
- End with `// END` comment after the final `end`

Generate only the Covenant code, no explanations.
```

### Prompt 4c: Match with Bindings and Wildcard

```
You are a code generator for Covenant, a machine-first programming language. Generate valid Covenant code following the grammar and rules below.

=== COVENANT GRAMMAR (EBNF) ===

snippet        = "snippet" "id" "=" STRING "kind" "=" "fn" { section } "end" ;
section        = signature_section | body_section ;

signature_section = "signature" fn_signature "end" ;
fn_signature   = "fn" "name" "=" STRING param_decl returns_decl "end" ;
param_decl     = "param" "name" "=" STRING "type" "=" type_ref ;
returns_decl   = "returns" "type" "=" type_ref "optional" ;

body_section   = "body" { step } "end" ;
step           = "step" "id" "=" STRING "kind" "=" step_kind step_body "as" "=" STRING "end" ;

(* Match Step *)
match_step     = "on" "=" STRING { match_case } ;
match_case     = "case" pattern { step } "end" ;
pattern        = "variant" "type" "=" STRING [ "bindings" "=" "(" STRING ")" ] | "wildcard" ;

(* Return Step *)
return_step    = "from" "=" STRING | "lit" "=" "none" ;

=== CRITICAL RULES ===

1. EVERY BLOCK NEEDS `end`: snippet...end, signature...end, fn...end, body...end, step...end, case...end
2. ALWAYS use `as="..."` with EQUALS SIGN
3. Match step ends with as="_" AFTER all cases
4. bindings=("varname") extracts the variant's data into a variable
5. wildcard is the catch-all default case (no "type" attribute)
6. Optional returns use: returns type="T" optional
7. Return none with: lit=none (no quotes around none)
8. MUST end snippet with `// END` comment on the line after the final `end`

=== TEMPLATE ===

snippet id="..." kind="fn"

signature
  fn name="..."
    param name="..." type="..."
    returns type="..." optional
  end
end

body
  step id="s1" kind="match"
    on="..."
    case variant type="..." bindings=("extracted_var")
      step id="s1a" kind="return"
        from="extracted_var"
        as="_"
      end
    end
    case wildcard
      step id="s1b" kind="return"
        lit=none
        as="_"
      end
    end
    as="_"
  end
end

end
// END

=== TASK ===

Generate ONE function snippet: json.get_string

- Snippet id: "json.get_string"
- Kind: "fn"
- Function name: "get_string"
- Parameter: name="value" type="Json"
- Returns: type="String" optional
- Body: ONE match step (id="s1") with 2 cases:
  - case variant type="Json::String" bindings=("s") -> step s1a return from="s"
  - case wildcard -> step s1b return lit=none
- Match step ends with as="_"
- End with `// END` comment after the final `end`

Generate only the Covenant code, no explanations.
```

**Expected Output for Prompt 4a**:
```covenant
snippet id="json.Json" kind="enum"

signature
  enum name="Json"
    variant name="Null"
    end
    variant name="Bool"
      field name="value" type="Bool"
    end
    variant name="Number"
      field name="value" type="Float"
    end
    variant name="String"
      field name="value" type="String"
    end
    variant name="Array"
      field name="items" type="Json[]"
    end
    variant name="Object"
      field name="fields" type="Map<String, Json>"
    end
  end
end

end
// END
```

**Expected Output for Prompt 4b**:
```covenant
snippet id="json.type_name" kind="fn"

signature
  fn name="json_type_name"
    param name="value" type="Json"
    returns type="String"
  end
end

body
  step id="s1" kind="match"
    on="value"
    case variant type="Json::Null"
      step id="s1a" kind="return"
        lit="null"
        as="_"
      end
    end
    case variant type="Json::Bool"
      step id="s1b" kind="return"
        lit="boolean"
        as="_"
      end
    end
    case variant type="Json::Number"
      step id="s1c" kind="return"
        lit="number"
        as="_"
      end
    end
    case variant type="Json::String"
      step id="s1d" kind="return"
        lit="string"
        as="_"
      end
    end
    case variant type="Json::Array"
      step id="s1e" kind="return"
        lit="array"
        as="_"
      end
    end
    case variant type="Json::Object"
      step id="s1f" kind="return"
        lit="object"
        as="_"
      end
    end
    as="_"
  end
end

end
// END
```

**Expected Output for Prompt 4c**:
```covenant
snippet id="json.get_string" kind="fn"

signature
  fn name="get_string"
    param name="value" type="Json"
    returns type="String" optional
  end
end

body
  step id="s1" kind="match"
    on="value"
    case variant type="Json::String" bindings=("s")
      step id="s1a" kind="return"
        from="s"
        as="_"
      end
    end
    case wildcard
      step id="s1b" kind="return"
        lit=none
        as="_"
      end
    end
    as="_"
  end
end

end
// END
```

---

## Token Estimates

| Prompt | Grammar | Template | Task | Total Input | Expected Output |
|--------|---------|----------|------|-------------|-----------------|
| 1: Hello World | ~800 | ~150 | ~150 | ~1,100 | ~150 |
| 2: Project Queries | ~800 | ~200 | ~350 | ~1,350 | ~600 |
| 3a: PostgreSQL CTE | ~500 | ~300 | ~300 | ~1,100 | ~400 |
| 3b: SQL Server | ~400 | ~250 | ~200 | ~850 | ~300 |
| 4a: Enum Definition | ~300 | ~150 | ~200 | ~650 | ~250 |
| 4b: Match Cases | ~400 | ~250 | ~250 | ~900 | ~450 |
| 4c: Match Bindings | ~400 | ~250 | ~200 | ~850 | ~200 |

All prompts are under 1,500 tokens input, leaving massive headroom in a 32K context window.

## Key Improvements in V2 Prompts

1. **One snippet per prompt** - Eliminates context accumulation errors
2. **Explicit template** - Shows exact nesting structure to copy
3. **"CRITICAL RULES" section** - Emphasizes `end` keywords and `as="..."` syntax
4. **Simplified grammar** - Only includes rules relevant to that specific task
5. **Shorter SQL** - Reduced SQL complexity to avoid attention drift
6. **Explicit step counts** - "TWO steps: query then return"
7. **Forced `// END` terminator** - Comment after final `end` prevents truncation issues

## Usage Notes

1. **Grammar Subsetting**: Each prompt includes only the grammar rules relevant to that example. This reduces noise and focuses the model on applicable syntax.

2. **Key Rules Section**: Explicitly states the most important constraints to prevent common errors (canonical ordering, quoting, effect requirements).

3. **Structured Task**: The task description mirrors the target output structure, making it easier for the model to generate correct code.

4. **No Ambiguity**: All identifiers, types, and structure are specified exactly - no room for creative interpretation.

5. **Evaluation**: Compare generated output against expected output for:
   - Syntax correctness (parseable)
   - Structural match (same sections, steps)
   - Semantic equivalence (same behavior)
