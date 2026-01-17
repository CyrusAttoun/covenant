# Extensible Kinds

## Summary

Covenant supports **pluggable kind definitions** that extend the language without modifying the core grammar. Kinds are imported through the effects system, making them first-class capabilities that propagate transitively.

---

## Motivation

The core grammar defines structural keywords (`snippet`, `step`, `end`) but the `kind` attribute determines what's valid inside each block. Rather than hardcoding all possible kinds into the grammar, we make kinds **data-driven**:

- **Core kinds** (`fn`, `data`, `extern`, `database`) are built-in
- **Extended kinds** (`std.concurrent.parallel`, `myorg.workflow.approval`) are imported via effects
- **No grammar changes** needed to add new constructs

This enables:
1. Domain-specific extensions without forking
2. Experimental features as opt-in modules
3. Organizational patterns as importable kinds
4. Future evolution without breaking changes

---

## Design

### Kinds as Effect-Modules

A kind definition is a snippet with `kind="effect-kind"` that declares:
1. **Structure** — required/optional sections and fields
2. **Effects required** — capabilities needed to use this kind
3. **Compile target** — how it maps to runtime

```covenant
snippet id="std.concurrent" kind="effect-kind"

note "Structured concurrency primitives for parallel I/O"

kinds
  kind name="parallel"
    note "Execute branches concurrently, wait for all to complete"

    structure
      section name="branch" multiple=true required=true
        contains kind="step"
      end
      field name="on_error" type="Enum"
        values=["fail_fast", "collect_all", "ignore_errors"]
        default="fail_fast"
      end
      field name="timeout" type="Duration" optional=true
      field name="on_timeout" type="Enum"
        values=["cancel", "return_partial"]
        default="cancel"
      end
    end

    semantics
      results_order="declaration"  // deterministic
      branch_isolation=true        // no shared state
    end

    compile_to="host_parallel"
  end

  kind name="race"
    note "Execute branches concurrently, return first to complete"

    structure
      section name="branch" multiple=true required=true
        contains kind="step"
      end
      field name="timeout" type="Duration" optional=true
    end

    semantics
      results_order="completion"   // first wins
      branch_isolation=true
      cancel_losers=true
    end

    compile_to="host_race"
  end
end

effects_required
  effect runtime  // requires host runtime support
end

end
```

### Importing Kinds

To use an extended kind, declare its effect:

```covenant
snippet id="app.fetch_all" kind="fn"

effects
  effect std.concurrent  // imports parallel and race kinds
  effect network
end

body
  step id="s1" kind="std.concurrent.parallel"
    branch id="b1"
      step id="b1.1" kind="call"
        fn="http.get"
        arg name="url" lit="https://api.example.com/users"
        as="users"
      end
    end
    branch id="b2"
      step id="b2.1" kind="call"
        fn="http.get"
        arg name="url" lit="https://api.example.com/products"
        as="products"
      end
    end
    as="results"
  end

  step id="s2" kind="return"
    from="results"
    as="_"
  end
end

end
```

### Effect Propagation

When a snippet uses `effect std.concurrent`:
1. The `parallel` and `race` kinds become available
2. The `std.concurrent` effect propagates to callers
3. Pure functions cannot call this snippet

This is the same propagation behavior as other effects (`network`, `database`, etc.).

---

## Core vs Extended Kinds

### Built-in Kinds (No Import Needed)

| Kind | Purpose |
|------|---------|
| `fn` | Function definition |
| `data` | Data/document node |
| `extern` | External binding |
| `database` | Database connection |
| `type` | Type definition |
| `effect-kind` | Kind definition (meta) |

### Standard Library Kinds

| Effect | Kinds Provided |
|--------|----------------|
| `std.concurrent` | `parallel`, `race` |
| `std.testing` | `test`, `mock`, `fixture` |
| `std.workflow` | `saga`, `compensation` |

### Custom Kinds

Organizations can define domain-specific kinds:

```covenant
snippet id="acme.approval" kind="effect-kind"

note "Multi-stage approval workflow for regulated industries"

kinds
  kind name="workflow"
    structure
      section name="stage" multiple=true required=true
        field name="approver_role" type="String" required=true
        field name="timeout" type="Duration" optional=true
        contains kind="step"
      end
      field name="on_rejection" type="Enum"
        values=["abort", "escalate", "retry"]
        default="abort"
      end
    end

    compile_to="acme_workflow_engine"
  end
end

effects_required
  effect acme.workflow_runtime
end

end
```

