# Remaining Compiler Phases - Implementation Plan

This document outlines the implementation plan for the remaining compiler phases after Phase 2 (Symbol Graph Builder).

## Current Status

| Phase | Name | Status | Crate |
|-------|------|--------|-------|
| 1 | Parser | ✅ Complete | `covenant-parser` |
| 2 | Symbol Graph Builder | ✅ Complete | `covenant-symbols` |
| 3 | Effect Checker | ✅ Complete | `covenant-checker` |
| 4 | Type Checker | 🔄 Partial | `covenant-checker` |
| 5 | Requirement Validator | ❌ Not Started | — |
| 6 | IR Optimizer | ❌ Not Started | — |
| 7 | WASM Emitter | 🔄 Partial | `covenant-codegen` |

---

## Phase 3: Effect Checker

### Purpose
Compute effect closures and validate effect declarations (I2 invariant).

### Current State
`covenant-checker` has basic effect tracking in `effects.rs` but lacks:
- Transitive effect closure computation
- Validation that declared effects match computed closure
- Pure function validation (calling effectful code)

### Implementation Plan

#### Files to Modify
- `crates/covenant-checker/src/effects.rs` - Enhance with closure computation

#### New Data Structures
```rust
pub struct EffectClosure {
    /// Declared effects (from effects section)
    pub declared: HashSet<String>,
    /// Computed transitive closure
    pub computed: HashSet<String>,
    /// Whether this symbol is pure (no effects)
    pub is_pure: bool,
}

pub struct EffectCheckResult {
    /// Effect closures for all symbols
    pub closures: HashMap<SymbolId, EffectClosure>,
    /// Effect violations found
    pub violations: Vec<EffectError>,
}
```

#### Algorithm
```
Input: SymbolGraph from Phase 2

1. Extract declared effects from each symbol
2. Topological sort symbols by call graph
3. For each symbol (in topological order):
   - Start with declared effects
   - Add effects from all callees (transitive)
   - Store as computed closure
4. Validate I2:
   - declared ⊇ computed (declared must cover computed)
   - If pure (declared empty), computed must also be empty
```

#### Error Types
- `E-EFFECT-001`: Pure function calls effectful code
- `E-EFFECT-002`: Missing effect declaration
- `E-EFFECT-003`: Effect transitivity violation

#### Integration Points
- Input: `SymbolGraph` from `covenant-symbols`
- Output: `EffectCheckResult` used by Phase 4

#### Estimated Effort
- Modify `effects.rs`: ~200 lines
- Add tests: ~150 lines
- Integration: ~50 lines

---

## Phase 4: Type Checker

### Purpose
Annotate every expression with a type and validate type correctness.

### Current State
`covenant-checker/src/snippet_checker.rs` has:
- Basic type inference for compute/call/bind steps
- Primitive type resolution
- Local scope tracking

Missing:
- Full union type handling
- Match exhaustiveness checking
- Query result type inference
- Struct/enum field type validation
- Generic type instantiation

### Implementation Plan

#### Files to Modify
- `crates/covenant-checker/src/snippet_checker.rs` - Enhance type checking
- `crates/covenant-checker/src/types.rs` - Add type unification

#### New Features

##### 1. Union Type Handling
```rust
fn check_union_assignment(target: &ResolvedType, value: &ResolvedType) -> bool {
    match (target, value) {
        (ResolvedType::Union(variants), value) => {
            variants.iter().any(|v| types_compatible(v, value))
        }
        _ => types_compatible(target, value)
    }
}
```

##### 2. Match Exhaustiveness
```rust
fn check_exhaustiveness(match_step: &MatchStep, union_type: &ResolvedType) -> Vec<TypeError> {
    let ResolvedType::Union(variants) = union_type else { return vec![] };

    let covered: HashSet<_> = match_step.cases.iter()
        .filter_map(|c| match &c.pattern {
            MatchPattern::Variant { variant, .. } => Some(variant.clone()),
            MatchPattern::Wildcard => None,
        })
        .collect();

    let has_wildcard = match_step.cases.iter()
        .any(|c| matches!(c.pattern, MatchPattern::Wildcard));

    if has_wildcard { return vec![]; }

    let missing: Vec<_> = variants.iter()
        .filter(|v| !covered.contains(&v.name()))
        .collect();

    if !missing.is_empty() {
        vec![TypeError::NonExhaustiveMatch { missing }]
    } else {
        vec![]
    }
}
```

