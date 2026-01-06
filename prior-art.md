# Prior Art: Lessons from Austral, Koka, and LLM-Native Design

> What to steal, what to adapt, what to avoid.

---

## LLM-Native Language Design

**Source**: Internal analysis of machine-first language principles

### Core Insight

Traditional languages optimize for human authorship. LLM-native languages optimize for:
- **Token predictability** — deterministic generation
- **Structural clarity** — tree-shaped, not expression-based
- **Unambiguous intent** — no implicit rules
- **Cheap parsing** — single-pass, no backtracking

### Key Ideas Adopted

#### 1. Deterministic, Fully Explicit Structure

No implied meaning. Ever.

- No operator precedence (use keywords)
- No defaults (everything explicit)
- No overloading (one meaning per construct)
- No context-dependent syntax

**Result**: LLMs fail most on implicit rules. Deterministic grammar = lower hallucination rate.

#### 2. Tree-First, Not Text-First

The language is conceptually a tree, serialized as text:

- AST-as-text format
- Explicit block markers (`end`)
- No "free-form" expressions
- One operation per node

LLMs reason better over trees than over linear expressions.

#### 3. Keyword-Heavy, Symbol-Light

Symbols are ambiguous. Words are not.

| Avoid | Prefer |
|-------|--------|
| `==` `!=` `&&` `||` | `equals` `not_equals` `and` `or` |
| `? :` `+=` | `if_else` `assign` |

This improves token predictability and cross-model consistency.

#### 4. Fixed Canonical Ordering

Every construct has one valid order. No reordering allowed.

**Bad** (human-friendly, multiple valid forms):
```
send_email(to="a", body="b", subject="c")
send_email(subject="c", to="a", body="b")
```

**Good** (machine-friendly, canonical):
```
call fn="send_email"
  arg name="to" from="a"
  arg name="subject" from="c"
  arg name="body" from="b"
end
```

Ordering variance explodes output space. Canonical forms reduce model entropy.

#### 5. No Expressions — Only Operations (SSA Form)

Expressions encourage nesting, shortcuts, and ambiguity.

**Instead**: One operation per step, outputs must be named, inputs must be explicit.

```
step id="s1" kind="compute"
  op=add
  input var="a"
  input lit=5
  as="sum"
end
```

LLMs struggle with dense expressions but excel at stepwise execution graphs.

#### 6. Built-In Self-Description

Every construct declares its type, intent, and side effects:

```
tools
  tool id="t1" contract="email.send@1"
    idempotent=message_id
    cost_hint=moderate
    latency_hint=slow
  end
end
```

This enables safer tool execution, better planning, and easier validation.

#### 7. Token-Stable Grammar

Critical property: small grammar surface area.

- ~50 keywords total
- No synonyms
- No optional punctuation
- Every construct expands to predictable token sequences

This improves fine-tuning efficiency and prompt-to-code reliability.

#### 8. First-Class Requirements and Tests

Specs and tests are nodes in the graph, not comments:

```
requires
  req id="R-001"
    text "Invoice total must include tax"
  end
end

tests
  test id="T-001" covers="R-001"
    property "total equals subtotal plus tax"
  end
end
```

Enables queries like "show requirements not covered by tests."

#### 9. Queryable by Default

Every program is stored as a symbol graph with:
- Symbol table (declarations)
- Reference graph (who uses what)
- Dependency closure (what each unit needs)
- Bidirectional links (called_by, calls)

Agents don't grep — they query.

### What This Enables

- Small models can generate valid code with constrained decoding
- Context retrieval is deterministic (fetch exact neighborhood)
- Verification loops work well (validate → error → retry)
- Translation to other languages becomes template-driven

---

## Austral: Capabilities via Linear Types

