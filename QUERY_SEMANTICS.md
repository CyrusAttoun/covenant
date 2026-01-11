# Covenant Query Semantics

Formal operational semantics for Covenant's dialect-based query system.

---

## Table of Contents

1. [Overview](#overview)
2. [Query Types](#query-types)
3. [Formal Semantics](#formal-semantics)
4. [Database Queries](#database-queries)
5. [Project Queries](#project-queries)
6. [Cost Model](#cost-model)
7. [Caching & Memoization](#caching--memoization)
8. [Null Handling](#null-handling)
9. [Query Optimization](#query-optimization)
10. [Examples](#examples)
11. [SQL Dialect Queries](#15-sql-dialect-queries)

---

## Overview

Covenant uses a **dialect-based query system** with two paths:

1. **Covenant Dialect** (default) — Strongly-typed, minimal syntax for Covenant types
2. **SQL Dialects** — Opaque body blocks for full SQL power on external databases

The `dialect` attribute determines which path is used:

- `target="app_db"` → Compiles to SQL, executes against database
- `target="project"` → Compiles to AST traversal, executes against symbol graph

**Key Properties:**

| Property | Database Queries | Project Queries |
|----------|------------------|-----------------|
| **Purity** | No (side effects via insert/update/delete) | Yes (read-only) |
| **Determinism** | Depends on isolation level | Guaranteed (same graph → same results) |
| **Caching** | No (data may change externally) | Yes (memoized with version-based invalidation) |
| **Cost** | Delegated to DB optimizer | O(n) linear scan, O(n²) joins |
| **Ordering** | Unspecified unless `order_by` | Lexicographic by ID (deterministic) |

---

## Query Types

### Read Queries

**Syntax:**
```
step id="s1" kind="query"
  target="<database_name | project>"
  select <all | field list>
  from="<table | node_type>"
  [where <condition>]
  [join ...]
  [order_by ...]
  [limit=N offset=M]
  as="<result_binding>"
end
```

**Semantics:**
- Returns: `array<T>` where T is the row/node type
- Side effects: None (read-only)
- Idempotent: Yes

### Write Queries

**Insert:**
```
step id="s1" kind="insert"
  into="<target>.<table>"
  set field="<name>" from="<var>"
  ...
  as="<inserted_row>"
end
```

**Update:**
```
step id="s2" kind="update"
  target="<target>.<table>"
  set field="<name>" from="<var>"
  where <condition>
  as="<updated_count>"
end
```

**Delete:**
```
step id="s3" kind="delete"
  from="<target>.<table>"
  where <condition>
  as="<deleted_count>"
end
```

**Semantics:**
- Returns: Inserted row, updated count, or deleted count
- Side effects: Modifies database state
- Idempotent: No (unless wrapped in transaction with idempotency key)

---

## Formal Semantics

### Notation

- `Γ` = Symbol graph (set of symbols with metadata)
- `Γ[v]` = Symbol with ID `v` in graph Γ
- `σ` = Variable environment (mapping from var names to values)
- `q` = Query AST
- `⟦q⟧(Γ, σ)` = Evaluation of query q in context (Γ, σ)

### Denotational Semantics

**Select All:**
```
⟦select all from="T"⟧(Γ, σ) = { s ∈ Γ | s.kind = T }
```

**Select Fields:**
```
⟦select field="f1" field="f2" from="T"⟧(Γ, σ) =
  { { f1: s.f1, f2: s.f2 } | s ∈ Γ, s.kind = T }
```

**Where Clause:**
```
⟦select all from="T" where c end⟧(Γ, σ) =
  { s ∈ Γ | s.kind = T ∧ ⟦c⟧(s, σ) = true }
```

**Where Condition:**
```
⟦equals field="f" var="v"⟧(s, σ) = (s.f = σ[v])
⟦equals field="f" lit=L⟧(s, σ) = (s.f = L)
⟦contains field="f" lit=L⟧(s, σ) = (L ∈ s.f)  // f is a collection
⟦and c1 c2 end⟧(s, σ) = ⟦c1⟧(s, σ) ∧ ⟦c2⟧(s, σ)
⟦or c1 c2 end⟧(s, σ) = ⟦c1⟧(s, σ) ∨ ⟦c2⟧(s, σ)
⟦not c⟧(s, σ) = ¬⟦c⟧(s, σ)
```

**Order By:**
```
⟦... order by="f" dir="asc"⟧(Γ, σ) =
  sort(⟦...⟧(Γ, σ), key=λs.s.f, reverse=false)
```

**Limit/Offset:**
```
⟦... limit=N offset=M⟧(Γ, σ) =
  ⟦...⟧(Γ, σ)[M : M+N]  // Slice notation
```

**Join (Nested Loop):**
```
⟦select ... from="T1" join type="inner" table="T2" on c⟧(Γ, σ) =
  { merge(s1, s2) | s1 ∈ Γ, s1.kind = T1,
                     s2 ∈ Γ, s2.kind = T2,
                     ⟦c⟧(merge(s1, s2), σ) = true }
```

---

## Database Queries

### Compilation to SQL

Database queries compile to SQL using a straightforward AST → SQL transformation:

**Example IR:**
```
step id="s1" kind="query"
  target="app_db"
  select all
  from="users"
  where
    and
      equals field="is_active" lit=true
      greater field="created_at" var="cutoff_date"
    end
  end
  order by="created_at" dir="desc"
  limit=10
  as="recent_users"
end
```

**Compiled SQL:**
```sql
SELECT *
FROM users
WHERE is_active = TRUE
  AND created_at > :cutoff_date
ORDER BY created_at DESC
LIMIT 10
```

### Parameterization

Variables in queries become SQL parameters (prevents injection):

```
var="user_id" → :user_id (parameterized)
lit="admin"   → 'admin' (literal)
lit=42        → 42 (literal)
```

### Null Handling

Covenant's `none` maps to SQL `NULL`:

```
equals field="deleted_at" lit=none
  ↓
WHERE deleted_at IS NULL

not_equals field="deleted_at" lit=none
  ↓
WHERE deleted_at IS NOT NULL
```

**Three-valued logic:**
- `NULL = NULL` → UNKNOWN (row excluded)
- Use explicit `IS NULL` / `IS NOT NULL` checks

### Transactions

Wrapped in `step kind="transaction"`:

```
step id="s1" kind="transaction"
  isolation="serializable"

  step id="s1a" kind="insert"
    into="app_db.orders"
    set field="user_id" from="user_id"
    as="order"
  end

  step id="s1b" kind="update"
    target="app_db.inventory"
    set field="quantity" op=sub from="1"
    where equals field="product_id" var="product_id" end
    as="updated"
  end

  as="transaction_result"
end
```

**Compiled SQL:**
```sql
BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE;

INSERT INTO orders (user_id) VALUES (:user_id) RETURNING *;

UPDATE inventory
SET quantity = quantity - 1
WHERE product_id = :product_id;

COMMIT;
```

**Rollback on error:**
If any step fails → `ROLLBACK` entire transaction

### Isolation Levels

| Level | Read Phenomena | Use Case |
|-------|----------------|----------|
| `read_uncommitted` | Dirty reads, non-repeatable reads, phantoms | Rarely used, max performance |
| `read_committed` | Non-repeatable reads, phantoms | **Default**, most common |
| `repeatable_read` | Phantoms | Strong consistency |
| `serializable` | None | Maximum consistency, lowest concurrency |

---

## Project Queries

### Symbol Graph Structure

Project queries execute against the **symbol graph**, a derived data structure computed by the compiler (see [COMPILER.md](COMPILER.md), Phase 2).

**Schema:**
```
Symbol = {
  id: String,
  kind: "fn" | "struct" | "enum" | "module" | "database" | "extern",
  location: SourceLocation,

  // Forward references (extracted from AST)
  calls: [SymbolId],
  references: [SymbolId],

  // Backward references (computed)
  called_by: [SymbolId],
  referenced_by: [SymbolId],

  // Effect metadata (Phase 3)
  effects: [EffectId],
  effect_closure: [EffectId],

  // Requirement/test linkage (Phase 5)
  requirements: [ReqId],
  tests: [TestId],
  covered_by: [TestId]
}
```

### Queryable Node Types

| Node Type | Description | Fields |
|-----------|-------------|--------|
| `functions` | All function snippets | `id`, `name`, `effects`, `calls`, `called_by`, ... |
| `structs` | Struct definitions | `id`, `name`, `fields`, `referenced_by`, ... |
| `enums` | Enum definitions | `id`, `name`, `variants`, ... |
| `requirements` | Requirements from `requires` sections | `id`, `text`, `priority`, `status`, `covered_by` |
| `tests` | Tests from `tests` sections | `id`, `kind`, `covers` |
| `steps` | Individual steps within bodies | `id`, `kind`, `inputs`, `type`, ... |

### Query Compilation

Project queries compile to **symbol graph traversal** code:

**Example IR:**
```
step id="s1" kind="query"
  target="project"
  select all
  from="functions"
  where
    contains field="effects" lit="database"
  end
  as="db_functions"
end
```

**Compiled to (pseudo-code):**
```python
def query_s1(symbol_graph, env):
    results = []

    for symbol in symbol_graph.symbols.values():
        if symbol.kind == "fn":
            if "database" in symbol.effect_closure:
                results.append(symbol)

    # Deterministic ordering (lexicographic by ID)
    results.sort(key=lambda s: s.id)

    return results
```

### Determinism Guarantee

**Theorem:** Project queries are deterministic.

**Proof sketch:**
1. Symbol graph is immutable within a compilation pass (atomic updates, see DESIGN.md section 15.1)
2. Query semantics are pure (no side effects)
3. Ordering is lexicographic by ID (no undefined behavior)
4. Therefore: Same graph Γ + same environment σ → same results

**Corollary:** Project queries are safe to memoize.

### Bidirectional Queries

Leverage forward and backward references:

**Find all callers of `authenticate`:**
```
step id="s1" kind="query"
  target="project"
  select field="called_by"
  from="functions"
  where
    equals field="id" lit="auth.authenticate"
  end
  as="callers"
end
```

Returns:
```json
[
  {"called_by": ["main.login", "api.verify_token", "middleware.auth_check"]}
]
```

**Alternative (using join):**
```
step id="s1" kind="query"
  target="project"
  select all
  from="functions"
  where
    contains field="calls" lit="auth.authenticate"
  end
  as="callers"
end
```

---

## Cost Model

### Complexity Analysis

**Select All (Linear Scan):**
```
⟦select all from="T"⟧ → O(|Γ|)
```
Iterates over all symbols in graph.

**Where Clause (Linear Scan + Filter):**
```
⟦select all from="T" where c⟧ → O(|Γ| × cost(c))
```
Filter cost depends on predicate complexity:
- `equals field="f" lit=L` → O(1)
- `contains field="f" lit=L` → O(|f|) if f is a collection
- `and c1 c2` → cost(c1) + cost(c2)

**Join (Nested Loop):**
```
⟦... join table="T2" on c⟧ → O(|T1| × |T2| × cost(c))
```
No indexes yet → quadratic in worst case

**Order By (Sort):**
```
⟦... order by="f"⟧ → O(|results| × log |results|)
```

**Limit/Offset:**
```
⟦... limit=N offset=M⟧ → O(|results|)
```
Applied after filtering and sorting.

### Cost Hints

Queries declare expected cost via metadata:

```
metadata
  cost_hint=cheap     // <10ms
  cost_hint=moderate  // <100ms
  cost_hint=expensive // <10s
end
```

**Compiler enforcement:**
- Static analysis estimates query cost
- If estimated cost exceeds hint → `E-QUERY-001`
- Override with `timeout=30s` for known expensive queries

**Example violation:**
```
snippet id="find_refs" kind="fn"
  metadata
    cost_hint=cheap  // Claims <10ms
  end

  body
    step id="s1" kind="query"
      target="project"
      select all
      from="functions"
      join type="inner" table="symbols" on
        contains field="calls" field="id"
      end  // O(N²) join exceeds 10ms for N > 1000
      as="refs"
    end
  end
end
```

Compiler error:
```json
{
  "code": "E-QUERY-001",
  "message": "Query exceeds cost budget",
  "estimated_cost": "O(N²)",
  "declared_hint": "cheap",
  "suggestion": "Change cost_hint to 'expensive' or optimize query"
}
```

---

## Caching & Memoization

### Project Query Memoization

**Cache Key:**
```
cache_key = hash(query_AST, symbol_graph.version)
```

**Invalidation:**
```python
class QueryCache:
    def __init__(self):
        self.cache = {}  # key → results
        self.version = 0

    def get(self, query_ast, symbol_graph_version):
        key = hash((query_ast, symbol_graph_version))
        return self.cache.get(key)

    def put(self, query_ast, symbol_graph_version, results):
        key = hash((query_ast, symbol_graph_version))
        self.cache[key] = results

    def invalidate_all(self):
        # Called when symbol graph version is bumped
        self.cache.clear()
```

**Version Bump Triggers:**
- Any snippet modification (see DESIGN.md section 15.1)
- Snippet deletion
- Refactor block execution

**Cache Hit Performance:**
Memoization provides:
- **100x speedup** for repeated queries
- IDE responsiveness: hover/go-to-def <10ms
- Incremental compilation: only recompute affected queries

### Database Query Non-Caching

Database queries are **never cached** because:
1. Data may change outside compiler's control
2. Side effects (insert/update/delete) preclude safe caching
3. Database already has internal query cache

Exception: Read-only snapshot queries with explicit cache pragma (future feature)

---

## Null Handling

### Three-Valued Logic

Covenant follows SQL's three-valued logic for null handling:

| Expression | Value when x = NULL |
|------------|---------------------|
| `x = y` | UNKNOWN (unless y = NULL, then UNKNOWN) |
| `x IS NULL` | TRUE |
| `x IS NOT NULL` | FALSE |
| `NOT (x = y)` | UNKNOWN |
| `x = y OR z = w` | TRUE if z = w, else UNKNOWN |
| `x = y AND z = w` | FALSE if z ≠ w, else UNKNOWN |

**UNKNOWN in WHERE clause:**
Rows where condition evaluates to UNKNOWN are **excluded** (treated as FALSE).

### Null Propagation

**Arithmetic:**
```
add(null, 5) → null
mul(null, 10) → null
```

**Logic:**
```
and(null, true) → null
or(null, false) → null
not(null) → null
```

**String:**
```
concat(null, "hello") → null
```

### Optional Types

`optional` is syntactic sugar for union with `None`:

```
type="User" optional
  ↓ (desugars to)
union
  type="User"
  type="None"
end
```

**Handling optional in queries:**
```
// Find users without emails
step id="s1" kind="query"
  target="app_db"
  select all
  from="users"
  where
    equals field="email" lit=none
  end
  as="users_without_email"
end
```

---

## Query Optimization

### Current Limitations

**No indexes:**
- All project queries use linear scan
- Joins are nested loop (O(n²))
- No cost-based optimization

**Future: Index Support**
```
// Declare index on frequently queried field
metadata
  index on="effects"
  index on="calls"
end
```

Compiler would generate:
```python
# Hash index for O(1) lookup
effect_index = {
  "database": [symbol_id1, symbol_id2, ...],
  "network": [symbol_id3, ...]
}

# Query becomes O(1) instead of O(N)
results = effect_index.get("database", [])
```

### Filter Reordering

Compiler may reorder filters for efficiency:

**Before:**
```
where
  and
    contains field="effects" lit="database"  // Selective (few results)
    equals field="is_exported" lit=true      // Non-selective (many results)
  end
end
```

**After (optimized):**
```
where
  and
    contains field="effects" lit="database"  // Evaluate first (prunes many)
    equals field="is_exported" lit=true      // Evaluate second (fewer rows)
  end
end
```

**Selectivity estimation:**
- `contains field="effects"` → Assume 10% of symbols
- `equals field="is_exported"` → Assume 50% of symbols
- Evaluate selective filters first

### Join Elimination

Use bidirectional references instead of joins:

**Inefficient (join):**
```
select all
from="functions"
join type="inner" table="functions" on
  contains field="called_by" var="target_id"
end
```

**Efficient (direct reference):**
```
select all
from="functions"
where
  equals field="id" var="target_id"
end

// Then access: result[0].called_by
```

---

## Examples

### Example 1: Find Uncovered Requirements

**Query:**
```
step id="s1" kind="query"
  target="project"
  select all
  from="requirements"
  where
    equals field="covered_by" lit=[]
  end
  as="uncovered"
end
```

**Semantics:**
```
⟦select all from="requirements" where equals field="covered_by" lit=[]⟧(Γ, σ) =
  { r ∈ Γ | r.kind = "requirement" ∧ r.covered_by = [] }
```

**Results:**
```json
[
  {
    "id": "R-AUTH-002",
    "text": "Failed logins must be rate-limited",
    "priority": "high",
    "status": "approved",
    "covered_by": []
  }
]
```

---

### Example 2: Find Functions with Transitive Database Effect

**Query:**
```
step id="s1" kind="query"
  target="project"
  select field="id" field="effect_closure"
  from="functions"
  where
    contains field="effect_closure" lit="database"
  end
  order by="id" dir="asc"
  as="db_functions"
end
```

**Semantics:**
```
⟦...⟧(Γ, σ) =
  sort(
    { {id: s.id, effect_closure: s.effect_closure} |
      s ∈ Γ, s.kind = "fn", "database" ∈ s.effect_closure },
    key=λr.r.id
  )
```

**Results:**
```json
[
  {"id": "api.create_user", "effect_closure": ["database"]},
  {"id": "auth.login", "effect_closure": ["database", "network"]},
  {"id": "reports.generate", "effect_closure": ["database", "filesystem"]}
]
```

---

### Example 3: Find All Call Sites of a Function

**Query:**
```
step id="s1" kind="query"
  target="project"
  select all
  from="steps"
  where
    and
      equals field="kind" lit="call"
      equals field="fn" var="target_function"
    end
  end
  as="call_sites"
end
```

**With environment:**
```
σ = { target_function: "auth.authenticate" }
```

**Semantics:**
```
⟦...⟧(Γ, σ) =
  { s ∈ Γ | s.kind = "step" ∧ s.step_kind = "call" ∧ s.fn = "auth.authenticate" }
```

**Results:**
```json
[
  {
    "id": "s5",
    "kind": "call",
    "fn": "auth.authenticate",
    "location": {"file": "src/api.cov", "line": 42},
    "parent_function": "api.login"
  },
  {
    "id": "s12",
    "kind": "call",
    "fn": "auth.authenticate",
    "location": {"file": "src/middleware.cov", "line": 18},
    "parent_function": "middleware.auth_check"
  }
]
```

---

### Example 4: Database Query with Join

**Query:**
```
step id="s1" kind="query"
  target="app_db"
  select all
  from="orders"
  join type="inner" table="users" on
    equals field="orders.user_id" field="users.id"
  end
  where
    greater field="orders.created_at" var="start_date"
  end
  order by="orders.created_at" dir="desc"
  limit=10
  as="recent_orders"
end
```

**Compiled SQL:**
```sql
SELECT orders.*, users.*
FROM orders
INNER JOIN users ON orders.user_id = users.id
WHERE orders.created_at > :start_date
ORDER BY orders.created_at DESC
LIMIT 10
```

---

### Example 5: Transactional Insert + Update

**Query:**
```
step id="s1" kind="transaction"
  isolation="serializable"

  step id="s1a" kind="insert"
    into="app_db.orders"
    set field="user_id" from="user_id"
    set field="total" from="total"
    as="order"
  end

  step id="s1b" kind="update"
    target="app_db.users"
    set field="total_spent" op=add from="total"
    where
      equals field="id" var="user_id"
    end
    as="updated"
  end

  as="transaction_result"
end
```

**Compiled SQL:**
```sql
BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE;

INSERT INTO orders (user_id, total)
VALUES (:user_id, :total)
RETURNING *;

UPDATE users
SET total_spent = total_spent + :total
WHERE id = :user_id;

COMMIT;
```

**Rollback:**
If update fails (e.g., user doesn't exist) → entire transaction rolls back, order is not inserted.

---

## 15. SQL Dialect Queries

For external databases, Covenant supports opaque SQL blocks with full SQL power. The compiler does not parse SQL — it only validates parameter bindings.

### 15.1 Dialect Query Syntax

```
step id="s1" kind="query"
  dialect="postgres"
  target="app_db"
  body
    SELECT u.id, u.name, COUNT(o.id) as order_count
    FROM users u
    LEFT JOIN orders o ON o.user_id = u.id
    WHERE u.created_at > :cutoff
    GROUP BY u.id, u.name
    HAVING COUNT(o.id) > 5
    ORDER BY order_count DESC
  end
  params
    param name="cutoff" from="cutoff_date"
  end
  returns collection of="UserOrderStats"
  as="high_volume_users"
end
```

**Key points:**
- `dialect` is required for SQL queries (postgres, sqlserver, mysql, sqlite, etc.)
- `body ... end` contains raw SQL — treated as opaque text
- `params` section declares parameter bindings
- `returns` type annotation is required for type safety

### 15.2 Parameter Validation

The compiler validates that declared parameters match placeholders in the body:

**Placeholder syntax by dialect:**

| Dialect | Placeholder | Example |
|---------|-------------|---------|
| postgres | `:name` | `:user_id` |
| sqlserver | `@name` | `@user_id` |
| mysql | `?` | Positional (order matches params order) |
| sqlite | `:name` or `?` | Named or positional |

**Validation rules:**
1. Each placeholder in body must have a matching `param` declaration
2. Each `param` must have a corresponding placeholder in body
3. For positional dialects (mysql), params are bound in declaration order

**Errors:**
- **E-QUERY-020: Unmatched placeholder** — Placeholder in body with no param declaration
- **E-QUERY-021: Missing placeholder** — Param declared but no matching placeholder
- **E-QUERY-022: Missing returns** — Returns annotation required for dialect queries

### 15.3 Database Bindings

External databases are declared as `kind="database"` snippets:

```
snippet id="db.app_db" kind="database"

metadata
  type="database"
  dialect="postgres"
  connection="env:APP_DB_URL"
  version="14.0"
end

schema
  table name="users"
    field name="id" type="Int" primary_key=true
    field name="email" type="String"
    field name="created_at" type="DateTime"
  end

  table name="orders"
    field name="id" type="Int" primary_key=true
    field name="user_id" type="Int"
    field name="total" type="Decimal"
  end
end

end
```

**Validation:**
- Query's `target` must reference a declared database binding
- Query's `dialect` should match the binding's declared dialect (warning if mismatch)

### 15.4 CRUD via SQL Dialects

All database operations use the same body block syntax:

```
// INSERT
step id="s1" kind="query"
  dialect="postgres"
  target="app_db"
  body
    INSERT INTO users (name, email)
    VALUES (:name, :email)
    RETURNING id, name, email
  end
  params
    param name="name" from="user_name"
    param name="email" from="user_email"
  end
  returns type="User"
  as="new_user"
end

// UPDATE
step id="s2" kind="query"
  dialect="postgres"
  target="app_db"
  body
    UPDATE users
    SET is_active = false
    WHERE last_login < :cutoff
    RETURNING id
  end
  params
    param name="cutoff" from="cutoff_date"
  end
  returns collection of="Int"
  as="deactivated_ids"
end

// DELETE
step id="s3" kind="query"
  dialect="postgres"
  target="app_db"
  body
    DELETE FROM users
    WHERE id = :user_id
    RETURNING id
  end
  params
    param name="user_id" from="target_user"
  end
  returns type="Int" optional
  as="deleted_id"
end
```

### 15.5 Transactions

For transactional operations, wrap SQL queries in a transaction step:

```
step id="s1" kind="transaction"
  isolation="serializable"

  step id="s1a" kind="query"
    dialect="postgres"
    target="app_db"
    body
      INSERT INTO orders (user_id, total)
      VALUES (:user_id, :total)
      RETURNING id
    end
    params
      param name="user_id" from="uid"
      param name="total" from="order_total"
    end
    returns type="Int"
    as="order_id"
  end

  step id="s1b" kind="query"
    dialect="postgres"
    target="app_db"
    body
      UPDATE users
      SET total_spent = total_spent + :amount
      WHERE id = :user_id
    end
    params
      param name="amount" from="order_total"
      param name="user_id" from="uid"
    end
    returns type="Int"
    as="updated"
  end

  as="transaction_result"
end
```

### 15.6 Future: Pluggable Dialect Validation

Future versions may support pluggable grammar/LSP for dialect-specific validation:

- IDE integration for SQL syntax highlighting within body blocks
- LSP-based autocomplete for SQL keywords and table/column names
- Optional SQL parsing for static analysis
- Custom dialects (lancedb, duckdb, vectordb, etc.)

For now, SQL bodies are treated as opaque strings with parameter binding validation only.

---

## Related Documents

- [DESIGN.md](DESIGN.md) - Section 5: Query System
- [COMPILER.md](COMPILER.md) - Phase 4: Query Parameter Validation
- [ERROR_CODES.md](ERROR_CODES.md) - Query error codes (E-QUERY-020 through E-QUERY-022)
- [grammar.ebnf](grammar.ebnf) - Query syntax definition
- [examples/16-database-dialects.cov](examples/16-database-dialects.cov) - Dialect examples