##### 3. Query Result Type Inference
```rust
fn infer_query_type(query: &QueryStep, symbol_graph: &SymbolGraph) -> ResolvedType {
    match &query.content {
        QueryContent::Covenant(cov) => {
            // Infer from target type and select clause
            let target_type = resolve_query_target(&cov.from, symbol_graph);
            match &cov.select {
                SnippetSelectClause::All => ResolvedType::List(Box::new(target_type)),
                SnippetSelectClause::Field(f) => extract_field_type(&target_type, f),
            }
        }
        QueryContent::Dialect(dialect) => {
            // Use declared returns type
            resolve_return_type(&dialect.returns)
        }
    }
}
```

#### Error Types
- `E-TYPE-001`: Type mismatch
- `E-TYPE-002`: Undefined type
- `E-TYPE-003`: Incompatible union
- `E-TYPE-004`: Non-exhaustive match

#### Integration Points
- Input: `SymbolGraph` + `EffectCheckResult`
- Output: Typed AST with annotations

#### Estimated Effort
- Enhance `snippet_checker.rs`: ~300 lines
- Add `types.rs` unification: ~200 lines
- Add tests: ~250 lines

---

## Phase 5: Requirement Validator

### Purpose
Validate that all requirements have test coverage, build coverage report.

### Current State
Not implemented. The AST has `RequiresSection` and `TestsSection` but no validation.

### Implementation Plan

#### New Crate
Create `crates/covenant-requirements/` with:
```
src/
  lib.rs           # Public API
  validator.rs     # Coverage validation
  report.rs        # Coverage report generation
```

#### Data Structures
```rust
pub struct RequirementInfo {
    pub id: String,
    pub text: Option<String>,
    pub priority: Priority,
    pub status: ReqStatus,
    pub covered_by: Vec<String>,  // Test IDs
}

pub struct TestInfo {
    pub id: String,
    pub kind: TestKind,
    pub covers: Vec<String>,  // Requirement IDs
    pub snippet_id: String,   // Parent snippet
}

pub struct CoverageReport {
    pub requirements: HashMap<String, RequirementInfo>,
    pub tests: HashMap<String, TestInfo>,
    pub summary: CoverageSummary,
}

pub struct CoverageSummary {
    pub total_requirements: usize,
    pub covered: usize,
    pub uncovered: usize,
    pub coverage_percent: f64,
}
```

#### Algorithm
```
Input: Typed AST from Phase 4

1. Extract all requirements from snippets
2. Extract all tests from snippets
3. Build coverage links:
   - For each test with `covers` attribute
   - Link test -> requirements
   - Link requirements -> tests (bidirectional)
4. Validate I3 (coverage linkage bidirectionality)
5. Report uncovered requirements:
   - Critical priority -> Error
   - High priority -> Warning
   - Medium/Low -> Info
```

#### Error Types
- `E-REQ-001`: Uncovered requirement (severity by priority)
- `E-REQ-002`: Test references nonexistent requirement

#### CLI Integration
Add `covenant requirements` command:
```
covenant requirements examples/*.cov --report json
covenant requirements examples/*.cov --uncovered-only
```

#### Estimated Effort
- New crate: ~400 lines
- CLI integration: ~50 lines
- Tests: ~150 lines

---

## Phase 6: IR Optimizer

### Purpose
Optimize IR for performance, emit warnings about inefficiencies.

### Current State
Not implemented. No optimization passes exist.

### Implementation Plan

#### New Crate
Create `crates/covenant-optimizer/` with:
```
src/
  lib.rs              # Public API
  passes/
    mod.rs            # Pass trait and registry
    dead_code.rs      # Dead code elimination
    constant_fold.rs  # Constant folding
    unused_binding.rs # Unused binding detection
  analysis/
    mod.rs
    reachability.rs   # Reachability analysis
    usage.rs          # Binding usage analysis
```

#### Pass Trait
```rust
pub trait OptimizationPass {
    fn name(&self) -> &'static str;
    fn run(&self, ir: &mut OptimizableIR, ctx: &mut OptContext) -> PassResult;
}

pub struct PassResult {
    pub modified: bool,
    pub warnings: Vec<OptWarning>,
}

pub struct OptContext {
    pub symbol_graph: &SymbolGraph,
    pub settings: OptSettings,
}

pub struct OptSettings {
    pub level: OptLevel,  // O0, O1, O2, O3
    pub enable_warnings: bool,
}
```

#### Optimization Passes

##### 1. Dead Code Elimination
```rust
pub struct DeadCodeElimination;

impl OptimizationPass for DeadCodeElimination {
    fn run(&self, ir: &mut OptimizableIR, ctx: &mut OptContext) -> PassResult {
        let reachable = compute_reachable_steps(ir);
        let mut warnings = vec![];

        ir.steps.retain(|step| {
            if reachable.contains(&step.id) {
                true
            } else {
                warnings.push(OptWarning::UnreachableCode { step_id: step.id.clone() });
                false
            }
        });

        PassResult { modified: !warnings.is_empty(), warnings }
    }
}
```

