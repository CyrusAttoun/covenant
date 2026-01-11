# Covenant LLM Prompt Templates

Reusable prompt templates for common code generation tasks.

---

## Template Structure

Each template follows this structure:
1. **System prompt** — Core context (spec + examples)
2. **Task prompt** — Specific generation task
3. **Output format** — Expected structure

---

## System Prompt (Base Context)

```
You are a Covenant code generator. Covenant is a machine-first IR designed for LLM generation.

CORE RULES:
1. No operators — use keywords: add, equals, and, or, not
2. No expression nesting — one operation per step (SSA form)
3. Canonical ordering — sections always in order: effects, requires, types, tools, signature, body, tests, metadata
4. Every node has an ID — format: "module.name" or "M-001"
5. Effects are transitive — if you call a function with effects, declare them
6. Pattern matches must be exhaustive — cover all union variants
7. CRUD operations (insert/update/delete) are for Covenant types only
8. SQL queries use dialect blocks with opaque body sections

{{CONDENSED_SPEC}}

{{SELECTED_EXAMPLES}}

Generate valid Covenant code following these rules exactly.
```

---

## Template 1: Simple Function Generation

**Use case:** Generate a pure function with no effects

**Prompt:**
```
Generate a Covenant function with the following specification:

Module: {{module_name}}
Function: {{function_name}}
Description: {{description}}
Parameters:
{{#each parameters}}
  - {{name}}: {{type}} {{#if description}}({{description}}){{/if}}
{{/each}}
Returns: {{return_type}}

Requirements:
{{#each requirements}}
  - [{{priority}}] {{text}}
{{/each}}

The function should have no side effects (no effects section needed).
Generate complete snippet with signature and body sections.
Include at least one test that covers the primary requirement.
```

**Example instantiation:**
```
Generate a Covenant function with the following specification:

Module: math
Function: factorial
Description: Calculate factorial of a positive integer
Parameters:
  - n: Int (the number to calculate factorial for)
Returns: Int

Requirements:
  - [high] Must return 1 for n=0
  - [high] Must return n * factorial(n-1) for n>0
  - [medium] Should handle large numbers correctly

The function should have no side effects (no effects section needed).
Generate complete snippet with signature and body sections.
Include at least one test that covers the primary requirement.
```

---

## Template 2: CRUD Function Generation

**Use case:** Generate database operation functions

**Prompt:**
```
Generate a Covenant CRUD function with the following specification:

Module: {{module_name}}
Function: {{function_name}}
Operation: {{operation_type}}  // create, read, update, delete
Description: {{description}}

Database Target: {{db_target}}  // e.g., "app_db" or "project.users"
SQL Dialect: {{sql_dialect}}    // "covenant" or "postgres" or "mysql" etc.

Parameters:
{{#each parameters}}
  - {{name}}: {{type}}
{{/each}}

Returns: {{return_type}}  // Should be a union with error type

Requirements:
{{#each requirements}}
  - [{{priority}}] {{text}}
{{/each}}

Generate complete snippet with:
1. effects section (include "database" effect)
2. signature section with parameters and return type
3. body section with appropriate query/insert/update/delete steps
4. Error handling for database operations
5. At least one unit test
```

**Example instantiation:**
```
Generate a Covenant CRUD function with the following specification:

Module: user
Function: get_by_email
Operation: read
Description: Retrieve a user by their email address

Database Target: app_db
SQL Dialect: postgres

Parameters:
  - email: String

Returns: union of User (optional) and DbError

Requirements:
  - [critical] Must query the users table by email field
  - [high] Must return none if user not found
  - [high] Must return DbError on database failure

Generate complete snippet with:
1. effects section (include "database" effect)
2. signature section with parameters and return type
3. body section with appropriate query steps
4. Error handling for database operations
5. At least one unit test
```

---

## Template 3: Complex Function with Multiple Steps

**Use case:** Multi-step business logic with effects

**Prompt:**
```
Generate a Covenant function with the following specification:

Module: {{module_name}}
Function: {{function_name}}
Description: {{description}}

Effects needed:
{{#each effects}}
  - {{effect_name}}{{#if description}} ({{description}}){{/if}}
{{/each}}

Parameters:
{{#each parameters}}
  - {{name}}: {{type}}
{{/each}}

Returns: {{return_type}}

Logic flow:
{{#each steps}}
{{step_number}}. {{step_description}}
{{/each}}

Requirements:
{{#each requirements}}
  - [{{priority}}] {{text}}
{{/each}}

Generate complete snippet with:
1. effects section declaring all needed effects
2. signature section
3. body section with step-by-step implementation (SSA form)
4. Error handling using match or handle blocks
5. Tests covering main scenarios
```

