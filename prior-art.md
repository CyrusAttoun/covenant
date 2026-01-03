# Prior Art: Lessons from Austral and Koka

> What to steal, what to adapt, what to avoid.

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

**For Covenant**: This maps directly to capability tracking. A `Database::read` capability is linear—you must thread it through, can't duplicate it, can't drop it silently.

#### 2. Capabilities as Linear Values
Capabilities aren't annotations—they're values you pass around:

```austral
function read_config(cap: Filesystem): Config is
    let path: Path := get_path(cap, "/etc/config");
    let contents: String := read_file(path);
    return parse(contents);
end;
```

The `Filesystem` capability must be passed in. You can't conjure it. Libraries that don't receive capabilities can't access the filesystem.

**For Covenant**: Tool contracts could require capability parameters:
```covenant
fn read_config(fs: Cap<Filesystem>) -> Result<Config, IoError>
    requires: fs
{
    // fs capability is consumed/borrowed here
}
```

#### 3. Hierarchical Capability Refinement
From the root capability, you derive narrower ones:
- `Filesystem` → `Path("/home/user")` → `File`

Each step narrows what you can do. Dependencies only see what you give them.

**For Covenant**: Tool contracts could declare capability hierarchies:
```covenant
tool Filesystem {
    capabilities: [root]

    fn narrow(path: String) -> Cap<Path>
        requires: root

    fn read(path: Cap<Path>) -> Result<Bytes, IoError>
        // no capability required—Path itself is the proof
}
```

#### 4. Simplicity as a Feature
The borrow checker is 600 lines of OCaml. The spec is readable in an afternoon. The type rules fit on a page.

**For Covenant**: This aligns with "AI-friendly." A simple, predictable system is easier for both humans and LLMs to reason about.

### What to Avoid from Austral

- **Verbose syntax** (`end if`, `end for`) — Rust-style braces are more compact
- **C backend** — We want WASM, not C
- **No effect tracking** — Austral tracks capabilities but not effects

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

If a function has no `exn` in its type, it provably never throws. If it has no `io`, it provably does no I/O.

**For Covenant**: This is exactly what `effects: [filesystem]` wants to be:
```covenant
fn write_log(msg: String) -> Result<(), IoError>
    effects: [filesystem, console]
```

#### 2. Effect Polymorphism via Row Types
Functions can be generic over effects:

```koka
fun map(xs: list<a>, f: (a) -> e b): e list<b>
```

The `e` is an effect variable. If `f` has `io` effect, then `map(xs, f)` has `io` effect. If `f` is `total`, so is the map.

**For Covenant**: Critical for higher-order functions:
```covenant
fn map<T, U, E>(xs: List<T>, f: fn(T) -> U with E) -> List<U>
    effects: E  // propagates whatever effects f has
```

#### 3. Effect Rows with Extension
Effects use row polymorphism: `<io, exn | e>` means "io and exn, plus whatever else is in e."

```koka
fun safe_read(): <io | e> string
```

This function does `io` but might have other effects too (the `e` is open).

**For Covenant**: Allows composing capabilities:
```covenant
fn process(db: Cap<Database>, fs: Cap<Filesystem>) -> Result<(), Error>
    effects: <database, filesystem | E>
```

#### 4. Effect Handlers
Effects can be "handled" like exceptions, but with resumption:

```koka
effect ask<a> {
    fun ask(): a
}

fun with_input(x: a, action: () -> <ask<a> | e> b): e b {
    with handler {
        fun ask() { resume(x) }  // resume with value x
    }
    action()
}
```

The handler intercepts `ask()` calls, provides a value, and resumes execution.

**For Covenant**: Tool implementations could work this way. The `tool Database` contract declares the effect; the host runtime provides the handler.

#### 5. Semantic Guarantees from Types
- No `exn` → never throws
- No `div` → always terminates
- No `io` → referentially transparent

The type system provides *proofs* about behavior.

**For Covenant**:
```covenant
fn pure_compute(x: Int) -> Int
    effects: []  // compiler verifies no effects
```

### What to Avoid from Koka

- **Complexity of full algebraic effects** — Resumption is powerful but complex. Start simpler.
- **No capability tracking** — Koka tracks effects but not fine-grained permissions.
- **Perceus/reference counting** — Interesting but orthogonal to our goals.

---

## Synthesis: What Covenant Should Take

| Feature | From | Adaptation |
|---------|------|------------|
| Linear capabilities | Austral | Capabilities are linear values passed to functions |
| Two-universe types | Austral | `Free` vs `Linear` universe distinction |
| Capability refinement | Austral | Hierarchical narrowing of permissions |
| Effects in types | Koka | Function signatures declare effects |
| Effect polymorphism | Koka | Generic over effect rows |
| Effect rows | Koka | `<effect1, effect2 | E>` syntax |
| Simplicity | Austral | Keep the rules minimal and predictable |

### Combined Example

```covenant
tool Database {
    fn query(sql: String) -> Result<Rows, DbError>
        requires: [database]
}

tool Filesystem {
    fn read(path: Path) -> Result<Bytes, IoError>
        requires: [filesystem]

    fn write(path: Path, content: Bytes) -> Result<(), IoError>
        requires: [filesystem]
}

// A function that uses both — requires propagate
fn backup_users() -> Result<(), Error>
    requires: [database, filesystem]
{
    let users = Database::query("SELECT * FROM users")?;
    Filesystem::write("/backup/users.json", users.to_json())?;
    Ok(())
}

// A pure function — no requires clause means no effects
fn parse_config(input: String) -> Result<Config, ParseError> {
    // compiler verifies this calls nothing with effects
}
```

---

## Design Decision: Capabilities = Effects

**Resolved**: Capabilities and effects are the same thing.

A capability is just an effect you have permission to perform. There's no distinction between "having the database capability" and "being able to do database effects." Simpler model, less to explain.

```covenant
// Before (two concepts):
fn query(db: Cap<Database>) -> Result<Rows, Error>
    effects: [database]

// After (unified):
fn query() -> Result<Rows, Error>
    requires: [database]  // this IS the effect declaration
```

The `requires` clause declares both:
- What capability/permission the function needs
- What effect/side-effect the function may perform

## Remaining Open Questions

1. **Effect handlers or just effect tracking?**
   - Koka's handlers are powerful but complex
   - Start with just tracking, add handlers later?

2. **Row polymorphism complexity**
   - Full row polymorphism adds inference complexity
   - Maybe start with closed effect sets?

3. **Are capabilities values or phantom types?**
   - Values = passed at runtime, enables dynamic capability patterns
   - Phantom = erased, purely compile-time verification

---

## Implementation Notes

### From Austral
- Compiler in OCaml, ~15k lines total
- Borrow checker is 600 lines
- Generates C, then compiles with cc
- Whole-program compilation (no separate compilation yet)

### From Koka
- Compiler in Haskell
- Perceus for memory management (reference counting)
- Generates C
- Type inference based on Hindley-Milner + row polymorphism

### For Covenant
- Could use Rust for the compiler (good WASM tooling)
- Target WASM directly (no C intermediate)
- Start with explicit types (inference later)
- Focus on queryable AST from day one

---

## References

- [Austral Spec](https://austral-lang.org/spec/spec.html)
- [Introducing Austral](https://borretti.me/article/introducing-austral)
- [Koka Book](https://koka-lang.github.io/koka/doc/book.html)
- [Koka: Programming with Row Polymorphic Effect Types](https://arxiv.org/abs/1406.2061)
- [Algebraic Effects for Functional Programming](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/08/algeff-tr-2016-v2.pdf)