##### 2. Constant Folding
```rust
pub struct ConstantFolding;

impl OptimizationPass for ConstantFolding {
    fn run(&self, ir: &mut OptimizableIR, ctx: &mut OptContext) -> PassResult {
        let mut modified = false;

        for step in &mut ir.steps {
            if let StepKind::Compute(compute) = &mut step.kind {
                if let Some(result) = try_fold_constant(compute) {
                    step.kind = StepKind::Bind(BindStep {
                        source: BindSource::Lit(result),
                        span: compute.span,
                    });
                    modified = true;
                }
            }
        }

        PassResult { modified, warnings: vec![] }
    }
}
```

##### 3. Unused Binding Detection
```rust
pub struct UnusedBindingDetection;

impl OptimizationPass for UnusedBindingDetection {
    fn run(&self, ir: &mut OptimizableIR, ctx: &mut OptContext) -> PassResult {
        let used = collect_used_bindings(ir);
        let mut warnings = vec![];

        for step in &ir.steps {
            if step.output_binding != "_" && !used.contains(&step.output_binding) {
                warnings.push(OptWarning::UnusedBinding {
                    name: step.output_binding.clone(),
                    step_id: step.id.clone(),
                });
            }
        }

        PassResult { modified: false, warnings }
    }
}
```

#### Warning Types
- `W-DEAD-001`: Unused binding
- `W-DEAD-002`: Unreachable code
- `W-DEAD-003`: Uncalled function
- `W-PERF-001`: Inefficient query

#### CLI Integration
```
covenant compile --optimize=2 file.cov
covenant compile --no-optimize file.cov
```

#### Estimated Effort
- New crate: ~600 lines
- Passes: ~400 lines total
- Tests: ~300 lines

---

## Phase 7: WASM Emitter

### Purpose
Generate WebAssembly binary from optimized IR.

### Current State
`covenant-codegen` has:
- Basic WASM emission for pure functions
- Type mapping to WASM types
- Simple expression compilation

Missing:
- Effect handling (WASI imports)
- Query compilation (SQL generation)
- Match/if control flow
- Struct/enum memory layout
- Function table for indirect calls

### Implementation Plan

#### Files to Modify
- `crates/covenant-codegen/src/snippet_wasm.rs` - Enhance snippet compilation
- `crates/covenant-codegen/src/wasm.rs` - Add control flow

#### New Features

##### 1. Effect Handling
```rust
fn emit_effect_imports(module: &mut Module, effects: &[String]) {
    for effect in effects {
        match effect.as_str() {
            "database" => {
                module.import_func("covenant_db", "execute_query",
                    &[ValType::I32, ValType::I32], &[ValType::I32]);
            }
            "network" => {
                module.import_func("covenant_http", "fetch",
                    &[ValType::I32, ValType::I32], &[ValType::I32]);
            }
            "filesystem" => {
                module.import_func("wasi_snapshot_preview1", "fd_write",
                    &[ValType::I32, ValType::I32, ValType::I32, ValType::I32],
                    &[ValType::I32]);
            }
            _ => {}
        }
    }
}
```

##### 2. Control Flow
```rust
fn emit_if_step(&mut self, if_step: &IfStep) {
    // Load condition
    self.emit_local_get(&if_step.condition);

    // if block
    self.func.instruction(&Instruction::If(BlockType::Empty));
    for step in &if_step.then_steps {
        self.emit_step(step);
    }

    // else block
    if let Some(else_steps) = &if_step.else_steps {
        self.func.instruction(&Instruction::Else);
        for step in else_steps {
            self.emit_step(step);
        }
    }

    self.func.instruction(&Instruction::End);
}

fn emit_match_step(&mut self, match_step: &MatchStep) {
    // Get discriminant
    self.emit_local_get(&match_step.on);
    self.emit_field_access("_tag"); // Enum tag

    // br_table for variant dispatch
    let labels: Vec<u32> = (0..match_step.cases.len() as u32).collect();
    self.func.instruction(&Instruction::BrTable(
        labels.clone().into(),
        labels.len() as u32 - 1
    ));

    // Emit each case block
    for (i, case) in match_step.cases.iter().enumerate() {
        self.func.instruction(&Instruction::Block(BlockType::Empty));
        for step in &case.steps {
            self.emit_step(step);
        }
        self.func.instruction(&Instruction::End);
    }
}
```