**Source**: [austral-lang.org](https://austral-lang.org/) | [GitHub](https://github.com/austral/austral) | [Spec](https://austral-lang.org/spec/spec.html)

### Key Ideas to Steal

#### 1. Two-Universe Type System
Austral partitions all types into two universes:
- **Free**: Can be used any number of times (integers, booleans, immutable data)
- **Linear**: Must be used exactly once (handles, capabilities, resources)

Linearity propagates: a struct containing a linear field becomes linear.

```austral
-- Free type: use as many times as you want
let x: Int32 := 5;
let y := x + x + x;  -- fine

-- Linear type: must use exactly once
let handle: FileHandle := open("foo.txt");
write(handle, "hello");  -- handle consumed here
-- can't use handle again
```

**For Covenant**: This maps directly to capability tracking. A database capability is linear—you must thread it through, can't duplicate it, can't drop it silently.

#### 2. Capabilities as Linear Values
Capabilities aren't annotations—they're values you pass around. Libraries that don't receive capabilities can't access resources.

#### 3. Hierarchical Capability Refinement
From the root capability, you derive narrower ones:
- `Filesystem` → `Path("/home/user")` → `File`

Each step narrows what you can do.

#### 4. Simplicity as a Feature
The borrow checker is 600 lines of OCaml. The spec is readable in an afternoon. The type rules fit on a page.

**For Covenant**: A simple, predictable system is easier for both humans and LLMs to reason about.

### What to Avoid from Austral

- **Verbose syntax** — We use explicit `end` delimiters but keep block structure clean
- **C backend** — We target WASM, not C
- **No effect tracking** — Austral tracks capabilities but not effects (Covenant unifies these)

---

## Koka: Row-Polymorphic Effect Types

**Source**: [koka-lang.github.io](https://koka-lang.github.io/koka/doc/book.html) | [Paper](https://arxiv.org/abs/1406.2061)

### Key Ideas to Steal

#### 1. Effects in the Type Signature
Every function's type includes its effects:

```koka
fun factorial(n: int): total int          // no effects
fun print_hello(): console ()             // console effect
fun read_file(path: string): <io,exn> string  // io and exception effects
```

If a function has no `exn` in its type, it provably never throws.

**For Covenant**: The `effects` section declares capabilities:
```
effects
  effect database
  effect filesystem
end
```

#### 2. Effect Polymorphism via Row Types
Functions can be generic over effects. If a higher-order function takes an effectful callback, it inherits those effects.

**For Covenant**: Effects propagate transitively. If snippet A calls snippet B, A inherits B's effects.

#### 3. Semantic Guarantees from Types
- No effects → pure function
- No `exn` → never throws
- No `io` → referentially transparent

The type system provides proofs about behavior.

**For Covenant**: Snippets with empty `effects` section are verified pure by the compiler.

### What to Avoid from Koka

- **Full algebraic effect handlers** — Resumption is powerful but complex. Start with tracking only.
- **Complex type inference** — We require explicit types initially.
- **Perceus/reference counting** — Orthogonal to our goals.

---

## Synthesis: What Covenant Takes

| Feature | From | How We Adapt |
|---------|------|--------------|
| Deterministic syntax | LLM-Native | Keywords, canonical ordering, SSA form |
| Queryable structure | LLM-Native | Every node has ID, bidirectional refs |
| Requirements/tests as nodes | LLM-Native | `requires` and `tests` sections |
| Linear capabilities | Austral | Effects are capability declarations |
| Capability refinement | Austral | Hierarchical narrowing (future) |
| Effects in types | Koka | `effects` section, transitive propagation |
| Effect polymorphism | Koka | Automatic propagation through call graph |
| Simplicity | Both | Small grammar, minimal rules |

---

## Design Decision: Capabilities = Effects

**Resolved**: Capabilities and effects are the same thing.

A capability is just an effect you have permission to perform. There's no distinction between "having the database capability" and "being able to do database effects." Simpler model, less to explain.

```
// The effects section declares both:
// - What capability/permission the snippet needs
// - What side effects the snippet may perform

effects
  effect database
  effect network
end
```

---

## Open Design Questions

1. **Effect handlers or just tracking?**
   - Start with tracking only
   - Handlers (like Koka) could be added later for mocking/testing

2. **Linear types for capabilities?**
   - Could add Austral-style linearity for fine-grained control
   - Current design uses declaration-only, not value-passing

3. **Row polymorphism?**
   - Current design uses closed effect sets with transitive propagation
   - Open rows could be added if needed for generics

---

## Implementation Notes

### For Covenant
- Compiler in Rust (good WASM tooling)
- Target WASM directly (no C intermediate)
- Explicit types required (no inference initially)
- Focus on queryable symbol graph from day one
- Parser should be trivial (LL(1) or simpler)

---

## References

- [Austral Spec](https://austral-lang.org/spec/spec.html)
- [Introducing Austral](https://borretti.me/article/introducing-austral)
- [Koka Book](https://koka-lang.github.io/koka/doc/book.html)
- [Koka: Programming with Row Polymorphic Effect Types](https://arxiv.org/abs/1406.2061)
- [Algebraic Effects for Functional Programming](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/08/algeff-tr-2016-v2.pdf)