**Example instantiation:**
```
Generate a Covenant function with the following specification:

Module: order
Function: process_payment
Description: Process a payment for an order and update order status

Effects needed:
  - database (for order status update)
  - network (for payment API call)

Parameters:
  - order_id: Int
  - amount: Float
  - payment_method: String

Returns: union of PaymentReceipt and PaymentError

Logic flow:
1. Validate amount is positive
2. Call external payment API
3. Handle payment result (success/failure)
4. If successful, update order status to "paid"
5. Return receipt or error

Requirements:
  - [critical] Must validate amount > 0 before API call
  - [critical] Must update order status only on successful payment
  - [high] Must return meaningful error messages
  - [medium] Should log payment attempts

Generate complete snippet with:
1. effects section declaring all needed effects
2. signature section
3. body section with step-by-step implementation (SSA form)
4. Error handling using match or handle blocks
5. Tests covering main scenarios
```

---

## Template 4: Type Definition Generation

**Use case:** Generate struct or enum types

**Prompt:**
```
Generate a Covenant type definition with the following specification:

Type kind: {{type_kind}}  // struct or enum
Name: {{type_name}}
Description: {{description}}

{{#if is_struct}}
Fields:
{{#each fields}}
  - {{name}}: {{type}}{{#if optional}} (optional){{/if}}{{#if description}} - {{description}}{{/if}}
{{/each}}
{{/if}}

{{#if is_enum}}
Variants:
{{#each variants}}
  - {{name}}{{#if has_data}}({{data_description}}){{/if}}
{{/each}}
{{/if}}

Generate complete snippet with:
1. signature section defining the type structure
2. metadata section with description
{{#if needs_examples}}
3. Include usage examples in notes
{{/if}}
```

**Example instantiation:**
```
Generate a Covenant type definition with the following specification:

Type kind: enum
Name: PaymentStatus
Description: Represents the status of a payment transaction

Variants:
  - Pending - Payment initiated but not confirmed
  - Completed(amount: Float, receipt_id: String) - Payment successful
  - Failed(error_code: String, message: String) - Payment failed
  - Refunded(amount: Float, reason: String) - Payment was refunded

Generate complete snippet with:
1. signature section defining the type structure
2. metadata section with description
3. Include usage examples in notes
```

---

## Template 5: Migration from Imperative Code

**Use case:** Translate Python/JS/TS to Covenant

**Prompt:**
```
Translate the following {{source_language}} code to Covenant IR:

```{{source_language}}
{{source_code}}
```

Requirements:
1. Preserve all functionality exactly
2. Identify and declare all effects (database, network, filesystem, etc.)
3. Convert expressions to SSA form (one operation per step)
4. Replace operators with keywords (==→equals, +→add, etc.)
5. Add error handling using union types and match
6. Add at least one requirement and one test
7. Use {{#if sql_dialect}}{{sql_dialect}}{{else}}Covenant{{/if}} dialect for queries

Context:
{{#if additional_context}}
{{additional_context}}
{{/if}}

Generate complete Covenant snippet.
```

**Example instantiation:**
```
Translate the following Python code to Covenant IR:

```python
def calculate_discount(price, discount_percent):
    if discount_percent < 0 or discount_percent > 100:
        raise ValueError("Discount must be between 0 and 100")

    discount_amount = price * (discount_percent / 100)
    final_price = price - discount_amount

    return {
        "original": price,
        "discount": discount_amount,
        "final": final_price
    }
```

Requirements:
1. Preserve all functionality exactly
2. Identify and declare all effects (none for pure function)
3. Convert expressions to SSA form (one operation per step)
4. Replace operators with keywords (==→equals, +→add, etc.)
5. Add error handling using union types and match
6. Add at least one requirement and one test
7. Use Covenant dialect for queries (none needed here)

Context:
This is a pure function for e-commerce pricing calculations.

Generate complete Covenant snippet.
```

---

## Template 6: Error Recovery / Self-Correction

**Use case:** Fix code based on compiler errors

**Prompt:**
```
The following Covenant code has compilation errors. Fix them.

ORIGINAL CODE:
```
{{original_code}}
```

COMPILER ERRORS:
{{#each errors}}
{{error_code}}: {{error_message}}
Location: {{location}}
{{#if auto_fix_suggestion}}
Suggested fix: {{auto_fix_suggestion}}
{{/if}}
{{/each}}

Generate corrected Covenant code that fixes all errors.
Preserve all functionality while fixing:
- Effect transitivity violations
- Pattern match exhaustiveness
- Canonical ordering issues
- SSA form violations
- Type mismatches
```

---

## Template 7: Test Generation

**Use case:** Generate tests for existing function

**Prompt:**
```
Generate comprehensive tests for the following Covenant function:

```
{{function_code}}
```

Generate test cases covering:
1. Happy path / normal execution
2. Edge cases (empty inputs, boundary values, etc.)
3. Error cases (if function returns union with error types)
4. Requirement coverage (reference requirement IDs)

Test kinds to include:
- unit tests (at least 3)
{{#if needs_property_tests}}
- property tests (at least 1)
{{/if}}
{{#if needs_integration_tests}}
- integration tests
{{/if}}

Add tests to the tests section of the snippet.
Each test should have unique ID and reference covered requirements.
```

