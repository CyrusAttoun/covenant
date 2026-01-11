# Covenant Compiler Specification

Detailed specification of the Covenant compilation pipeline, from IR source to WASM binary.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Phase 1: Parser](#phase-1-parser)
3. [Phase 2: Symbol Graph Builder](#phase-2-symbol-graph-builder)
4. [Phase 3: Effect Checker](#phase-3-effect-checker)
5. [Phase 4: Type Checker](#phase-4-type-checker)
6. [Phase 5: Requirement Validator](#phase-5-requirement-validator)
7. [Phase 6: IR Optimizer](#phase-6-ir-optimizer)
8. [Phase 7: WASM Emitter](#phase-7-wasm-emitter)
9. [Error Handling](#error-handling)
10. [Incremental Compilation](#incremental-compilation)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Covenant Compiler                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  .cov Source Files                                          │
│         ↓                                                   │
│  ┌──────────────────────────────────────────────┐          │
│  │ Phase 1: Parser                              │          │
│  │   Input:  .cov text files                    │          │
│  │   Output: Raw AST (JSON)                     │          │
│  │   Errors: E-PARSE-xxx                        │          │
│  └──────────────────────────────────────────────┘          │
│         ↓                                                   │
│  ┌──────────────────────────────────────────────┐          │
│  │ Phase 2: Symbol Graph Builder                │          │
│  │   Input:  Raw AST                            │          │
│  │   Output: Symbol Table + Forward Refs        │          │
│  │   Errors: E-SYMBOL-xxx                       │          │
│  │   Validates: I1, I3, I4, I5                   │          │
│  └──────────────────────────────────────────────┘          │
│         ↓                                                   │
│  ┌──────────────────────────────────────────────┐          │
│  │ Phase 3: Effect Checker                      │          │
│  │   Input:  Symbol Table                       │          │
│  │   Output: Symbol Table + Effect Closures     │          │
│  │   Errors: E-EFFECT-xxx                       │          │
│  │   Validates: I2 (Effect Transitivity)        │          │
│  └──────────────────────────────────────────────┘          │
│         ↓                                                   │
│  ┌──────────────────────────────────────────────┐          │
│  │ Phase 4: Type Checker                        │          │
│  │   Input:  Symbol Table + Effect Closures     │          │
│  │   Output: Fully Typed AST                    │          │
│  │   Errors: E-TYPE-xxx                         │          │
│  └──────────────────────────────────────────────┘          │
│         ↓                                                   │
│  ┌──────────────────────────────────────────────┐          │
│  │ Phase 5: Requirement Validator               │          │
│  │   Input:  Typed AST                          │          │
│  │   Output: Coverage Report                    │          │
│  │   Errors: E-REQ-xxx                          │          │
│  └──────────────────────────────────────────────┘          │
│         ↓                                                   │
│  ┌──────────────────────────────────────────────┐          │
│  │ Phase 6: IR Optimizer                        │          │
│  │   Input:  Typed AST + Coverage Report        │          │
│  │   Output: Optimized IR                       │          │
│  │   Warnings: W-DEAD-xxx, W-PERF-xxx           │          │
│  └──────────────────────────────────────────────┘          │
│         ↓                                                   │
│  ┌──────────────────────────────────────────────┐          │
│  │ Phase 7: WASM Emitter                        │          │
│  │   Input:  Optimized IR                       │          │
│  │   Output: .wasm Binary                       │          │
│  │   Errors: Backend errors (rare)              │          │
│  └──────────────────────────────────────────────┘          │
│         ↓                                                   │
│  .wasm Binary Module                                        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Phase Boundary Rules:**
- Each phase only runs if previous phase succeeded (no errors)
- Exception: Optimizer warnings do not block WASM emission
- All errors within a phase are collected before proceeding
- Phase outputs are immutable (next phase creates new structures)

---

## Phase 1: Parser

### Purpose
Convert `.cov` text files into structured AST (Abstract Syntax Tree).

### Input
- Source files: `*.cov`
- Character encoding: UTF-8
- Grammar: `grammar.ebnf`

### Output

**AST Schema (JSON):**
```json
{
  "version": "0.1.0",
  "source_files": [
    {
      "path": "src/main.cov",
      "checksum": "sha256:abc123..."
    }
  ],
  "snippets": [
    {
      "id": "module.function_name",
      "kind": "fn",
      "location": {
        "file": "src/main.cov",
        "line": 1,
        "column": 0,
        "span": {
          "start": {"line": 1, "col": 0},
          "end": {"line": 42, "col": 3}
        }
      },
      "sections": {
        "effects": [...],
        "requires": [...],
        "signature": {...},
        "body": {...},
        "tests": [...],
        "metadata": {...}
      }
    }
  ],
  "refactor_blocks": [
    {
      "id": "rename_function",
      "location": {...},
      "steps": [...]
    }
  ]
}
```

### Error Recovery

**Strategy:** Panic mode recovery at block boundaries

1. **On syntax error:**
   - Report error with location
   - Skip tokens until next `end` keyword
   - Insert synthetic `end` if needed
   - Continue parsing

2. **Unterminated blocks at EOF:**
   - Insert synthetic `end` keywords
   - Report E-PARSE-004 for each

3. **Invalid token sequences:**
   - Report E-PARSE-001
   - Skip to next valid production start

**Example Recovery:**
```
snippet id="foo" kind="fn"
  body
    step id="s1" kind="compute"
      op=add
      input var="x"
      // Missing: second input, 'as', and 'end'
  end  // Parser recovers here
end
```

Parser inserts:
```
      input lit=0        // Synthetic default
      as="_"             // Synthetic binding
    end                  // Synthetic end
```

Errors reported:
- E-PARSE-001: Expected 'input' at line X
- E-PARSE-001: Expected 'as' at line Y
- E-PARSE-004: Unterminated step at line Z

### Canonical Ordering Validation

Parser enforces canonical section ordering within snippets:

**Correct order:**
1. `effects`
2. `requires`
3. `types`
4. `tools`
5. `signature`
6. `body`
7. `tests`
8. `metadata`

**Out-of-order detection:**
```
snippet id="foo" kind="fn"
  body        // ERROR: body before signature
    ...
  end
  signature   // ERROR: E-PARSE-003
    ...
  end
end
```

Auto-fix: Reorder sections to canonical order (confidence: 1.0)

### Performance Targets

- **Throughput:** >10,000 lines/second
- **Memory:** O(n) where n = file size
- **Incremental:** Single-file re-parse <10ms

---

## Phase 2: Symbol Graph Builder

### Purpose
Build bidirectional symbol table with forward references.

### Input
Raw AST from Phase 1

### Output

**Symbol Table Schema:**
```json
{
  "version": 1,
  "symbols": {
    "module.function_name": {
      "kind": "fn",
      "id": "module.function_name",
      "location": {...},

      // Forward references (extracted from AST)
      "calls": ["other.func", "utils.helper"],
      "references": ["User", "DbError"],
      "param_types": ["Int", "String"],
      "return_type": "union { User | DbError }",

      // Backward references (computed)
      "called_by": ["main.process"],
      "referenced_by": ["app.controller"],

      // Not yet computed (Phase 3)
      "effects": null,
      "effect_closure": null,

      // Not yet computed (Phase 5)
      "requirements": [],
      "tests": [],
      "covered_by": []
    }
  },

  "invariants": {
    "I1_bidirectionality": true,
    "I3_coverage_linkage": true,
    "I4_acyclicity": true,
    "I5_relation_bidirectionality": true
  }
}
```

### Algorithm

**Step 1: First Pass — Extract Forward References**
```python
for snippet in ast.snippets:
    symbol = {
        "id": snippet.id,
        "kind": snippet.kind,
        "calls": extract_calls(snippet.body),
        "references": extract_type_references(snippet),
        "called_by": [],  # Computed in step 2
        "referenced_by": []
    }

    if snippet.id in symbol_table:
        error(E_SYMBOL_002, f"Duplicate ID: {snippet.id}")

    symbol_table[snippet.id] = symbol
```

**Step 2: Second Pass — Compute Backward References**
```python
for symbol_id, symbol in symbol_table.items():
    # Compute called_by
    for callee_id in symbol.calls:
        if callee_id not in symbol_table:
            error(E_SYMBOL_001, f"Undefined function: {callee_id}")
            continue

        symbol_table[callee_id].called_by.append(symbol_id)

    # Compute referenced_by
    for ref_id in symbol.references:
        if ref_id not in symbol_table:
            error(E_SYMBOL_001, f"Undefined type: {ref_id}")
            continue

        symbol_table[ref_id].referenced_by.append(symbol_id)
```

**Step 3: Validate I1 (Bidirectionality)**
```python
for symbol_id, symbol in symbol_table.items():
    for callee_id in symbol.calls:
        assert symbol_id in symbol_table[callee_id].called_by, \
            f"I1 violated: {symbol_id} calls {callee_id} but not in called_by"
```

**Step 4: Validate I4 (Acyclicity)**
```python
def detect_cycle(graph):
    visited = set()
    rec_stack = set()

    def visit(node):
        if node in rec_stack:
            return True  # Cycle detected
        if node in visited:
            return False

        visited.add(node)
        rec_stack.add(node)

        for callee in graph[node].calls:
            if visit(callee):
                return True

        rec_stack.remove(node)
        return False

    for node in graph:
        if visit(node):
            error(E_SYMBOL_003, f"Circular import involving {node}")
```

**Step 5: Extract and Validate Relations (I5)**
```python
# Inverse relation mapping
RELATION_INVERSES = {
    "contains": "contained_by",
    "contained_by": "contains",
    "describes": "described_by",
    "described_by": "describes",
    "next": "previous",
    "previous": "next",
    "supersedes": "precedes",
    "precedes": "supersedes",
    "causes": "caused_by",
    "caused_by": "causes",
    "motivates": "enables",
    "enables": "motivates",
    "implements": "implemented_by",
    "implemented_by": "implements",
    # Symmetric relations (inverse is same type)
    "elaborates_on": "elaborates_on",
    "contrasts_with": "contrasts_with",
    "example_of": "example_of",
    "related_to": "related_to",
    "depends_on": "depends_on",
    "version_of": "version_of",
}

for symbol_id, symbol in symbol_table.items():
    snippet = ast.snippets[symbol_id]

    if snippet.sections.relations:
        for rel in snippet.sections.relations:
            target_id = rel.target if rel.direction == "to" else rel.source
            rel_type = rel.type

            # Validate target exists
            if target_id not in symbol_table:
                error(E_REL_001, f"Relation target not found: {target_id}")
                continue

            # Store forward relation
            symbol.relations_to.append({
                "target": target_id,
                "type": rel_type
            })

            # Compute and store inverse relation
            inverse_type = RELATION_INVERSES[rel_type]
            symbol_table[target_id].relations_from.append({
                "source": symbol_id,
                "type": inverse_type
            })
```

**Step 6: Validate I5 (Relation Bidirectionality)**
```python
for symbol_id, symbol in symbol_table.items():
    for rel in symbol.relations_to:
        target = symbol_table[rel["target"]]
        inverse_type = RELATION_INVERSES[rel["type"]]

        # Check inverse exists
        found = any(
            r["source"] == symbol_id and r["type"] == inverse_type
            for r in target.relations_from
        )

        assert found, \
            f"I5 violated: {symbol_id} -> {rel['target']} has no inverse"
```

### Error Handling

**E-SYMBOL-001: Undefined Reference**
- Tolerate during Phase 2 (insert placeholder)
- Mark for resolution in later phases
- If still unresolved after Phase 4 → hard error

**E-SYMBOL-002: Duplicate ID**
- Hard error, cannot proceed
- Suggest renaming second occurrence

**E-SYMBOL-003: Circular Import**
- Hard error, violates I4
- Report full cycle path
- Suggest refactoring to break cycle

**E-REL-001: Relation Target Not Found**
- Hard error, violates I5 (relation graph integrity)
- Report: `Relation to "nonexistent.id" not found`
- Occurs when `rel to="..."` references a snippet ID that doesn't exist
- Auto-fix: None (requires creating target or removing relation)

### Performance Targets

- **Time complexity:** O(n + e) where n = symbols, e = edges (calls/references)
- **Memory:** O(n + e)
- **Incremental:** Single snippet update: <5ms (see section 10)

---

## Phase 3: Effect Checker

### Purpose
Compute effect closures and validate effect declarations.

### Input
Symbol table from Phase 2 (with forward/backward refs)

### Output
Symbol table with `effects` and `effect_closure` fields populated

**Example:**
```json
{
  "symbols": {
    "main.process": {
      "effects": ["network"],           // Declared
      "effect_closure": ["network", "database", "filesystem"],  // Computed
      "calls": ["db.query", "http.fetch"]
    },
    "db.query": {
      "effects": ["database"],
      "effect_closure": ["database", "filesystem"],  // Transitively from logging
      "calls": ["utils.log"]
    },
    "utils.log": {
      "effects": ["filesystem"],
      "effect_closure": ["filesystem"],
      "calls": []
    }
  }
}
```

### Algorithm

**Step 1: Extract Declared Effects**
```python
for symbol_id, symbol in symbol_table.items():
    snippet = ast.snippets[symbol_id]

    if snippet.sections.effects:
        symbol.effects = [e.name for e in snippet.sections.effects]
    else:
        symbol.effects = []  # Pure function
```

**Step 2: Compute Effect Closure (Transitive)**
```python
def compute_effect_closure(symbol_id, visited=None):
    if visited is None:
        visited = set()

    if symbol_id in visited:
        return set()  # Already computed (or cycle detected in Phase 2)

    visited.add(symbol_id)
    symbol = symbol_table[symbol_id]

    # Start with declared effects
    closure = set(symbol.effects)

    # Add effects from all callees (transitive)
    for callee_id in symbol.calls:
        if callee_id in symbol_table:
            closure |= compute_effect_closure(callee_id, visited)

    return closure

for symbol_id in symbol_table:
    symbol_table[symbol_id].effect_closure = list(
        compute_effect_closure(symbol_id)
    )
```

**Step 3: Validate I2 (Effect Transitivity)**
```python
for symbol_id, symbol in symbol_table.items():
    declared = set(symbol.effects)
    computed = set(symbol.effect_closure)

    if not declared.issuperset(computed):
        missing = computed - declared
        error(E_EFFECT_001,
              f"{symbol_id} is missing effect declarations: {missing}")
```

**Step 4: Validate Pure Functions**
```python
for symbol_id, symbol in symbol_table.items():
    if len(symbol.effects) == 0:  # Pure function
        for callee_id in symbol.calls:
            callee = symbol_table[callee_id]
            if len(callee.effect_closure) > 0:
                error(E_EFFECT_001,
                      f"Pure function {symbol_id} calls effectful {callee_id}")
```

### Error Handling

**E-EFFECT-001: Pure Function Calls Effectful Code**
- Report violation with call chain
- Auto-fix: Add missing effect declarations (confidence: 1.0)

**E-EFFECT-002: Missing Effect Declaration**
- Detected when `declared ⊂ computed` (strict subset)
- Auto-fix: Insert effects section with computed closure

**E-EFFECT-003: Effect Transitivity Violation**
- Complex case: effects propagate through multiple call levels
- Auto-fix: Suggest adding effect to intermediate functions (ranked by confidence)

### Performance Targets

- **Time complexity:** O(n × d) where d = max call depth
- **Memoization:** Cache computed closures to avoid redundant traversals
- **Incremental:** Only recompute closures for affected subgraph

---

## Phase 4: Type Checker

### Purpose
Annotate every expression with a type and validate type correctness.

### Input
Symbol table with effect closures from Phase 3

### Output
Fully typed AST with type annotations on every step

**Example:**
```json
{
  "step": {
    "id": "s1",
    "kind": "compute",
    "op": "add",
    "inputs": [
      {"var": "x", "type": "Int"},
      {"lit": 5, "type": "Int"}
    ],
    "as": "result",
    "type": "Int"  // Inferred from op=add with Int inputs
  }
}
```

### Type System

**Primitive Types:**
- `Int`, `Float`, `String`, `Bool`, `None`

**Composite Types:**
- `struct { field: Type, ... }`
- `enum { Variant1(T1), Variant2(T2), ... }`
- `union { Type1 | Type2 | ... }`
- `array<T>` (sugar for `T[]`)
- `optional<T>` (sugar for `union { T | None }`)

**Type Rules:**

**Compute Operations:**
```
add, sub, mul, div, mod:
  Int × Int → Int
  Float × Float → Float
  Int × Float → Float
  String × String → String  (concat for add)

equals, not_equals:
  T × T → Bool (for any T)

less, greater, less_eq, greater_eq:
  Int × Int → Bool
  Float × Float → Bool
  String × String → Bool

and, or:
  Bool × Bool → Bool

not:
  Bool → Bool

neg:
  Int → Int
  Float → Float
```

**Match Exhaustiveness:**
```
match on union { T1 | T2 | T3 }
  case T1 → ...
  case T2 → ...
  case T3 → ...  // All variants must be covered
```

### Algorithm

**Step 1: Type Inference (Bottom-Up)**
```python
def infer_type(step):
    if step.kind == "compute":
        input_types = [infer_type(inp) for inp in step.inputs]
        return infer_op_type(step.op, input_types)

    elif step.kind == "call":
        callee = symbol_table[step.fn]
        return callee.return_type

    elif step.kind == "bind":
        return infer_type(step.from)

    elif step.kind == "if":
        then_type = infer_type(step.then[-1])  # Last step in then
        else_type = infer_type(step.else[-1]) if step.else else None

        if else_type and then_type != else_type:
            error(E_TYPE_001, f"Branches have different types: {then_type} vs {else_type}")

        return then_type

    elif step.kind == "match":
        case_types = [infer_type(case.steps[-1]) for case in step.cases]

        if not all_equal(case_types):
            error(E_TYPE_001, f"Match cases have different types: {case_types}")

        return case_types[0]
```

**Step 2: Type Checking (Top-Down)**
```python
def check_type(step, expected_type):
    inferred = infer_type(step)

    if not compatible(inferred, expected_type):
        error(E_TYPE_001,
              f"Type mismatch: expected {expected_type}, got {inferred}")

    return inferred
```

**Step 3: Match Exhaustiveness**
```python
def check_exhaustiveness(match_step, union_type):
    covered_variants = {case.variant for case in match_step.cases}
    all_variants = {v.name for v in union_type.variants}

    missing = all_variants - covered_variants
    if missing:
        error(E_TYPE_004, f"Non-exhaustive match, missing: {missing}")
```

### Error Handling

**E-TYPE-001: Type Mismatch**
- Report expected vs. actual
- Suggest type conversion if available

**E-TYPE-002: Undefined Type**
- Report with fuzzy match suggestions (Levenshtein distance)

**E-TYPE-003: Incompatible Union**
- Detect duplicate types in union
- Auto-fix: Remove duplicates

**E-TYPE-004: Non-Exhaustive Match**
- List missing variants
- Auto-fix: Insert wildcard case or missing variants

### 4.5 Query Validation

The type checker handles two types of queries differently:

#### 4.5.1 Covenant Queries (No Dialect)

For queries without a `dialect` attribute (or `dialect="covenant"`), full type checking is performed:

**Validations:**
1. **Target exists** — `target` must reference a valid Covenant type or "project"
2. **From clause** — `from` must reference a queryable type
3. **Where conditions** — Field names and types must be valid
4. **Order clause** — Field must exist in result type
5. **Join/Follow** — Related types and relations must exist

```python
def check_covenant_query(step, symbol_table):
    # Validate target
    if step.target != "project" and step.target not in symbol_table:
        error(E_QUERY_001, f"Unknown target: {step.target}")

    # Validate from clause
    from_type = resolve_type(step.from_clause, step.target)
    if not from_type:
        error(E_QUERY_002, f"Unknown type: {step.from_clause}")

    # Validate where conditions
    for condition in step.where.conditions:
        validate_field_access(condition.field, from_type)

    # Validate follow relation
    if step.follow:
        if step.follow.rel not in get_relations(from_type):
            error(E_QUERY_003, f"Unknown relation: {step.follow.rel}")
```

#### 4.5.2 SQL Dialect Queries

For queries with a `dialect` attribute (postgres, sqlserver, mysql, sqlite, etc.), the compiler validates parameter bindings only — SQL is not parsed.

**Validations:**

1. **Dialect Required (E-QUERY-020)**
   - SQL queries must have a `dialect` attribute

2. **Returns Required (E-QUERY-022)**
   - `returns` type annotation is required

3. **Parameter Binding Validation (E-QUERY-020, E-QUERY-021)**
   - Each placeholder in body must have matching `param` declaration
   - Each declared `param` must have corresponding placeholder

4. **Target Binding (Warning)**
   - Query's `target` should reference a declared database binding
   - Warning if query's `dialect` doesn't match binding's dialect

**Placeholder Patterns by Dialect:**
| Dialect | Pattern | Example |
|---------|---------|---------|
| postgres | `:name` | `:user_id` |
| sqlserver | `@name` | `@user_id` |
| mysql | `?` | Positional |
| sqlite | `:name` or `?` | Either |

**Algorithm:**
```python
def check_dialect_query(step, database_bindings):
    # Dialect is required
    if not step.dialect:
        error(E_QUERY_020, "SQL queries require 'dialect' attribute")

    # Returns is required
    if not step.returns:
        error(E_QUERY_022, "SQL queries require 'returns' type annotation")

    # Extract placeholders from body using dialect-specific pattern
    placeholders = extract_placeholders(step.body, step.dialect)
    declared_params = {p.name for p in step.params}

    # Validate all placeholders have declarations
    for placeholder in placeholders:
        if placeholder not in declared_params:
            error(E_QUERY_020, f"Unmatched placeholder: {placeholder}")

    # Validate all declarations have placeholders
    for param in step.params:
        if param.name not in placeholders:
            error(E_QUERY_021, f"Param '{param.name}' has no matching placeholder")

    # Warn on dialect mismatch
    if step.target in database_bindings:
        binding_dialect = database_bindings[step.target].dialect
        if step.dialect != binding_dialect:
            warning(W_QUERY_001, f"Dialect mismatch: query uses {step.dialect}, binding uses {binding_dialect}")

def extract_placeholders(body: str, dialect: str) -> Set[str]:
    """Extract placeholder names from SQL body."""
    if dialect == "postgres":
        # Match :name patterns
        return set(re.findall(r':(\w+)', body))
    elif dialect == "sqlserver":
        # Match @name patterns
        return set(re.findall(r'@(\w+)', body))
    elif dialect == "mysql":
        # Count ? for positional params
        count = body.count('?')
        return {str(i) for i in range(count)}
    elif dialect == "sqlite":
        # Both :name and ? supported
        named = set(re.findall(r':(\w+)', body))
        positional = body.count('?')
        return named | {str(i) for i in range(positional)}
    else:
        # Unknown dialect - extract :name as default
        return set(re.findall(r':(\w+)', body))
```

### Performance Targets

- **Time complexity:** O(n × s) where s = avg steps per function
- **Incremental:** Only re-check modified functions

---

## Phase 5: Requirement Validator

### Purpose
Validate that all requirements have test coverage.

### Input
Typed AST from Phase 4

### Output
Coverage report linking requirements to tests

**Schema:**
```json
{
  "coverage": {
    "requirements": {
      "R-AUTH-001": {
        "text": "Users must authenticate with email and password",
        "priority": "critical",
        "status": "implemented",
        "covered_by": ["T-AUTH-001", "T-AUTH-002"],
        "coverage": 1.0
      },
      "R-AUTH-002": {
        "text": "Failed logins must be rate-limited",
        "priority": "high",
        "status": "approved",
        "covered_by": [],
        "coverage": 0.0  // UNCOVERED
      }
    },
    "tests": {
      "T-AUTH-001": {
        "kind": "unit",
        "covers": ["R-AUTH-001"],
        "location": {...}
      }
    }
  },
  "summary": {
    "total_requirements": 2,
    "covered": 1,
    "uncovered": 1,
    "coverage_percent": 50.0
  }
}
```

### Algorithm

**Step 1: Extract Requirements and Tests**
```python
requirements = {}
tests = {}

for snippet in ast.snippets:
    if snippet.sections.requires:
        for req in snippet.sections.requires:
            requirements[req.id] = {
                "text": req.text,
                "priority": req.priority,
                "status": req.status,
                "covered_by": []
            }

    if snippet.sections.tests:
        for test in snippet.sections.tests:
            tests[test.id] = {
                "kind": test.kind,
                "covers": test.covers if hasattr(test, 'covers') else [],
                "location": test.location
            }
```

**Step 2: Build Coverage Links**
```python
for test_id, test in tests.items():
    for req_id in test.covers:
        if req_id not in requirements:
            error(E_REQ_002, f"Test {test_id} references nonexistent requirement {req_id}")
            continue

        requirements[req_id].covered_by.append(test_id)
```

**Step 3: Validate I3 (Coverage Linkage)**
```python
for req_id, req in requirements.items():
    # Forward link
    for test_id in req.covered_by:
        assert req_id in tests[test_id].covers, \
            f"I3 violated: {req_id} in covered_by but not in test.covers"

for test_id, test in tests.items():
    # Backward link
    for req_id in test.covers:
        if req_id in requirements:  # Already validated in step 2
            assert test_id in requirements[req_id].covered_by, \
                f"I3 violated: {req_id} in test.covers but not in req.covered_by"
```

**Step 4: Report Uncovered Requirements**
```python
for req_id, req in requirements.items():
    if len(req.covered_by) == 0:
        severity = "error" if req.priority == "critical" else "warning"
        error(E_REQ_001, f"Uncovered requirement: {req_id}", severity=severity)
```

### Error Handling

**E-REQ-001: Uncovered Requirement**
- Severity depends on priority:
  - `critical` → error (blocks compilation)
  - `high` → warning
  - `medium`, `low` → info
- Auto-fix: Insert placeholder test

**E-REQ-002: Test References Nonexistent Requirement**
- Hard error
- Suggest removing `covers` attribute or creating requirement

### Performance Targets

- **Time complexity:** O(r + t) where r = requirements, t = tests
- **Memory:** O(r + t)

---

## Phase 6: IR Optimizer

### Purpose
Optimize IR for performance and emit warnings about inefficiencies.

### Input
Typed AST + Coverage Report

### Output
Optimized IR (same schema as typed AST)

### Optimizations

**1. Dead Code Elimination**
```python
def eliminate_dead_code(snippet):
    # Find all steps reachable from entry point
    reachable = set()

    def mark_reachable(step_id):
        if step_id in reachable:
            return
        reachable.add(step_id)

        step = snippet.body.steps[step_id]

        # Mark dependencies
        for dep in step.dependencies:
            mark_reachable(dep)

    # Start from last step (usually return)
    mark_reachable(snippet.body.steps[-1].id)

    # Remove unreachable steps
    for step in snippet.body.steps:
        if step.id not in reachable:
            warn(W_DEAD_002, f"Unreachable step: {step.id}")
            snippet.body.steps.remove(step)
```

**2. Constant Folding**
```python
def fold_constants(step):
    if step.kind == "compute":
        if all(inp.kind == "lit" for inp in step.inputs):
            # All inputs are literals - compute at compile time
            result = evaluate(step.op, [inp.value for inp in step.inputs])
            return {"kind": "bind", "lit": result, "as": step.as}

    return step
```

**3. Unused Binding Detection**
```python
def detect_unused_bindings(snippet):
    used = set()

    for step in snippet.body.steps:
        for inp in step.inputs:
            if inp.kind == "var":
                used.add(inp.var)

    for step in snippet.body.steps:
        if step.as not in used and step.as != "_":
            warn(W_DEAD_001, f"Unused binding: {step.as}")
```

**4. Query Optimization**
```python
def optimize_query(step):
    if step.kind == "query" and step.target == "project":
        # Estimate cost
        estimated_cost = estimate_query_cost(step)

        if step.metadata.cost_hint == "cheap" and estimated_cost > 10_000:
            error(E_QUERY_001, f"Query exceeds cost budget")

        # Suggest filter reordering
        if can_reorder_filters(step):
            info("Consider reordering filters for better performance")
```

### Warnings

**W-DEAD-001: Unused Binding**
- Auto-fix: Remove step (confidence: 0.9)

**W-DEAD-002: Unreachable Code**
- Auto-fix: Remove step (confidence: 1.0)

**W-DEAD-003: Uncalled Function**
- Auto-fix: Delete function (confidence: 0.6) or mark exported (0.4)

**W-PERF-001: Inefficient Query**
- Suggest optimization (no auto-fix)

### Performance Targets

- **Time complexity:** O(n × s) where s = avg steps per function
- **Optimizations enabled by default:** All
- **Opt-out:** `--no-optimize` flag

---

## Phase 7: WASM Emitter

### Purpose
Generate WebAssembly binary from optimized IR.

### Input
Optimized IR from Phase 6

### Output
`.wasm` binary module

**Module Structure:**
```
(module
  (import "wasi_snapshot_preview1" "fd_write" ...)
  (memory (export "memory") 1)

  ;; Generated from snippet id="main.process"
  (func $main_process (param $x i32) (result i32)
    ;; Compiled from IR steps
    local.get $x
    i32.const 2
    i32.mul
    return
  )

  (export "main_process" (func $main_process))
)
```

### Compilation Strategy

**Step → WASM Instruction Mapping:**

| IR Step Kind | WASM Instructions |
|--------------|-------------------|
| `compute op=add` | `i32.add` / `f32.add` |
| `compute op=mul` | `i32.mul` / `f32.mul` |
| `bind` | `local.set` |
| `call fn="foo"` | `call $foo` |
| `return` | `return` |
| `if` | `if ... else ... end` |
| `match` | `block ... br_table ...` |
| `query target="app_db"` | `call $db_execute_query` (runtime) |

**Effect Handling:**
- Effects compile to WASI imports
- `effect database` → link to WASI database API
- `effect network` → link to WASI HTTP API
- `effect filesystem` → link to WASI filesystem API

### 7.3 SQL Code Generation

Queries compile to SQL strings stored in the WASM data segment, with runtime calls to execute them.

**Dialect-Specific SQL Generation:**

| Dialect | Identifier Quoting | LIMIT Syntax | Parameter Syntax |
|---------|-------------------|--------------|------------------|
| postgres | `"name"` | `LIMIT n OFFSET m` | `:name` |
| sqlserver | `[name]` | `OFFSET m ROWS FETCH NEXT n ROWS ONLY` | `@name` |
| mysql | `` `name` `` | `LIMIT n OFFSET m` | `?` (positional) |
| sqlite | `"name"` | `LIMIT n OFFSET m` | `?` or `:name` |
| generic | `"name"` | `LIMIT n OFFSET m` | `:name` |

**SQL Emitter:**
```python
class SQLEmitter:
    def __init__(self, dialect: str):
        self.dialect = dialect

    def emit_query(self, step) -> str:
        if step.kind == "raw_sql":
            return step.sql  # Pass through raw SQL

        parts = []

        # WITH clause (CTEs)
        if step.with_clause:
            parts.append(self.emit_with_clause(step.with_clause))

        # SELECT clause
        parts.append(self.emit_select(step.select))

        # FROM clause
        parts.append(f"FROM {self.quote(step.from)}")

        # JOIN clauses
        for join in step.joins:
            parts.append(self.emit_join(join))

        # WHERE clause
        if step.where:
            parts.append(f"WHERE {self.emit_condition(step.where)}")

        # GROUP BY
        if step.group_by:
            fields = ', '.join(self.quote(f) for f in step.group_by)
            parts.append(f"GROUP BY {fields}")

        # HAVING
        if step.having:
            parts.append(f"HAVING {self.emit_condition(step.having)}")

        # ORDER BY
        if step.order_by:
            parts.append(self.emit_order_by(step.order_by))

        # LIMIT/OFFSET (dialect-specific)
        if step.limit:
            parts.append(self.emit_limit(step.limit, step.offset))

        return '\n'.join(parts)

    def quote(self, name: str) -> str:
        if self.dialect == "sqlserver":
            return f"[{name}]"
        elif self.dialect == "mysql":
            return f"`{name}`"
        else:
            return f'"{name}"'

    def emit_limit(self, limit, offset) -> str:
        if self.dialect == "sqlserver" and offset:
            return f"OFFSET {offset} ROWS FETCH NEXT {limit} ROWS ONLY"
        elif offset:
            return f"LIMIT {limit} OFFSET {offset}"
        else:
            return f"LIMIT {limit}"
```

**WASM Integration:**
```wasm
;; Query step s1
i32.const SQL_OFFSET      ;; SQL string pointer in data segment
i32.const SQL_LENGTH      ;; SQL string length
i32.const PARAM_COUNT     ;; Number of parameters
;; ... push parameter values ...
call $__db_execute_query  ;; Runtime function
local.set $result         ;; Store result
```

**Aggregate Function Mapping:**
```python
AGGREGATE_TO_SQL = {
    "count": "COUNT",
    "count_distinct": lambda inp: f"COUNT(DISTINCT {inp})",
    "sum": "SUM",
    "avg": "AVG",
    "min": "MIN",
    "max": "MAX",
    "string_agg": {
        "postgres": "STRING_AGG",
        "sqlserver": "STRING_AGG",
        "mysql": "GROUP_CONCAT",
    },
    "array_agg": {"postgres": "ARRAY_AGG"},
}
```

**Window Function SQL Generation:**
```python
def emit_window(win, dialect) -> str:
    func = win.op.upper()

    # Function call
    if win.input:
        func_sql = f"{func}({quote(win.input)})"
    else:
        func_sql = f"{func}()"

    # OVER clause
    over_parts = []

    if win.partition_by:
        cols = ', '.join(quote(f) for f in win.partition_by)
        over_parts.append(f"PARTITION BY {cols}")

    if win.order_by:
        over_parts.append(emit_order_by(win.order_by))

    if win.frame:
        over_parts.append(emit_frame(win.frame))

    return f"{func_sql} OVER ({' '.join(over_parts)})"
```

**Raw SQL Passthrough:**
Raw SQL queries bypass codegen and pass the SQL string directly to the runtime:
- SQL stored verbatim in data segment
- Parameter bindings validated at compile time
- Return type enforced at runtime

**Memory Layout:**
```
┌─────────────────────────────────────┐
│ 0x0000 - 0x00FF: Globals            │
│ 0x0100 - 0x0FFF: Stack               │
│ 0x1000 - ...:    Heap (dynamic)     │
└─────────────────────────────────────┘
```

### Error Handling

Backend errors should be **extremely rare** if previous phases succeeded.

Possible errors:
- Resource limits exceeded (too many functions, locals, etc.)
- Unsupported feature (should have been caught earlier)

If backend error occurs → **compiler bug**, report as E-INTERNAL-001

### Performance Targets

- **Emission speed:** >50,000 instructions/second
- **Binary size:** Minimal overhead (<10% metadata)
- **Optimization level:** Configurable (`-O0`, `-O1`, `-O2`, `-O3`)

---

## Error Handling

### Error Accumulation Strategy

```python
class CompilationContext:
    def __init__(self):
        self.errors = []
        self.warnings = []

    def error(self, code, message, location=None):
        self.errors.append({
            "code": code,
            "severity": "error",
            "message": message,
            "location": location
        })

    def warn(self, code, message, location=None):
        self.warnings.append({
            "code": code,
            "severity": "warning",
            "message": message,
            "location": location
        })

    def has_errors(self):
        return len(self.errors) > 0

    def report(self):
        # Group errors by file and line
        # Sort by severity, then location
        # Format as JSON for machine parsing
        return json.dumps({
            "errors": self.errors,
            "warnings": self.warnings,
            "summary": {
                "error_count": len(self.errors),
                "warning_count": len(self.warnings)
            }
        })
```

### Phase Boundaries

```python
def compile(source_files):
    ctx = CompilationContext()

    # Phase 1
    ast = parse(source_files, ctx)
    if ctx.has_errors():
        return ctx.report()

    # Phase 2
    symbol_table = build_symbol_graph(ast, ctx)
    if ctx.has_errors():
        return ctx.report()

    # Phase 3
    symbol_table = check_effects(symbol_table, ctx)
    if ctx.has_errors():
        return ctx.report()

    # Phase 4
    typed_ast = type_check(symbol_table, ctx)
    if ctx.has_errors():
        return ctx.report()

    # Phase 5
    coverage = validate_requirements(typed_ast, ctx)
    if ctx.has_errors():
        return ctx.report()

    # Phase 6 (warnings OK)
    optimized_ir = optimize(typed_ast, ctx)
    # Continue even if warnings

    # Phase 7
    wasm_binary = emit_wasm(optimized_ir, ctx)
    if ctx.has_errors():
        return ctx.report()

    return {
        "success": True,
        "output": wasm_binary,
        "warnings": ctx.warnings
    }
```

---

## Incremental Compilation

### Change Detection

```python
class IncrementalCompiler:
    def __init__(self):
        self.symbol_table = {}
        self.symbol_graph_version = 0
        self.query_cache = LRUCache(max_size=1000)

    def recompile_snippet(self, snippet_id, new_source):
        old_snippet = self.symbol_table.get(snippet_id)

        # Parse new snippet
        new_snippet = parse_snippet(new_source)

        # Invalidate affected metadata
        if old_snippet:
            self.invalidate_dependencies(old_snippet)

        # Recompute local metadata
        new_snippet.calls = extract_calls(new_snippet)
        new_snippet.references = extract_references(new_snippet)

        # Update symbol table
        self.symbol_table[snippet_id] = new_snippet

        # Recompute effect closure (transitive)
        affected = self.find_affected_by_effects(snippet_id)
        for affected_id in affected:
            recompute_effect_closure(affected_id)

        # Bump version (invalidates all query caches)
        self.symbol_graph_version += 1
        self.query_cache.clear()

    def invalidate_dependencies(self, old_snippet):
        # Clear backward links
        for callee_id in old_snippet.calls:
            self.symbol_table[callee_id].called_by.remove(old_snippet.id)

        for ref_id in old_snippet.references:
            self.symbol_table[ref_id].referenced_by.remove(old_snippet.id)
```

### Performance

Incremental recompilation targets:
- Single snippet modification: <10ms
- Effect closure recomputation: <50ms for 10k snippet codebase
- No full recompile unless schema changes

---

## Related Documents

- [DESIGN.md](DESIGN.md) - Sections 14-18
- [ERROR_CODES.md](ERROR_CODES.md) - All error codes
- [QUERY_SEMANTICS.md](QUERY_SEMANTICS.md) - Query execution
- [STORAGE.md](STORAGE.md) - Storage provider interface
- [grammar.ebnf](grammar.ebnf) - Formal grammar
