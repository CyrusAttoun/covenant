# Metaprogramming Examples

Covenant treats source code as data. The symbol graph is computed by the compiler and can be queried at runtime using `target="project"`. With `effect meta`, you can also modify the AST—changes serialize back to `.cov` files.

## Examples

| File | Description |
|------|-------------|
| `metaprogramming.cov` | AST queries and mutations using `effect meta` |

## Functions

### Read-Only Queries

| Function | Description |
|----------|-------------|
| `meta.find_db_functions` | Find all functions with `effect database` |
| `meta.find_callers` | Find callers of a function using bidirectional refs |
| `meta.find_dead_code` | Find unreachable functions (not called, not exported, not entry point) |
| `meta.find_urls` | Find URL literals in source code |
| `meta.find_untested_requirements` | Find requirements without test coverage |

### AST Mutations

| Function | Description |
|----------|-------------|
| `refactor.rename_function` | Rename a function and update all call sites |
| `refactor.add_getter` | Generate a getter method for a struct field |
| `refactor.prune_dead_code` | Delete dead code and return the count |

## Key Concepts

### Querying the Symbol Graph

Use `target="project"` to query functions, types, requirements, and other AST nodes:

```covenant
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

### Bidirectional References

The compiler computes metadata on every symbol, enabling reverse lookups:

```covenant
step id="s1" kind="query"
  target="project"
  select field="called_by"
  from="functions"
  where
    equals field="name" var="fn_name"
  end
  as="callers"
end
```

### Mutating the AST

CRUD operations modify the symbol graph, which serializes back to `.cov` files:

```covenant
step id="s1" kind="update"
  target="project.functions"
  set field="name" from="new_name"
  where
    equals field="name" var="old_name"
  end
  as="_"
end
```

The host runtime can refuse to grant `effect meta` to untrusted code.