---

## Template 8: Database Schema to Binding

**Use case:** Generate database snippet from schema

**Prompt:**
```
Generate a Covenant database binding for the following schema:

Database dialect: {{dialect}}  // postgres, mysql, sqlserver, sqlite
Connection string: {{connection_string}}

Tables:
{{#each tables}}
Table: {{table_name}}
{{#each columns}}
  - {{column_name}}: {{column_type}}{{#if is_primary_key}} (PRIMARY KEY){{/if}}{{#if is_nullable}} (NULLABLE){{/if}}
{{/each}}
{{#if foreign_keys}}
Foreign Keys:
{{#each foreign_keys}}
  - {{from_column}} → {{to_table}}.{{to_column}}
{{/each}}
{{/if}}
{{/each}}

Generate complete database snippet with:
1. kind="database"
2. metadata section with dialect and connection
3. schema section with all tables and fields
4. Correct type mappings (SQL types → Covenant types)
```

---

## Template 9: Query Optimization

**Use case:** Generate efficient queries with cost awareness

**Prompt:**
```
Generate an optimized Covenant query with the following specification:

Query purpose: {{purpose}}
Data source: {{data_source}}
Expected result size: {{expected_size}}  // small (<100), medium (100-10k), large (>10k)
Performance requirements: {{performance_requirements}}

Selection criteria:
{{#each criteria}}
  - {{criterion}}
{{/each}}

Optimization constraints:
- Maximum cost budget: {{cost_budget}}  // O(1), O(log n), O(n), O(n log n)
- Use indexes on: {{indexed_fields}}
- Avoid: {{avoid_patterns}}  // e.g., "nested loops", "full table scans"

Generate query step with:
1. Appropriate select (specific fields vs all)
2. Efficient where conditions (indexed fields first)
3. Proper ordering and limits
4. Cost-aware approach
```

---

## Template 10: Refactoring

**Use case:** Refactor existing code

**Prompt:**
```
Refactor the following Covenant code according to these requirements:

ORIGINAL CODE:
```
{{original_code}}
```

REFACTORING GOALS:
{{#each goals}}
  - {{goal}}
{{/each}}

CONSTRAINTS:
- Preserve all functionality
- Maintain all tests
- Keep effect declarations accurate
- Follow SSA form strictly

{{#if use_refactor_block}}
Generate a refactor block with multiple update_snippet steps.
{{else}}
Generate the refactored snippet directly.
{{/if}}
```

---

## Context Size Guidelines

**For each generation:**
- Base system prompt: ~500 tokens
- Condensed spec: ~2,800 tokens
- Selected examples: ~1,000-1,500 tokens (2-3 examples)
- Task prompt: ~200-500 tokens
- **Total input**: ~4,500-5,300 tokens

**Budget for output:**
- Simple function: ~300-600 tokens
- Medium function: ~600-1,200 tokens
- Complex function: ~1,200-2,500 tokens

**Total per generation**: ~5,000-8,000 tokens (well within 200k context)

---

## Example Selection Strategy

Choose examples based on task type:

| Task Type | Recommended Examples |
|-----------|---------------------|
| Pure function | 02-pure-functions.cov, 09-pattern-matching.cov |
| CRUD operation | 16-database-dialects.cov, 17-advanced-sql.cov |
| Error handling | 04-error-handling.cov, 09-pattern-matching.cov |
| Effects | 03-effects.cov, 04-error-handling.cov |
| Types | 05-types.cov, 06-enums.cov |
| Tests | 02-pure-functions.cov (has good test examples) |
| Query | 11-project-queries.cov, 17-advanced-sql.cov |
| Migration | Closest example to source language patterns |

---

## Quality Validation Prompts

After generation, optionally run validation:

```
Review the generated Covenant code and check for:

1. Effect transitivity: Are all effects from called functions declared?
2. Pattern exhaustiveness: Are all union variants covered in matches?
3. SSA form: Are variable names unique (no redefinitions)?
4. Canonical ordering: Are sections in correct order?
5. Type safety: Do all operations use compatible types?
6. Error handling: Are error cases properly handled?

If any issues found, provide corrected version.
```

---

## Metrics to Collect

For each generation, track:
- Input tokens
- Output tokens
- Generation time
- First-pass success (compiles without errors)
- Error codes if failed
- Self-correction attempts needed
- Final success status

Store in JSONL format:
```json
{
  "timestamp": "2026-01-10T12:34:56Z",
  "template": "simple_function",
  "input_tokens": 4823,
  "output_tokens": 456,
  "duration_ms": 2341,
  "first_pass_success": true,
  "compiler_errors": [],
  "correction_rounds": 0,
  "final_success": true,
  "cost_usd": 0.087
}
```