##### 3. SQL Query Compilation
```rust
fn emit_query_step(&mut self, query: &QueryStep) {
    match &query.content {
        QueryContent::Dialect(dialect) => {
            // Store SQL string in data segment
            let sql_offset = self.data_segment.add_string(&dialect.body);
            let sql_len = dialect.body.len();

            // Push SQL pointer and length
            self.func.instruction(&Instruction::I32Const(sql_offset as i32));
            self.func.instruction(&Instruction::I32Const(sql_len as i32));

            // Push parameter count and values
            self.func.instruction(&Instruction::I32Const(dialect.params.len() as i32));
            for param in &dialect.params {
                self.emit_local_get(&param.from);
            }

            // Call runtime query function
            self.func.instruction(&Instruction::Call(self.db_execute_idx));
        }
        QueryContent::Covenant(cov) => {
            // Generate SQL from Covenant query
            let sql = self.generate_sql_from_covenant(cov);
            // ... same as above
        }
    }
}
```

##### 4. Memory Layout for Structs
```rust
struct StructLayout {
    size: u32,
    alignment: u32,
    fields: HashMap<String, FieldLayout>,
}

struct FieldLayout {
    offset: u32,
    size: u32,
    ty: WasmType,
}

fn compute_struct_layout(struct_type: &ResolvedType) -> StructLayout {
    let mut offset = 0u32;
    let mut fields = HashMap::new();

    if let ResolvedType::Struct(struct_fields) = struct_type {
        for (name, ty) in struct_fields {
            let wasm_ty = to_wasm_type(ty);
            let size = wasm_ty.size();
            let align = wasm_ty.alignment();

            // Align offset
            offset = (offset + align - 1) & !(align - 1);

            fields.insert(name.clone(), FieldLayout { offset, size, ty: wasm_ty });
            offset += size;
        }
    }

    StructLayout { size: offset, alignment: 4, fields }
}
```

#### Runtime Requirements
The WASM module will need a runtime library providing:
- `covenant_db.execute_query(sql_ptr, sql_len, param_count, ...) -> result_ptr`
- `covenant_http.fetch(url_ptr, url_len) -> response_ptr`
- Memory allocator functions

#### Estimated Effort
- Enhance `snippet_wasm.rs`: ~500 lines
- Add control flow: ~200 lines
- Add SQL generation: ~300 lines
- Add struct layout: ~150 lines
- Tests: ~400 lines

---

## Implementation Order

### Recommended Sequence

1. **Phase 3: Effect Checker** (1-2 days)
   - Depends on: Phase 2 ✅
   - Enables: Proper pure/effectful validation

2. **Phase 4: Type Checker Enhancements** (2-3 days)
   - Depends on: Phase 3
   - Enables: Full type safety

3. **Phase 5: Requirement Validator** (1-2 days)
   - Depends on: Phase 4
   - Can be done in parallel with Phase 6

4. **Phase 6: IR Optimizer** (2-3 days)
   - Depends on: Phase 4
   - Can be done in parallel with Phase 5

5. **Phase 7: WASM Emitter Enhancements** (3-4 days)
   - Depends on: Phases 4, 6
   - Final phase, requires all prior work

### Total Estimated Effort
- Phase 3: ~400 lines
- Phase 4: ~750 lines
- Phase 5: ~600 lines
- Phase 6: ~1300 lines
- Phase 7: ~1550 lines
- **Total: ~4600 lines**

---

## Testing Strategy

### Unit Tests
Each phase should have unit tests covering:
- Happy path (valid input)
- Error cases (each error type)
- Edge cases (empty input, large input)

### Integration Tests
- End-to-end compilation of example files
- Verify WASM output runs correctly
- Performance benchmarks

### Property-Based Tests
- Random IR generation for optimizer testing
- Fuzzing for parser/type checker robustness

---

## CLI Enhancements

### New Commands
```bash
# Show effect analysis
covenant effects file.cov

# Show requirement coverage
covenant requirements file.cov --report markdown

# Optimize without compiling
covenant optimize file.cov --level 2 --output optimized.cov

# Compile with options
covenant compile file.cov --optimize=3 --debug-info
```

### Enhanced Output
```bash
$ covenant check file.cov --verbose
Phase 1: Parsing...          ✓ (12ms)
Phase 2: Symbol Graph...     ✓ 45 symbols (8ms)
Phase 3: Effect Checking...  ✓ 12 pure, 8 effectful (5ms)
Phase 4: Type Checking...    ✓ (15ms)
Phase 5: Requirements...     ✓ 100% coverage (3ms)
Total: 43ms
```