---

## Namespacing

Kind names are **fully qualified** by their effect:

```
std.concurrent.parallel     // standard library
std.testing.mock            // standard library
acme.approval.workflow      // organization-specific
myproject.custom.thing      // project-local
```

**Benefits:**
- No collisions between different kind providers
- Clear provenance (who defined this kind?)
- Explicit imports via effects

**Usage:**
```covenant
step id="s1" kind="std.concurrent.parallel"
  ...
end
```

---

## Validation

The compiler validates kind usage:

1. **Effect declared** — Cannot use `std.concurrent.parallel` without `effect std.concurrent`
2. **Structure valid** — All required sections/fields present
3. **Types match** — Field values match declared types
4. **Nesting valid** — Contains clauses respected

### Error Example

```
Error E-KIND-001: Unknown kind 'std.concurrent.parallel'
  --> app.cov:15:3
   |
15 |   step id="s1" kind="std.concurrent.parallel"
   |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = hint: Add 'effect std.concurrent' to snippet effects section
```

---

## Compilation

### Kind Definition → Compiler Plugin

Each `compile_to` value maps to a code generation strategy:

| compile_to | Implementation |
|------------|----------------|
| `host_parallel` | Generate Promise.all wrapper in runtime.js |
| `host_race` | Generate Promise.race wrapper in runtime.js |
| `acme_workflow_engine` | Generate calls to external workflow service |

The compiler ships with built-in handlers for `std.*` kinds. Custom kinds require either:
1. A registered compiler plugin
2. An extern binding to a runtime service

### WASM Output

Extended kinds compile to:
1. **Host imports** for runtime-provided functionality
2. **Standard WASM** for kinds that are pure transformations

```javascript
// Generated runtime.js for std.concurrent
const imports = {
  "std.concurrent.parallel": async (branches) => {
    const results = await Promise.all(branches.map(executeBranch));
    return { success: true, values: results };
  },
  "std.concurrent.race": async (branches) => {
    const result = await Promise.race(branches.map(executeBranch));
    // Cancel other branches
    return { success: true, value: result };
  }
};
```

---

## Design Principles

### No Versioning

Kind definitions do not have versions. If a kind's structure changes incompatibly, create a new kind with a new name.

**Rationale:** Versioning adds complexity. Covenant prefers explicit, breaking changes over implicit compatibility layers.

### No Inheritance

Kinds do not inherit from other kinds. Each kind is self-contained.

**Rationale:** Inheritance creates hidden dependencies and makes LLM generation harder. Composition (nesting kinds) is preferred.

### Effects as Imports

The effect system already handles:
- Transitive propagation
- Capability tracking
- Module boundaries

Using effects for kind imports leverages existing machinery rather than inventing a new import system.

---

## Future Considerations

### Kind Discovery

How do LLMs learn about available kinds?

1. **Query the project** — `select all from="effect-kinds"` returns available kinds
2. **Standard library docs** — `std.*` kinds documented in language docs
3. **Inline notes** — Each kind has a `note` describing its purpose

### Kind Composition

Could kinds contain other kinds?

```covenant
kind name="retry-parallel"
  wraps kind="std.concurrent.parallel"
  adds
    field name="max_retries" type="Int" default=3
  end
end
```

**Status:** Not in initial design. May add if clear use cases emerge.

### Kind Constraints

Could kinds declare constraints beyond structure?

```covenant
kind name="parallel"
  constraints
    max_branches=10
    max_nesting_depth=2
  end
end
```

**Status:** Planned. Would generate compiler warnings/errors.

---

## Summary

| Aspect | Design Choice |
|--------|---------------|
| Kind definitions | `kind="effect-kind"` snippets |
| Import mechanism | Via effect declarations |
| Namespacing | Fully qualified: `effect.kindname` |
| Versioning | None (new name = new kind) |
| Inheritance | None (prefer composition) |
| Validation | Compiler checks structure against definition |
| Compilation | `compile_to` maps to code generation strategy |

Extensible kinds let Covenant grow without grammar changes, while keeping the core language simple and LLM-friendly.
