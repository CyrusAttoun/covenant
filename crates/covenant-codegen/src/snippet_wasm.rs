//! WASM code generation for snippet-mode programs
//!
//! Compiles Covenant snippets to WebAssembly with support for:
//! - Pure functions (no effects)
//! - Effectful functions (with WASI imports)
//! - Control flow (if, match, for)
//! - SQL query compilation
//! - Struct/enum memory layout

use std::collections::HashMap;
use wasm_encoder::{
    BlockType, CodeSection, DataSection, ExportKind, ExportSection, Function, FunctionSection,
    GlobalSection, GlobalType, ImportSection, Instruction, MemorySection, MemoryType, Module,
    TypeSection, ValType,
};
use covenant_ast::{
    BindSource, BindStep, CallStep, ComputeStep, EffectsSection, ForStep, FunctionSignature,
    InputSource, IfStep, Literal, MatchPattern, MatchStep, Operation, QueryContent,
    QueryStep, ReturnStep, ReturnType, ReturnValue, Section, SignatureKind, Snippet, SnippetKind,
    Step, StepKind, StructConstruction, Type, TypeKind,
};
use covenant_checker::SymbolTable;
use crate::CodegenError;

// ===== Memory Layout Types =====

/// Layout information for struct fields
#[derive(Debug, Clone)]
pub struct FieldLayout {
    /// Offset from struct base pointer
    pub offset: u32,
    /// Size in bytes
    pub size: u32,
    /// WASM type for this field
    pub wasm_type: WasmType,
}

/// Layout information for structs
#[derive(Debug, Clone)]
pub struct StructLayout {
    /// Total size in bytes
    pub size: u32,
    /// Alignment requirement
    pub alignment: u32,
    /// Field layouts by name
    pub fields: HashMap<String, FieldLayout>,
}

/// WASM type representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmType {
    I32,
    I64,
    F32,
    F64,
    /// Pointer to memory (represented as i32)
    Ptr,
}

impl WasmType {
    pub fn size(&self) -> u32 {
        match self {
            WasmType::I32 | WasmType::F32 | WasmType::Ptr => 4,
            WasmType::I64 | WasmType::F64 => 8,
        }
    }

    pub fn alignment(&self) -> u32 {
        self.size()
    }

    pub fn to_valtype(&self) -> ValType {
        match self {
            WasmType::I32 | WasmType::Ptr => ValType::I32,
            WasmType::I64 => ValType::I64,
            WasmType::F32 => ValType::F32,
            WasmType::F64 => ValType::F64,
        }
    }
}

// ===== Data Segment Builder =====

/// Manages WASM data segment for storing strings and SQL queries
#[derive(Debug, Default)]
pub struct DataSegmentBuilder {
    /// The accumulated data bytes
    data: Vec<u8>,
    /// String offset cache to avoid duplicates
    string_offsets: HashMap<String, u32>,
}

impl DataSegmentBuilder {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            string_offsets: HashMap::new(),
        }
    }

    /// Add a string and return its offset
    pub fn add_string(&mut self, s: &str) -> u32 {
        if let Some(&offset) = self.string_offsets.get(s) {
            return offset;
        }

        let offset = self.data.len() as u32;
        self.data.extend_from_slice(s.as_bytes());
        self.data.push(0); // Null terminator
        self.string_offsets.insert(s.to_string(), offset);
        offset
    }

    /// Get the data and total length
    pub fn finish(self) -> Vec<u8> {
        self.data
    }

    /// Check if segment has any data
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

// ===== Import Tracker =====

/// Tracks required imports based on effects
#[derive(Debug, Default)]
struct ImportTracker {
    /// Import entries: (module, name, param_types, result_types)
    imports: Vec<(String, String, Vec<ValType>, Vec<ValType>)>,
    /// Map from (module.name) to function index
    import_indices: HashMap<String, u32>,
}

impl ImportTracker {
    fn new() -> Self {
        Self {
            imports: Vec::new(),
            import_indices: HashMap::new(),
        }
    }

    /// Add an import and return its function index
    fn add_import(
        &mut self,
        module: &str,
        name: &str,
        params: Vec<ValType>,
        results: Vec<ValType>,
    ) -> u32 {
        let key = format!("{}.{}", module, name);
        if let Some(&idx) = self.import_indices.get(&key) {
            return idx;
        }

        let idx = self.imports.len() as u32;
        self.imports
            .push((module.to_string(), name.to_string(), params, results));
        self.import_indices.insert(key, idx);
        idx
    }

    fn len(&self) -> u32 {
        self.imports.len() as u32
    }
}

// ===== Main Compiler =====

/// WASM compiler for snippet-mode programs
pub struct SnippetWasmCompiler<'a> {
    #[allow(dead_code)]
    symbols: &'a SymbolTable,
    /// Function name to index mapping (adjusted for imports)
    function_indices: HashMap<String, u32>,
    /// Local variable indices per function
    locals: HashMap<String, u32>,
    /// Current local count
    local_count: u32,
    /// Import tracker
    imports: ImportTracker,
    /// Data segment builder
    data_segment: DataSegmentBuilder,
    /// Struct layouts by type name
    struct_layouts: HashMap<String, StructLayout>,
    /// Runtime function indices (set after imports are processed)
    runtime: RuntimeFunctions,
}

/// Runtime function indices for effectful operations
#[derive(Debug, Default, Clone)]
struct RuntimeFunctions {
    /// Database query execution: covenant_db.execute_query(sql_ptr, sql_len, param_count) -> result_ptr
    db_execute_query: Option<u32>,
    /// HTTP fetch: covenant_http.fetch(url_ptr, url_len) -> response_ptr
    http_fetch: Option<u32>,
    /// Console print: covenant_io.print(ptr, len)
    io_print: Option<u32>,
    /// Memory allocation: covenant_mem.alloc(size) -> ptr
    mem_alloc: Option<u32>,
    /// WASI fd_write for filesystem
    wasi_fd_write: Option<u32>,

    // --- Text/String operations (covenant_text module) ---
    // Unary -> String: (ptr, len) -> i64 fat pointer
    text_upper: Option<u32>,
    text_lower: Option<u32>,
    text_trim: Option<u32>,
    text_trim_start: Option<u32>,
    text_trim_end: Option<u32>,
    text_str_reverse: Option<u32>,
    // Unary -> Int: (ptr, len) -> i64
    text_str_len: Option<u32>,
    text_byte_len: Option<u32>,
    text_is_empty: Option<u32>,
    // Binary -> String: (ptr1, len1, ptr2, len2) -> i64 fat pointer
    text_concat: Option<u32>,
    // Binary -> Bool: (ptr1, len1, ptr2, len2) -> i64 (0/1)
    text_contains: Option<u32>,
    text_starts_with: Option<u32>,
    text_ends_with: Option<u32>,
    // Binary -> Int: (ptr1, len1, ptr2, len2) -> i64
    text_index_of: Option<u32>,
    // Slice: (ptr, len, start, end) -> i64 fat pointer
    text_slice: Option<u32>,
    // CharAt: (ptr, len, idx) -> i64 fat pointer
    text_char_at: Option<u32>,
    // Replace: (s_ptr, s_len, from_ptr, from_len, to_ptr, to_len) -> i64 fat pointer
    text_replace: Option<u32>,
    // Split: (ptr, len, delim_ptr, delim_len) -> i64 fat pointer (serialized array)
    text_split: Option<u32>,
    // Join: (arr_ptr, arr_len, sep_ptr, sep_len) -> i64 fat pointer
    text_join: Option<u32>,
    // Repeat: (ptr, len, count) -> i64 fat pointer
    text_repeat: Option<u32>,
    // Pad: (ptr, len, target_len, fill_ptr, fill_len) -> i64 fat pointer
    text_pad_start: Option<u32>,
    text_pad_end: Option<u32>,

    // --- Regex operations (covenant_text module) ---
    text_regex_test: Option<u32>,
    text_regex_match: Option<u32>,
    text_regex_replace: Option<u32>,
    text_regex_replace_all: Option<u32>,
    text_regex_split: Option<u32>,
}

impl<'a> SnippetWasmCompiler<'a> {
    pub fn new(symbols: &'a SymbolTable) -> Self {
        Self {
            symbols,
            function_indices: HashMap::new(),
            locals: HashMap::new(),
            local_count: 0,
            imports: ImportTracker::new(),
            data_segment: DataSegmentBuilder::new(),
            struct_layouts: HashMap::new(),
            runtime: RuntimeFunctions::default(),
        }
    }

    /// Compile snippets to WASM
    pub fn compile_snippets(&mut self, snippets: &[Snippet]) -> Result<Vec<u8>, CodegenError> {
        let mut module = Module::new();

        // Collect all function snippets (both pure and effectful)
        let functions: Vec<&Snippet> = snippets
            .iter()
            .filter(|s| s.kind == SnippetKind::Function)
            .collect();

        if functions.is_empty() {
            // Return minimal valid WASM module
            return Ok(module.finish());
        }

        // First pass: collect all effects and register imports
        let all_effects = collect_all_effects(&functions);
        self.register_effect_imports(&all_effects);

        // Register text imports if any snippet uses string operations
        let has_string_ops = functions.iter().any(|s| snippet_has_string_ops(s));
        if has_string_ops {
            self.register_text_imports();
        }

        // Pre-scan for string literals to determine if we need memory
        let has_strings = functions.iter().any(|s| snippet_has_string_literals(s));

        // Build type section (imports first, then functions)
        let mut types = TypeSection::new();

        // Add import types
        for (_, _, params, results) in &self.imports.imports {
            types.function(params.clone(), results.clone());
        }

        // Add function types
        for snippet in &functions {
            if let Some(sig) = find_function_signature(snippet) {
                let params: Vec<ValType> = sig
                    .params
                    .iter()
                    .filter_map(|p| self.type_to_valtype(&p.ty))
                    .collect();

                let results: Vec<ValType> = sig
                    .returns
                    .as_ref()
                    .and_then(|r| self.return_type_to_valtype(r))
                    .map(|t| vec![t])
                    .unwrap_or_default();

                types.function(params, results);
            }
        }
        module.section(&types);

        // Import section (if there are any imports)
        if !self.imports.imports.is_empty() {
            let mut import_section = ImportSection::new();
            for (i, (mod_name, func_name, _, _)) in self.imports.imports.iter().enumerate() {
                import_section.import(mod_name, func_name, wasm_encoder::EntityType::Function(i as u32));
            }
            module.section(&import_section);
        }

        // Build function index map (accounting for imports)
        let import_count = self.imports.len();
        for (i, snippet) in functions.iter().enumerate() {
            if let Some(sig) = find_function_signature(snippet) {
                self.function_indices
                    .insert(sig.name.clone(), import_count + i as u32);
                // Also map by snippet ID for fully-qualified calls
                self.function_indices
                    .insert(snippet.id.clone(), import_count + i as u32);
            }
        }

        // Function section
        let mut func_section = FunctionSection::new();
        for i in 0..functions.len() {
            func_section.function(import_count + i as u32);
        }
        module.section(&func_section);

        // Memory section - always export memory when compiling functions
        // The runtime requires exported memory to run WASM modules
        let needs_memory = !functions.is_empty()
            || !self.data_segment.is_empty()
            || !all_effects.is_empty()
            || has_strings;
        if needs_memory {
            let mut memory = MemorySection::new();
            // 1 page = 64KB, start with 16 pages (1MB)
            memory.memory(MemoryType {
                minimum: 16,
                maximum: Some(256), // 16MB max
                memory64: false,
                shared: false,
            });
            module.section(&memory);
        }

        // Global section for heap pointer
        if needs_memory {
            let mut globals = GlobalSection::new();
            // Heap pointer starts after data segment
            let heap_start = self.data_segment.data.len() as i32;
            globals.global(
                GlobalType {
                    val_type: ValType::I32,
                    mutable: true,
                },
                &wasm_encoder::ConstExpr::i32_const(heap_start),
            );
            module.section(&globals);
        }

        // Export section
        let mut exports = ExportSection::new();
        for (i, snippet) in functions.iter().enumerate() {
            if let Some(sig) = find_function_signature(snippet) {
                exports.export(&sig.name, ExportKind::Func, import_count + i as u32);
            }
        }
        // Export memory if present
        if needs_memory {
            exports.export("memory", ExportKind::Memory, 0);
        }
        module.section(&exports);

        // Code section
        let mut codes = CodeSection::new();
        for snippet in &functions {
            let wasm_func = self.compile_function_snippet(snippet)?;
            codes.function(&wasm_func);
        }
        module.section(&codes);

        // Data section (if we have string constants or SQL queries)
        if !self.data_segment.is_empty() {
            let mut data = DataSection::new();
            let segment_data = std::mem::take(&mut self.data_segment).finish();
            data.active(
                0, // Memory index
                &wasm_encoder::ConstExpr::i32_const(0),
                segment_data,
            );
            module.section(&data);
        }

        Ok(module.finish())
    }

    /// Register imports for the given effects
    fn register_effect_imports(&mut self, effects: &[String]) {
        for effect in effects {
            match effect.as_str() {
                "database" => {
                    self.runtime.db_execute_query = Some(self.imports.add_import(
                        "covenant_db",
                        "execute_query",
                        vec![ValType::I32, ValType::I32, ValType::I32], // sql_ptr, sql_len, param_count
                        vec![ValType::I32],                             // result_ptr
                    ));
                }
                "network" => {
                    self.runtime.http_fetch = Some(self.imports.add_import(
                        "covenant_http",
                        "fetch",
                        vec![ValType::I32, ValType::I32], // url_ptr, url_len
                        vec![ValType::I32],              // response_ptr
                    ));
                }
                "filesystem" => {
                    self.runtime.wasi_fd_write = Some(self.imports.add_import(
                        "wasi_snapshot_preview1",
                        "fd_write",
                        vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
                        vec![ValType::I32],
                    ));
                }
                "console" | "std.io" => {
                    self.runtime.io_print = Some(self.imports.add_import(
                        "covenant_io",
                        "print",
                        vec![ValType::I32, ValType::I32], // ptr, len
                        vec![],
                    ));
                }
                _ => {
                    // Unknown effect - might be user-defined, skip
                }
            }
        }

        // Always add memory allocator if we have any effects
        if !effects.is_empty() {
            self.runtime.mem_alloc = Some(self.imports.add_import(
                "covenant_mem",
                "alloc",
                vec![ValType::I32], // size
                vec![ValType::I32], // ptr
            ));
        }
    }

    /// Register imports for text/string operations (effectless — triggered by AST scan)
    fn register_text_imports(&mut self) {
        // Ensure memory allocator is available (needed for host to write results)
        if self.runtime.mem_alloc.is_none() {
            self.runtime.mem_alloc = Some(self.imports.add_import(
                "covenant_mem",
                "alloc",
                vec![ValType::I32],
                vec![ValType::I32],
            ));
        }

        // Unary -> String: (ptr, len) -> i64 fat pointer
        self.runtime.text_upper = Some(self.imports.add_import(
            "covenant_text", "upper",
            vec![ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_lower = Some(self.imports.add_import(
            "covenant_text", "lower",
            vec![ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_trim = Some(self.imports.add_import(
            "covenant_text", "trim",
            vec![ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_trim_start = Some(self.imports.add_import(
            "covenant_text", "trim_start",
            vec![ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_trim_end = Some(self.imports.add_import(
            "covenant_text", "trim_end",
            vec![ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_str_reverse = Some(self.imports.add_import(
            "covenant_text", "str_reverse",
            vec![ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));

        // Unary -> Int: (ptr, len) -> i64
        self.runtime.text_str_len = Some(self.imports.add_import(
            "covenant_text", "str_len",
            vec![ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_byte_len = Some(self.imports.add_import(
            "covenant_text", "byte_len",
            vec![ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_is_empty = Some(self.imports.add_import(
            "covenant_text", "is_empty",
            vec![ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));

        // Binary -> String: (ptr1, len1, ptr2, len2) -> i64 fat pointer
        self.runtime.text_concat = Some(self.imports.add_import(
            "covenant_text", "concat",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));

        // Binary -> Bool (as i64): (ptr1, len1, ptr2, len2) -> i64
        self.runtime.text_contains = Some(self.imports.add_import(
            "covenant_text", "contains",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_starts_with = Some(self.imports.add_import(
            "covenant_text", "starts_with",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_ends_with = Some(self.imports.add_import(
            "covenant_text", "ends_with",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));

        // Binary -> Int: (ptr1, len1, ptr2, len2) -> i64
        self.runtime.text_index_of = Some(self.imports.add_import(
            "covenant_text", "index_of",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));

        // Slice: (ptr, len, start, end) -> i64 fat pointer
        self.runtime.text_slice = Some(self.imports.add_import(
            "covenant_text", "slice",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));

        // CharAt: (ptr, len, idx) -> i64 fat pointer
        self.runtime.text_char_at = Some(self.imports.add_import(
            "covenant_text", "char_at",
            vec![ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));

        // Replace: (s_ptr, s_len, from_ptr, from_len, to_ptr, to_len) -> i64 fat pointer
        self.runtime.text_replace = Some(self.imports.add_import(
            "covenant_text", "replace",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));

        // Split: (ptr, len, delim_ptr, delim_len) -> i64 fat pointer
        self.runtime.text_split = Some(self.imports.add_import(
            "covenant_text", "split",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));

        // Join: (arr_ptr, arr_len, sep_ptr, sep_len) -> i64 fat pointer
        self.runtime.text_join = Some(self.imports.add_import(
            "covenant_text", "join",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));

        // Repeat: (ptr, len, count) -> i64 fat pointer
        self.runtime.text_repeat = Some(self.imports.add_import(
            "covenant_text", "repeat",
            vec![ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));

        // Pad: (ptr, len, target_len, fill_ptr, fill_len) -> i64 fat pointer
        self.runtime.text_pad_start = Some(self.imports.add_import(
            "covenant_text", "pad_start",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_pad_end = Some(self.imports.add_import(
            "covenant_text", "pad_end",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));

        // Regex: (pat_ptr, pat_len, in_ptr, in_len) -> i64 (bool or fat pointer)
        self.runtime.text_regex_test = Some(self.imports.add_import(
            "covenant_text", "regex_test",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_regex_match = Some(self.imports.add_import(
            "covenant_text", "regex_match",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        // Regex replace: (pat_ptr, pat_len, in_ptr, in_len, rep_ptr, rep_len) -> i64
        self.runtime.text_regex_replace = Some(self.imports.add_import(
            "covenant_text", "regex_replace",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_regex_replace_all = Some(self.imports.add_import(
            "covenant_text", "regex_replace_all",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
        self.runtime.text_regex_split = Some(self.imports.add_import(
            "covenant_text", "regex_split",
            vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I64],
        ));
    }

    /// Compile a single function snippet
    fn compile_function_snippet(&mut self, snippet: &Snippet) -> Result<Function, CodegenError> {
        let sig = find_function_signature(snippet)
            .ok_or_else(|| CodegenError::UndefinedFunction {
                name: snippet.id.clone(),
            })?;

        let body = find_body_section(snippet);

        // Reset locals
        self.locals.clear();
        self.local_count = 0;

        // Add parameters as locals
        for param in &sig.params {
            self.locals.insert(param.name.clone(), self.local_count);
            self.local_count += 1;
        }

        // Count additional locals needed from step bindings
        let additional_locals = if let Some(body) = body {
            self.count_step_bindings(&body.steps)
        } else {
            0
        };

        let mut wasm_func = Function::new(vec![(additional_locals, ValType::I64)]);

        // Compile body steps
        if let Some(body) = body {
            for step in &body.steps {
                self.compile_step(step, &mut wasm_func)?;
            }
        }

        // If function returns a value, we need something on the stack for the implicit return.
        // Push a dummy value (0) in case all paths returned early via explicit returns.
        // The unreachable instruction would be better but let's keep it simple.
        if sig.returns.is_some() {
            wasm_func.instruction(&Instruction::I64Const(0));
        }

        // Add end instruction
        wasm_func.instruction(&Instruction::End);

        Ok(wasm_func)
    }

    /// Count the number of step bindings that need locals
    fn count_step_bindings(&self, steps: &[Step]) -> u32 {
        let mut count = 0;
        let mut needs_text_locals = false;
        for step in steps {
            if step.output_binding != "_" {
                count += 1;
            }
            // Count nested steps and special cases
            match &step.kind {
                StepKind::Compute(compute) => {
                    if is_string_operation(&compute.op) {
                        needs_text_locals = true;
                    }
                }
                StepKind::If(if_step) => {
                    count += self.count_step_bindings(&if_step.then_steps);
                    if let Some(else_steps) = &if_step.else_steps {
                        count += self.count_step_bindings(else_steps);
                    }
                }
                StepKind::Match(match_step) => {
                    for case in &match_step.cases {
                        // Count bindings from variant patterns
                        if let MatchPattern::Variant { bindings, .. } = &case.pattern {
                            count += bindings.len() as u32;
                        }
                        count += self.count_step_bindings(&case.steps);
                    }
                }
                StepKind::For(for_step) => {
                    // Count: index local, length local, item local
                    count += 3;
                    count += self.count_step_bindings(&for_step.steps);
                }
                StepKind::Call(call) => {
                    // Runtime calls to console.* need a temp local for fat pointer unpacking
                    if call.fn_name.starts_with("console.") {
                        count += 1;
                    }
                    // Regex calls need 2 temp locals per argument (ptr unpack)
                    if call.fn_name.starts_with("std.text.regex_") {
                        count += call.args.len() as u32;
                    }
                }
                _ => {}
            }
        }
        // Text operation temp locals (allocated by name, so max across all ops):
        // __text_unary, __text_bin_a, __text_bin_b, __text_slice_str/start/end,
        // __text_char_str/idx, __text_rep_str/from/to, __text_repeat_str/n,
        // __text_pad_str/tlen/fill = 15 unique names max
        if needs_text_locals {
            count += 15;
        }
        count
    }

    /// Compile a single step
    fn compile_step(&mut self, step: &Step, func: &mut Function) -> Result<(), CodegenError> {
        match &step.kind {
            StepKind::Compute(compute) => {
                self.compile_compute_step(compute, func)?;
                // Store result if not discarded
                if step.output_binding != "_" {
                    let local = self.allocate_local(&step.output_binding);
                    func.instruction(&Instruction::LocalSet(local));
                }
            }
            StepKind::Call(call) => {
                self.compile_call_step(call, func)?;
                // Store result if not discarded
                if step.output_binding != "_" {
                    let local = self.allocate_local(&step.output_binding);
                    func.instruction(&Instruction::LocalSet(local));
                }
            }
            StepKind::Return(ret) => {
                self.compile_return_step(ret, func)?;
                func.instruction(&Instruction::Return);
            }
            StepKind::If(if_step) => {
                self.compile_if_step(if_step, func)?;
            }
            StepKind::Bind(bind) => {
                self.compile_bind_step(bind, func)?;
                // Store result if not discarded
                if step.output_binding != "_" {
                    let local = self.allocate_local(&step.output_binding);
                    func.instruction(&Instruction::LocalSet(local));
                }
            }
            StepKind::Match(match_step) => {
                self.compile_match_step(match_step, &step.output_binding, func)?;
            }
            StepKind::For(for_step) => {
                self.compile_for_step(for_step, func)?;
            }
            StepKind::Query(query) => {
                self.compile_query_step(query, func)?;
                // Store result if not discarded
                if step.output_binding != "_" {
                    let local = self.allocate_local(&step.output_binding);
                    func.instruction(&Instruction::LocalSet(local));
                }
            }
            StepKind::Construct(construct) => {
                self.compile_construct_step(construct, func)?;
                // Store result if not discarded
                if step.output_binding != "_" {
                    let local = self.allocate_local(&step.output_binding);
                    func.instruction(&Instruction::LocalSet(local));
                }
            }
            StepKind::Insert(_) | StepKind::Update(_) | StepKind::Delete(_) |
            StepKind::Transaction(_) | StepKind::Traverse(_) => {
                // These require database effects and runtime support
                // For now, generate a placeholder call to runtime
                return Err(CodegenError::UnsupportedExpression);
            }
        }
        Ok(())
    }

    /// Compile a match step
    ///
    /// Match expressions are compiled to a series of if-else blocks for pattern matching.
    /// For enum variants, we use the tag field (first word) to dispatch.
    fn compile_match_step(
        &mut self,
        match_step: &MatchStep,
        output_binding: &str,
        func: &mut Function,
    ) -> Result<(), CodegenError> {
        // Get the value being matched (copy the index to avoid borrow issues)
        let match_local = *self.locals.get(&match_step.on)
            .ok_or_else(|| CodegenError::UndefinedFunction { name: match_step.on.clone() })?;

        // For simple integer/enum tag matching, we compile to a series of if-else
        // More complex pattern matching (structs, nested patterns) would need additional work

        let num_cases = match_step.cases.len();

        // Handle empty match (shouldn't happen in valid code)
        if num_cases == 0 {
            return Ok(());
        }

        // Compile each case as an if-else chain
        for (i, case) in match_step.cases.iter().enumerate() {
            match &case.pattern {
                MatchPattern::Variant { variant: _, bindings } => {
                    // Load the value (or its tag)
                    func.instruction(&Instruction::LocalGet(match_local));

                    // Use ordinal-based matching: case index maps to expected tag value
                    // This assumes enums use 0, 1, 2, ... for variant tags
                    let tag_value = i as i64;
                    func.instruction(&Instruction::I64Const(tag_value));
                    func.instruction(&Instruction::I64Eq);
                    func.instruction(&Instruction::If(BlockType::Empty));

                    // Set up bindings for destructured values
                    // For now, we assume single binding gets the value
                    if !bindings.is_empty() {
                        func.instruction(&Instruction::LocalGet(match_local));
                        let binding_local = self.allocate_local(&bindings[0]);
                        func.instruction(&Instruction::LocalSet(binding_local));
                    }

                    // Compile case body
                    for step in &case.steps {
                        self.compile_step(step, func)?;
                    }

                    // Add else if there are more cases
                    if i < num_cases - 1 {
                        func.instruction(&Instruction::Else);
                    }
                }
                MatchPattern::Wildcard => {
                    // Wildcard matches anything - just compile the body
                    // If this is the last case, it's the default
                    for step in &case.steps {
                        self.compile_step(step, func)?;
                    }
                }
            }
        }

        // Close all the if blocks
        for (i, case) in match_step.cases.iter().enumerate() {
            if !matches!(case.pattern, MatchPattern::Wildcard) {
                func.instruction(&Instruction::End);
            }
            // Only close non-wildcard cases that aren't the last
            if i < num_cases - 1 && matches!(case.pattern, MatchPattern::Variant { .. }) {
                // End was already added, nothing more needed
            }
        }

        // Store result if needed
        if output_binding != "_" {
            // Match results would need additional infrastructure to collect
            // For now, we leave this as a TODO for full implementation
        }

        Ok(())
    }

    /// Compile a for loop step
    ///
    /// For loops iterate over collections. We compile to a WASM loop with
    /// index tracking and bounds checking.
    fn compile_for_step(&mut self, for_step: &ForStep, func: &mut Function) -> Result<(), CodegenError> {
        // Get the collection (copy the index to avoid borrow issues)
        let collection_local = *self.locals.get(&for_step.collection)
            .ok_or_else(|| CodegenError::UndefinedFunction { name: for_step.collection.clone() })?;

        // Allocate locals for loop index and length
        let index_local = self.allocate_local(&format!("__for_idx_{}", for_step.var));
        let len_local = self.allocate_local(&format!("__for_len_{}", for_step.var));
        let item_local = self.allocate_local(&for_step.var);

        // Initialize index to 0
        func.instruction(&Instruction::I64Const(0));
        func.instruction(&Instruction::LocalSet(index_local));

        // Get collection length (assume it's stored as first word of collection struct)
        func.instruction(&Instruction::LocalGet(collection_local));
        // For now, assume length is accessible - in real implementation would need
        // proper collection type handling
        func.instruction(&Instruction::LocalSet(len_local));

        // Start loop block
        func.instruction(&Instruction::Block(BlockType::Empty)); // outer block for break
        func.instruction(&Instruction::Loop(BlockType::Empty)); // loop block

        // Check if index >= length (exit condition)
        func.instruction(&Instruction::LocalGet(index_local));
        func.instruction(&Instruction::LocalGet(len_local));
        func.instruction(&Instruction::I64GeS); // index >= length means exit
        // I64GeS returns i32 (0 or 1), convert to i32 for br_if
        func.instruction(&Instruction::BrIf(1)); // Break out if done (to outer block)

        // Get current item (would need proper collection indexing)
        // For now, just use the index as a placeholder
        func.instruction(&Instruction::LocalGet(index_local));
        func.instruction(&Instruction::LocalSet(item_local));

        // Compile loop body
        for step in &for_step.steps {
            self.compile_step(step, func)?;
        }

        // Increment index
        func.instruction(&Instruction::LocalGet(index_local));
        func.instruction(&Instruction::I64Const(1));
        func.instruction(&Instruction::I64Add);
        func.instruction(&Instruction::LocalSet(index_local));

        // Branch back to loop start
        func.instruction(&Instruction::Br(0));

        // End loop and outer block
        func.instruction(&Instruction::End); // end loop
        func.instruction(&Instruction::End); // end outer block

        Ok(())
    }

    /// Compile a struct construction step
    ///
    /// For simple structs (like Point with 2 Int fields), we pack the fields
    /// into a single i64: (field1 << 32) | (field2 & 0xFFFFFFFF)
    fn compile_construct_step(
        &mut self,
        construct: &StructConstruction,
        func: &mut Function,
    ) -> Result<(), CodegenError> {
        // For a 2-field struct like Point{x, y}, pack into single i64
        // This is a simplified MVP - real implementation would use memory allocation
        if construct.fields.len() == 2 {
            // Compile first field, shift left 32 bits
            self.compile_input(&construct.fields[0].value, func)?;
            func.instruction(&Instruction::I64Const(32));
            func.instruction(&Instruction::I64Shl);

            // Compile second field, mask to 32 bits
            self.compile_input(&construct.fields[1].value, func)?;
            func.instruction(&Instruction::I64Const(0xFFFFFFFF));
            func.instruction(&Instruction::I64And);

            // Combine with OR
            func.instruction(&Instruction::I64Or);
        } else if construct.fields.len() == 1 {
            // Single field struct - just use the field value
            self.compile_input(&construct.fields[0].value, func)?;
        } else {
            // For more complex structs, we'd need memory allocation
            // For now, just push 0 as placeholder
            func.instruction(&Instruction::I64Const(0));
        }

        Ok(())
    }

    // ===== Text Operation Compilation Helpers =====

    /// Compile a unary text operation: one i64 fat pointer on stack -> host call -> i64 result
    /// Stack before: [fat_ptr]
    /// Stack after: [result_i64]
    fn compile_unary_text_op(&mut self, func: &mut Function, import_idx: Option<u32>) -> Result<(), CodegenError> {
        let idx = import_idx.ok_or(CodegenError::UnsupportedExpression)?;
        let temp = self.allocate_local("__text_unary");

        // Store fat pointer
        func.instruction(&Instruction::LocalSet(temp));

        // Unpack: ptr = fat_ptr >> 32
        func.instruction(&Instruction::LocalGet(temp));
        func.instruction(&Instruction::I64Const(32));
        func.instruction(&Instruction::I64ShrU);
        func.instruction(&Instruction::I32WrapI64);

        // Unpack: len = fat_ptr & 0xFFFFFFFF
        func.instruction(&Instruction::LocalGet(temp));
        func.instruction(&Instruction::I32WrapI64);

        // Call host function
        func.instruction(&Instruction::Call(idx));
        Ok(())
    }

    /// Compile a binary text operation: two i64 fat pointers on stack -> host call -> i64 result
    /// Stack before: [fat_ptr_a, fat_ptr_b] (b on top)
    /// Stack after: [result_i64]
    fn compile_binary_text_op(&mut self, func: &mut Function, import_idx: Option<u32>) -> Result<(), CodegenError> {
        let idx = import_idx.ok_or(CodegenError::UnsupportedExpression)?;
        let temp_b = self.allocate_local("__text_bin_b");
        let temp_a = self.allocate_local("__text_bin_a");

        // Pop b then a
        func.instruction(&Instruction::LocalSet(temp_b));
        func.instruction(&Instruction::LocalSet(temp_a));

        // Unpack a: ptr, len
        func.instruction(&Instruction::LocalGet(temp_a));
        func.instruction(&Instruction::I64Const(32));
        func.instruction(&Instruction::I64ShrU);
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::LocalGet(temp_a));
        func.instruction(&Instruction::I32WrapI64);

        // Unpack b: ptr, len
        func.instruction(&Instruction::LocalGet(temp_b));
        func.instruction(&Instruction::I64Const(32));
        func.instruction(&Instruction::I64ShrU);
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::LocalGet(temp_b));
        func.instruction(&Instruction::I32WrapI64);

        // Call host function
        func.instruction(&Instruction::Call(idx));
        Ok(())
    }

    /// Compile slice(string, start, end): string fat ptr + two int args
    /// Stack before: [str_fat_ptr, start_i64, end_i64] (end on top)
    fn compile_slice_op(&mut self, func: &mut Function) -> Result<(), CodegenError> {
        let idx = self.runtime.text_slice.ok_or(CodegenError::UnsupportedExpression)?;
        let temp_end = self.allocate_local("__text_slice_end");
        let temp_start = self.allocate_local("__text_slice_start");
        let temp_str = self.allocate_local("__text_slice_str");

        func.instruction(&Instruction::LocalSet(temp_end));
        func.instruction(&Instruction::LocalSet(temp_start));
        func.instruction(&Instruction::LocalSet(temp_str));

        // Unpack string: ptr, len
        func.instruction(&Instruction::LocalGet(temp_str));
        func.instruction(&Instruction::I64Const(32));
        func.instruction(&Instruction::I64ShrU);
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::LocalGet(temp_str));
        func.instruction(&Instruction::I32WrapI64);

        // Start index as i32
        func.instruction(&Instruction::LocalGet(temp_start));
        func.instruction(&Instruction::I32WrapI64);

        // End index as i32
        func.instruction(&Instruction::LocalGet(temp_end));
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::Call(idx));
        Ok(())
    }

    /// Compile char_at(string, index): string fat ptr + int arg
    /// Stack before: [str_fat_ptr, idx_i64] (idx on top)
    fn compile_char_at_op(&mut self, func: &mut Function) -> Result<(), CodegenError> {
        let idx = self.runtime.text_char_at.ok_or(CodegenError::UnsupportedExpression)?;
        let temp_idx = self.allocate_local("__text_char_idx");
        let temp_str = self.allocate_local("__text_char_str");

        func.instruction(&Instruction::LocalSet(temp_idx));
        func.instruction(&Instruction::LocalSet(temp_str));

        // Unpack string: ptr, len
        func.instruction(&Instruction::LocalGet(temp_str));
        func.instruction(&Instruction::I64Const(32));
        func.instruction(&Instruction::I64ShrU);
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::LocalGet(temp_str));
        func.instruction(&Instruction::I32WrapI64);

        // Index as i32
        func.instruction(&Instruction::LocalGet(temp_idx));
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::Call(idx));
        Ok(())
    }

    /// Compile replace(string, from, to): three fat pointers
    /// Stack before: [str_fat_ptr, from_fat_ptr, to_fat_ptr] (to on top)
    fn compile_replace_op(&mut self, func: &mut Function) -> Result<(), CodegenError> {
        let idx = self.runtime.text_replace.ok_or(CodegenError::UnsupportedExpression)?;
        let temp_to = self.allocate_local("__text_rep_to");
        let temp_from = self.allocate_local("__text_rep_from");
        let temp_str = self.allocate_local("__text_rep_str");

        func.instruction(&Instruction::LocalSet(temp_to));
        func.instruction(&Instruction::LocalSet(temp_from));
        func.instruction(&Instruction::LocalSet(temp_str));

        // Unpack string: ptr, len
        func.instruction(&Instruction::LocalGet(temp_str));
        func.instruction(&Instruction::I64Const(32));
        func.instruction(&Instruction::I64ShrU);
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::LocalGet(temp_str));
        func.instruction(&Instruction::I32WrapI64);

        // Unpack from: ptr, len
        func.instruction(&Instruction::LocalGet(temp_from));
        func.instruction(&Instruction::I64Const(32));
        func.instruction(&Instruction::I64ShrU);
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::LocalGet(temp_from));
        func.instruction(&Instruction::I32WrapI64);

        // Unpack to: ptr, len
        func.instruction(&Instruction::LocalGet(temp_to));
        func.instruction(&Instruction::I64Const(32));
        func.instruction(&Instruction::I64ShrU);
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::LocalGet(temp_to));
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::Call(idx));
        Ok(())
    }

    /// Compile repeat(string, count): string fat ptr + int arg
    /// Stack before: [str_fat_ptr, count_i64] (count on top)
    fn compile_repeat_op(&mut self, func: &mut Function) -> Result<(), CodegenError> {
        let idx = self.runtime.text_repeat.ok_or(CodegenError::UnsupportedExpression)?;
        let temp_count = self.allocate_local("__text_repeat_n");
        let temp_str = self.allocate_local("__text_repeat_str");

        func.instruction(&Instruction::LocalSet(temp_count));
        func.instruction(&Instruction::LocalSet(temp_str));

        // Unpack string: ptr, len
        func.instruction(&Instruction::LocalGet(temp_str));
        func.instruction(&Instruction::I64Const(32));
        func.instruction(&Instruction::I64ShrU);
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::LocalGet(temp_str));
        func.instruction(&Instruction::I32WrapI64);

        // Count as i32
        func.instruction(&Instruction::LocalGet(temp_count));
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::Call(idx));
        Ok(())
    }

    /// Compile pad_start/pad_end(string, target_len, fill): string fat ptr + int + fat ptr
    /// Stack before: [str_fat_ptr, target_len_i64, fill_fat_ptr] (fill on top)
    fn compile_pad_op(&mut self, func: &mut Function, import_idx: Option<u32>) -> Result<(), CodegenError> {
        let idx = import_idx.ok_or(CodegenError::UnsupportedExpression)?;
        let temp_fill = self.allocate_local("__text_pad_fill");
        let temp_tlen = self.allocate_local("__text_pad_tlen");
        let temp_str = self.allocate_local("__text_pad_str");

        func.instruction(&Instruction::LocalSet(temp_fill));
        func.instruction(&Instruction::LocalSet(temp_tlen));
        func.instruction(&Instruction::LocalSet(temp_str));

        // Unpack string: ptr, len
        func.instruction(&Instruction::LocalGet(temp_str));
        func.instruction(&Instruction::I64Const(32));
        func.instruction(&Instruction::I64ShrU);
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::LocalGet(temp_str));
        func.instruction(&Instruction::I32WrapI64);

        // Target length as i32
        func.instruction(&Instruction::LocalGet(temp_tlen));
        func.instruction(&Instruction::I32WrapI64);

        // Unpack fill: ptr, len
        func.instruction(&Instruction::LocalGet(temp_fill));
        func.instruction(&Instruction::I64Const(32));
        func.instruction(&Instruction::I64ShrU);
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::LocalGet(temp_fill));
        func.instruction(&Instruction::I32WrapI64);

        func.instruction(&Instruction::Call(idx));
        Ok(())
    }

    /// Compile a query step
    ///
    /// Queries are compiled differently based on dialect:
    /// - Covenant queries are compiled to runtime calls
    /// - SQL dialect queries have their SQL stored in data segment
    fn compile_query_step(&mut self, query: &QueryStep, func: &mut Function) -> Result<(), CodegenError> {
        match &query.content {
            QueryContent::Dialect(dialect) => {
                // Store SQL in data segment
                let sql_offset = self.data_segment.add_string(&dialect.body);
                let sql_len = dialect.body.len();

                // Push SQL pointer and length
                func.instruction(&Instruction::I32Const(sql_offset as i32));
                func.instruction(&Instruction::I32Const(sql_len as i32));

                // Push parameter count
                func.instruction(&Instruction::I32Const(dialect.params.len() as i32));

                // Call database execute function if available
                if let Some(db_fn) = self.runtime.db_execute_query {
                    func.instruction(&Instruction::Call(db_fn));
                } else {
                    // No database runtime available - return 0
                    func.instruction(&Instruction::Drop);
                    func.instruction(&Instruction::Drop);
                    func.instruction(&Instruction::Drop);
                    func.instruction(&Instruction::I32Const(0));
                }
            }
            QueryContent::Covenant(cov) => {
                // Generate SQL from Covenant query syntax
                let sql = generate_sql_from_covenant(cov, &query.target);
                let sql_offset = self.data_segment.add_string(&sql);
                let sql_len = sql.len();

                // Push SQL pointer and length
                func.instruction(&Instruction::I32Const(sql_offset as i32));
                func.instruction(&Instruction::I32Const(sql_len as i32));

                // No parameters for simple Covenant queries
                func.instruction(&Instruction::I32Const(0));

                // Call database execute function if available
                if let Some(db_fn) = self.runtime.db_execute_query {
                    func.instruction(&Instruction::Call(db_fn));
                } else {
                    func.instruction(&Instruction::Drop);
                    func.instruction(&Instruction::Drop);
                    func.instruction(&Instruction::Drop);
                    func.instruction(&Instruction::I32Const(0));
                }
            }
        }

        // Convert result pointer to i64 for uniform local storage
        func.instruction(&Instruction::I64ExtendI32U);

        Ok(())
    }

    /// Compile a compute step
    fn compile_compute_step(&mut self, compute: &ComputeStep, func: &mut Function) -> Result<(), CodegenError> {
        // Push inputs onto stack
        for input in &compute.inputs {
            self.compile_input(&input.source, func)?;
        }

        // Emit operation instruction
        // Note: Comparison operations return i32, we extend to i64 for uniform storage
        match compute.op {
            Operation::Add => { func.instruction(&Instruction::I64Add); }
            Operation::Sub => { func.instruction(&Instruction::I64Sub); }
            Operation::Mul => { func.instruction(&Instruction::I64Mul); }
            Operation::Div => { func.instruction(&Instruction::I64DivS); }
            Operation::Mod => { func.instruction(&Instruction::I64RemS); }
            // Comparison ops return i32, extend to i64
            Operation::Equals => {
                func.instruction(&Instruction::I64Eq);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            Operation::NotEquals => {
                func.instruction(&Instruction::I64Ne);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            Operation::Less => {
                func.instruction(&Instruction::I64LtS);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            Operation::Greater => {
                func.instruction(&Instruction::I64GtS);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            Operation::LessEq => {
                func.instruction(&Instruction::I64LeS);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            Operation::GreaterEq => {
                func.instruction(&Instruction::I64GeS);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            // Boolean ops: we treat bools as i64 (0 or 1)
            Operation::And => {
                func.instruction(&Instruction::I64And);
            }
            Operation::Or => {
                func.instruction(&Instruction::I64Or);
            }
            Operation::Not => {
                // Not: check if value is zero, result is i32, extend to i64
                func.instruction(&Instruction::I64Eqz);
                func.instruction(&Instruction::I64ExtendI32U);
            }
            Operation::Neg => {
                // Negate: compute 0 - x
                // First swap the operand with 0
                func.instruction(&Instruction::I64Const(0));
                func.instruction(&Instruction::I64Sub);
            }
            // String operations: all compile to host calls via covenant_text imports.
            // Inputs are i64 fat pointers on the stack; we unpack to (i32 ptr, i32 len) pairs.
            Operation::Concat => {
                self.compile_binary_text_op(func, self.runtime.text_concat)?;
            }
            Operation::Contains => {
                self.compile_binary_text_op(func, self.runtime.text_contains)?;
            }
            Operation::StartsWith => {
                self.compile_binary_text_op(func, self.runtime.text_starts_with)?;
            }
            Operation::EndsWith => {
                self.compile_binary_text_op(func, self.runtime.text_ends_with)?;
            }
            Operation::IndexOf => {
                self.compile_binary_text_op(func, self.runtime.text_index_of)?;
            }
            Operation::Upper => {
                self.compile_unary_text_op(func, self.runtime.text_upper)?;
            }
            Operation::Lower => {
                self.compile_unary_text_op(func, self.runtime.text_lower)?;
            }
            Operation::Trim => {
                self.compile_unary_text_op(func, self.runtime.text_trim)?;
            }
            Operation::TrimStart => {
                self.compile_unary_text_op(func, self.runtime.text_trim_start)?;
            }
            Operation::TrimEnd => {
                self.compile_unary_text_op(func, self.runtime.text_trim_end)?;
            }
            Operation::StrReverse => {
                self.compile_unary_text_op(func, self.runtime.text_str_reverse)?;
            }
            Operation::StrLen => {
                self.compile_unary_text_op(func, self.runtime.text_str_len)?;
            }
            Operation::ByteLen => {
                self.compile_unary_text_op(func, self.runtime.text_byte_len)?;
            }
            Operation::IsEmpty => {
                self.compile_unary_text_op(func, self.runtime.text_is_empty)?;
            }
            Operation::Slice => {
                // Inputs: string (fat ptr), start (i64), end (i64)
                // Host sig: (ptr, len, start_i32, end_i32) -> i64
                self.compile_slice_op(func)?;
            }
            Operation::CharAt => {
                // Inputs: string (fat ptr), index (i64)
                // Host sig: (ptr, len, idx_i32) -> i64
                self.compile_char_at_op(func)?;
            }
            Operation::Replace => {
                // Inputs: string (fat ptr), from (fat ptr), to (fat ptr)
                // Host sig: (s_ptr, s_len, from_ptr, from_len, to_ptr, to_len) -> i64
                self.compile_replace_op(func)?;
            }
            Operation::Split => {
                self.compile_binary_text_op(func, self.runtime.text_split)?;
            }
            Operation::Join => {
                self.compile_binary_text_op(func, self.runtime.text_join)?;
            }
            Operation::Repeat => {
                // Inputs: string (fat ptr), count (i64)
                // Host sig: (ptr, len, count_i32) -> i64
                self.compile_repeat_op(func)?;
            }
            Operation::PadStart => {
                // Inputs: string (fat ptr), target_len (i64), fill (fat ptr)
                // Host sig: (ptr, len, target_len, fill_ptr, fill_len) -> i64
                self.compile_pad_op(func, self.runtime.text_pad_start)?;
            }
            Operation::PadEnd => {
                self.compile_pad_op(func, self.runtime.text_pad_end)?;
            }
            // All other operations are not yet supported in WASM codegen
            _ => {
                return Err(CodegenError::UnsupportedExpression);
            }
        }

        Ok(())
    }

    /// Compile a call step
    fn compile_call_step(&mut self, call: &CallStep, func: &mut Function) -> Result<(), CodegenError> {
        // Check for runtime/builtin functions first
        if let Some(idx) = self.try_compile_runtime_call(call, func)? {
            func.instruction(&Instruction::Call(idx));
            return Ok(());
        }

        // Regular user-defined function call
        // Push arguments onto stack
        for arg in &call.args {
            self.compile_input(&arg.source, func)?;
        }

        // Get function index
        let idx = self.function_indices.get(&call.fn_name)
            .ok_or_else(|| CodegenError::UndefinedFunction { name: call.fn_name.clone() })?;

        func.instruction(&Instruction::Call(*idx));
        Ok(())
    }

    /// Try to compile a call to a runtime/builtin function
    /// Returns Some(import_index) if this is a runtime function, None otherwise
    fn try_compile_runtime_call(&mut self, call: &CallStep, func: &mut Function) -> Result<Option<u32>, CodegenError> {
        match call.fn_name.as_str() {
            "console.println" | "console.print" | "console.error" => {
                // These functions take a String argument and call covenant_io.print
                let print_fn = self.runtime.io_print
                    .ok_or_else(|| CodegenError::UndefinedFunction {
                        name: "covenant_io.print (console effect not declared?)".to_string()
                    })?;

                // Compile the message argument - this produces an i64 fat pointer on the stack
                if let Some(arg) = call.args.first() {
                    self.compile_input(&arg.source, func)?;
                } else {
                    return Err(CodegenError::UndefinedFunction {
                        name: format!("{} requires a message argument", call.fn_name)
                    });
                }

                // The argument is an i64 fat pointer: (offset << 32) | len
                // We need to unpack it into two i32 values for the print import
                // Allocate a temp local if we don't have one
                let temp_local = self.allocate_local("__temp_fat_ptr");

                // Store the fat pointer to temp local
                func.instruction(&Instruction::LocalSet(temp_local));

                // Extract offset (shift right 32, wrap to i32)
                func.instruction(&Instruction::LocalGet(temp_local));
                func.instruction(&Instruction::I64Const(32));
                func.instruction(&Instruction::I64ShrU);
                func.instruction(&Instruction::I32WrapI64);

                // Extract length (mask lower 32 bits, wrap to i32)
                func.instruction(&Instruction::LocalGet(temp_local));
                func.instruction(&Instruction::I32WrapI64);

                Ok(Some(print_fn))
            }
            // Regex operations: std.text.regex_*
            name if name.starts_with("std.text.regex_") => {
                let import_idx = match name {
                    "std.text.regex_test" => self.runtime.text_regex_test,
                    "std.text.regex_match" => self.runtime.text_regex_match,
                    "std.text.regex_replace" => self.runtime.text_regex_replace,
                    "std.text.regex_replace_all" => self.runtime.text_regex_replace_all,
                    "std.text.regex_split" => self.runtime.text_regex_split,
                    _ => return Err(CodegenError::UndefinedFunction { name: name.to_string() }),
                };
                let idx = import_idx.ok_or_else(|| CodegenError::UndefinedFunction {
                    name: format!("{} (text imports not registered?)", name),
                })?;

                // Compile all arguments and unpack their fat pointers
                for arg in &call.args {
                    self.compile_input(&arg.source, func)?;
                    // Each arg is an i64 fat pointer; unpack to (i32 ptr, i32 len)
                    let temp = self.allocate_local(&format!("__regex_arg_{}", arg.name));
                    func.instruction(&Instruction::LocalSet(temp));
                    func.instruction(&Instruction::LocalGet(temp));
                    func.instruction(&Instruction::I64Const(32));
                    func.instruction(&Instruction::I64ShrU);
                    func.instruction(&Instruction::I32WrapI64);
                    func.instruction(&Instruction::LocalGet(temp));
                    func.instruction(&Instruction::I32WrapI64);
                }

                Ok(Some(idx))
            }
            _ => Ok(None),
        }
    }

    /// Compile a return step
    fn compile_return_step(&mut self, ret: &ReturnStep, func: &mut Function) -> Result<(), CodegenError> {
        match &ret.value {
            ReturnValue::Var(name) => {
                let local = self.locals.get(name)
                    .ok_or_else(|| CodegenError::UndefinedFunction { name: name.clone() })?;
                func.instruction(&Instruction::LocalGet(*local));
            }
            ReturnValue::Lit(lit) => {
                self.compile_literal(lit, func)?;
            }
            ReturnValue::Struct(_) | ReturnValue::Variant(_) => {
                // Structs and variants not yet supported in WASM
                return Err(CodegenError::UnsupportedExpression);
            }
        }
        Ok(())
    }

    /// Compile an if step
    fn compile_if_step(&mut self, if_step: &IfStep, func: &mut Function) -> Result<(), CodegenError> {
        // Load condition variable
        let cond_local = self.locals.get(&if_step.condition)
            .ok_or_else(|| CodegenError::UndefinedFunction { name: if_step.condition.clone() })?;
        func.instruction(&Instruction::LocalGet(*cond_local));

        // Wrap i64 to i32 for the if condition (if instruction expects i32)
        func.instruction(&Instruction::I32WrapI64);

        // Emit if block
        func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));

        // Compile then steps
        for step in &if_step.then_steps {
            self.compile_step(step, func)?;
        }

        // Compile else steps if present
        if let Some(else_steps) = &if_step.else_steps {
            func.instruction(&Instruction::Else);
            for step in else_steps {
                self.compile_step(step, func)?;
            }
        }

        func.instruction(&Instruction::End);
        Ok(())
    }

    /// Compile a bind step
    fn compile_bind_step(&mut self, bind: &BindStep, func: &mut Function) -> Result<(), CodegenError> {
        match &bind.source {
            BindSource::Var(name) => {
                let local = self.locals.get(name)
                    .ok_or_else(|| CodegenError::UndefinedFunction { name: name.clone() })?;
                func.instruction(&Instruction::LocalGet(*local));
            }
            BindSource::Lit(lit) => {
                self.compile_literal(lit, func)?;
            }
            BindSource::Field { .. } => {
                // Field access not yet supported
                return Err(CodegenError::UnsupportedExpression);
            }
        }
        Ok(())
    }

    /// Compile an input source
    fn compile_input(&mut self, source: &InputSource, func: &mut Function) -> Result<(), CodegenError> {
        match source {
            InputSource::Var(name) => {
                let local = self.locals.get(name)
                    .ok_or_else(|| CodegenError::UndefinedFunction { name: name.clone() })?;
                func.instruction(&Instruction::LocalGet(*local));
            }
            InputSource::Lit(lit) => {
                self.compile_literal(lit, func)?;
            }
            InputSource::Field { .. } => {
                // Field access not yet supported
                return Err(CodegenError::UnsupportedExpression);
            }
        }
        Ok(())
    }

    /// Compile a literal value
    fn compile_literal(&mut self, lit: &Literal, func: &mut Function) -> Result<(), CodegenError> {
        match lit {
            Literal::Int(n) => {
                func.instruction(&Instruction::I64Const(*n));
            }
            Literal::Float(n) => {
                func.instruction(&Instruction::F64Const(*n));
            }
            Literal::Bool(b) => {
                func.instruction(&Instruction::I64Const(if *b { 1 } else { 0 }));
            }
            Literal::None => {
                // Represent None as sentinel value i64::MIN to distinguish from valid 0
                func.instruction(&Instruction::I64Const(i64::MIN));
            }
            Literal::String(s) => {
                // Store string in data segment and return fat pointer (offset << 32 | len)
                let offset = self.data_segment.add_string(s);
                let len = s.len() as i64;
                let packed = ((offset as i64) << 32) | len;
                func.instruction(&Instruction::I64Const(packed));
            }
        }
        Ok(())
    }

    /// Allocate a local variable
    fn allocate_local(&mut self, name: &str) -> u32 {
        if let Some(&idx) = self.locals.get(name) {
            return idx;
        }
        let idx = self.local_count;
        self.locals.insert(name.to_string(), idx);
        self.local_count += 1;
        idx
    }

    /// Convert an AST type to a WASM ValType
    fn type_to_valtype(&self, ty: &Type) -> Option<ValType> {
        match &ty.kind {
            TypeKind::Named(path) => match path.name() {
                "Int" => Some(ValType::I64),
                "Float" => Some(ValType::F64),
                "Bool" => Some(ValType::I64),
                "String" => Some(ValType::I64), // Fat pointer (offset << 32 | len)
                // All other named types (enums, structs) are represented as i64
                // Enums use tag values, structs use packed fields or pointers
                _ => Some(ValType::I64),
            },
            TypeKind::Optional(inner) => self.type_to_valtype(inner),
            TypeKind::List(_) => Some(ValType::I64), // Fat pointer (ptr << 32 | len)
            _ => None,
        }
    }

    /// Convert a return type to a WASM ValType
    fn return_type_to_valtype(&self, ret: &ReturnType) -> Option<ValType> {
        match ret {
            ReturnType::Single { ty, .. } => self.type_to_valtype(ty),
            ReturnType::Collection { of } => self.type_to_valtype(of),
            ReturnType::Union { types } => {
                // For unions, use the first type's representation
                types.first().and_then(|m| self.type_to_valtype(&m.ty))
            }
        }
    }
}

// ===== Helper Functions =====

/// Check if a snippet has effects
#[allow(dead_code)]
fn has_effects(snippet: &Snippet) -> bool {
    snippet
        .sections
        .iter()
        .any(|s| matches!(s, Section::Effects(_)))
}

/// Collect all effects from a set of snippets
fn collect_all_effects(snippets: &[&Snippet]) -> Vec<String> {
    let mut effects = Vec::new();
    for snippet in snippets {
        if let Some(effects_section) = find_effects_section(snippet) {
            for effect in &effects_section.effects {
                if !effects.contains(&effect.name) {
                    effects.push(effect.name.clone());
                }
            }
        }
    }
    effects
}

/// Find the effects section in a snippet
fn find_effects_section(snippet: &Snippet) -> Option<&EffectsSection> {
    for section in &snippet.sections {
        if let Section::Effects(effects) = section {
            return Some(effects);
        }
    }
    None
}

/// Find the function signature in a snippet
fn find_function_signature(snippet: &Snippet) -> Option<&FunctionSignature> {
    for section in &snippet.sections {
        if let Section::Signature(sig) = section {
            if let SignatureKind::Function(fn_sig) = &sig.kind {
                return Some(fn_sig);
            }
        }
    }
    None
}

/// Find the body section in a snippet
fn find_body_section(snippet: &Snippet) -> Option<&covenant_ast::BodySection> {
    for section in &snippet.sections {
        if let Section::Body(body) = section {
            return Some(body);
        }
    }
    None
}

/// Check if a snippet contains any string literals
fn snippet_has_string_literals(snippet: &Snippet) -> bool {
    if let Some(body) = find_body_section(snippet) {
        return steps_have_string_literals(&body.steps);
    }
    false
}

/// Check if steps contain any string literals
fn steps_have_string_literals(steps: &[Step]) -> bool {
    for step in steps {
        match &step.kind {
            StepKind::Return(ret) => {
                if let ReturnValue::Lit(Literal::String(_)) = &ret.value {
                    return true;
                }
            }
            StepKind::Bind(bind) => {
                if let BindSource::Lit(Literal::String(_)) = &bind.source {
                    return true;
                }
            }
            StepKind::Compute(compute) => {
                for input in &compute.inputs {
                    if let InputSource::Lit(Literal::String(_)) = &input.source {
                        return true;
                    }
                }
            }
            StepKind::If(if_step) => {
                if steps_have_string_literals(&if_step.then_steps) {
                    return true;
                }
                if let Some(else_steps) = &if_step.else_steps {
                    if steps_have_string_literals(else_steps) {
                        return true;
                    }
                }
            }
            StepKind::Match(match_step) => {
                for case in &match_step.cases {
                    if steps_have_string_literals(&case.steps) {
                        return true;
                    }
                }
            }
            StepKind::For(for_step) => {
                if steps_have_string_literals(&for_step.steps) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Check if a snippet uses any string operations (triggers text import registration)
fn snippet_has_string_ops(snippet: &Snippet) -> bool {
    if let Some(body) = find_body_section(snippet) {
        return steps_have_string_ops(&body.steps);
    }
    false
}

/// Check if steps contain any string operations
fn steps_have_string_ops(steps: &[Step]) -> bool {
    for step in steps {
        match &step.kind {
            StepKind::Compute(compute) => {
                if is_string_operation(&compute.op) {
                    return true;
                }
            }
            StepKind::Call(call) => {
                if call.fn_name.starts_with("std.text.") {
                    return true;
                }
            }
            StepKind::If(if_step) => {
                if steps_have_string_ops(&if_step.then_steps) {
                    return true;
                }
                if let Some(else_steps) = &if_step.else_steps {
                    if steps_have_string_ops(else_steps) {
                        return true;
                    }
                }
            }
            StepKind::Match(match_step) => {
                for case in &match_step.cases {
                    if steps_have_string_ops(&case.steps) {
                        return true;
                    }
                }
            }
            StepKind::For(for_step) => {
                if steps_have_string_ops(&for_step.steps) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Returns true if the operation is a string operation requiring host calls
fn is_string_operation(op: &Operation) -> bool {
    matches!(
        op,
        Operation::Concat
            | Operation::Contains
            | Operation::Slice
            | Operation::Upper
            | Operation::Lower
            | Operation::Trim
            | Operation::TrimStart
            | Operation::TrimEnd
            | Operation::Replace
            | Operation::Split
            | Operation::Join
            | Operation::Repeat
            | Operation::StrLen
            | Operation::ByteLen
            | Operation::IsEmpty
            | Operation::StartsWith
            | Operation::EndsWith
            | Operation::IndexOf
            | Operation::CharAt
            | Operation::StrReverse
            | Operation::PadStart
            | Operation::PadEnd
    )
}

/// Compute a deterministic tag value for a variant name
#[allow(dead_code)]
fn variant_tag(variant: &str) -> i64 {
    // Simple hash: sum of byte values
    // In a real implementation, this would use the type registry
    variant.bytes().map(|b| b as i64).sum()
}

/// Generate SQL from a Covenant query
fn generate_sql_from_covenant(
    query: &covenant_ast::CovenantQuery,
    _target: &str,
) -> String {
    use covenant_ast::{SnippetSelectClause, SnippetOrderDirection};

    let mut sql = String::new();

    // SELECT clause
    sql.push_str("SELECT ");
    match &query.select {
        SnippetSelectClause::All => sql.push('*'),
        SnippetSelectClause::Field(field) => sql.push_str(field),
    }

    // FROM clause
    sql.push_str(" FROM ");
    sql.push_str(&query.from);

    // WHERE clause
    if let Some(condition) = &query.where_clause {
        sql.push_str(" WHERE ");
        sql.push_str(&condition_to_sql(&condition.kind));
    }

    // ORDER BY clause
    if let Some(order) = &query.order {
        sql.push_str(" ORDER BY ");
        sql.push_str(&order.field);
        match order.direction {
            SnippetOrderDirection::Asc => sql.push_str(" ASC"),
            SnippetOrderDirection::Desc => sql.push_str(" DESC"),
        }
    }

    // LIMIT clause
    if let Some(limit) = query.limit {
        sql.push_str(" LIMIT ");
        sql.push_str(&limit.to_string());
    }

    sql
}

/// Convert a condition to SQL
fn condition_to_sql(condition: &covenant_ast::ConditionKind) -> String {
    use covenant_ast::ConditionKind;

    match condition {
        ConditionKind::Equals { field, value } => {
            format!("{} = {}", field, input_source_to_sql(value))
        }
        ConditionKind::NotEquals { field, value } => {
            format!("{} <> {}", field, input_source_to_sql(value))
        }
        ConditionKind::Contains { field, value } => {
            format!("{} LIKE '%' || {} || '%'", field, input_source_to_sql(value))
        }
        ConditionKind::And(left, right) => {
            format!(
                "({}) AND ({})",
                condition_to_sql(&left.kind),
                condition_to_sql(&right.kind)
            )
        }
        ConditionKind::Or(left, right) => {
            format!(
                "({}) OR ({})",
                condition_to_sql(&left.kind),
                condition_to_sql(&right.kind)
            )
        }
        ConditionKind::RelTo { target, rel_type } | ConditionKind::RelFrom { source: target, rel_type } => {
            // Relationship conditions need join logic - placeholder
            format!("/* relation {} to {} */", rel_type, target)
        }
    }
}

/// Convert an input source to SQL value
fn input_source_to_sql(source: &InputSource) -> String {
    match source {
        InputSource::Var(name) => format!(":{}", name), // Parameter placeholder
        InputSource::Lit(lit) => literal_to_sql(lit),
        InputSource::Field { of, field } => format!("{}.{}", of, field),
    }
}

/// Convert a literal to SQL
fn literal_to_sql(lit: &Literal) -> String {
    match lit {
        Literal::Int(n) => n.to_string(),
        Literal::Float(n) => n.to_string(),
        Literal::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        Literal::String(s) => format!("'{}'", s.replace('\'', "''")),
        Literal::None => "NULL".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiler_creation() {
        let symbols = SymbolTable::new();
        let compiler = SnippetWasmCompiler::new(&symbols);
        assert!(compiler.function_indices.is_empty());
    }

    #[test]
    fn test_data_segment_builder() {
        let mut builder = DataSegmentBuilder::new();

        // Add a string
        let offset1 = builder.add_string("hello");
        assert_eq!(offset1, 0);

        // Adding same string returns cached offset
        let offset2 = builder.add_string("hello");
        assert_eq!(offset1, offset2);

        // Adding different string gets new offset
        let offset3 = builder.add_string("world");
        assert!(offset3 > offset1);

        let data = builder.finish();
        assert!(!data.is_empty());
    }

    #[test]
    fn test_wasm_type_sizes() {
        assert_eq!(WasmType::I32.size(), 4);
        assert_eq!(WasmType::I64.size(), 8);
        assert_eq!(WasmType::F32.size(), 4);
        assert_eq!(WasmType::F64.size(), 8);
        assert_eq!(WasmType::Ptr.size(), 4);
    }

    #[test]
    fn test_variant_tag() {
        let tag1 = variant_tag("Some");
        let tag2 = variant_tag("None");
        assert_ne!(tag1, tag2);

        // Same variant should produce same tag
        assert_eq!(variant_tag("Some"), variant_tag("Some"));
    }

    #[test]
    fn test_literal_to_sql() {
        assert_eq!(literal_to_sql(&Literal::Int(42)), "42");
        assert_eq!(literal_to_sql(&Literal::Float(3.14)), "3.14");
        assert_eq!(literal_to_sql(&Literal::Bool(true)), "TRUE");
        assert_eq!(literal_to_sql(&Literal::Bool(false)), "FALSE");
        assert_eq!(literal_to_sql(&Literal::String("test".to_string())), "'test'");
        assert_eq!(literal_to_sql(&Literal::None), "NULL");
    }

    #[test]
    fn test_literal_to_sql_escapes_quotes() {
        assert_eq!(
            literal_to_sql(&Literal::String("it's".to_string())),
            "'it''s'"
        );
    }

    #[test]
    fn test_input_source_to_sql() {
        assert_eq!(
            input_source_to_sql(&InputSource::Var("user_id".to_string())),
            ":user_id"
        );
        assert_eq!(
            input_source_to_sql(&InputSource::Lit(Literal::Int(100))),
            "100"
        );
        assert_eq!(
            input_source_to_sql(&InputSource::Field {
                of: "user".to_string(),
                field: "name".to_string()
            }),
            "user.name"
        );
    }

    #[test]
    fn test_import_tracker() {
        let mut tracker = ImportTracker::new();

        let idx1 = tracker.add_import(
            "covenant_db",
            "execute_query",
            vec![ValType::I32],
            vec![ValType::I32],
        );
        assert_eq!(idx1, 0);

        // Same import returns same index
        let idx2 = tracker.add_import(
            "covenant_db",
            "execute_query",
            vec![ValType::I32],
            vec![ValType::I32],
        );
        assert_eq!(idx1, idx2);

        // Different import gets new index
        let idx3 = tracker.add_import(
            "covenant_http",
            "fetch",
            vec![ValType::I32],
            vec![ValType::I32],
        );
        assert_eq!(idx3, 1);

        assert_eq!(tracker.len(), 2);
    }

    #[test]
    fn test_generate_sql_simple_select() {
        use covenant_ast::{CovenantQuery, SnippetSelectClause, Span};

        let query = CovenantQuery {
            select: SnippetSelectClause::All,
            from: "users".to_string(),
            where_clause: None,
            order: None,
            limit: None,
            span: Span::default(),
        };

        let sql = generate_sql_from_covenant(&query, "test_db");
        assert_eq!(sql, "SELECT * FROM users");
    }

    #[test]
    fn test_generate_sql_with_limit() {
        use covenant_ast::{CovenantQuery, SnippetSelectClause, Span};

        let query = CovenantQuery {
            select: SnippetSelectClause::Field("name".to_string()),
            from: "users".to_string(),
            where_clause: None,
            order: None,
            limit: Some(10),
            span: Span::default(),
        };

        let sql = generate_sql_from_covenant(&query, "test_db");
        assert_eq!(sql, "SELECT name FROM users LIMIT 10");
    }

    #[test]
    fn test_generate_sql_with_order() {
        use covenant_ast::{
            CovenantQuery, OrderClause, SnippetOrderDirection, SnippetSelectClause, Span,
        };

        let query = CovenantQuery {
            select: SnippetSelectClause::All,
            from: "products".to_string(),
            where_clause: None,
            order: Some(OrderClause {
                field: "price".to_string(),
                direction: SnippetOrderDirection::Desc,
                span: Span::default(),
            }),
            limit: None,
            span: Span::default(),
        };

        let sql = generate_sql_from_covenant(&query, "test_db");
        assert_eq!(sql, "SELECT * FROM products ORDER BY price DESC");
    }

    #[test]
    fn test_generate_sql_with_where() {
        use covenant_ast::{
            Condition, ConditionKind, CovenantQuery, SnippetSelectClause, Span,
        };

        let query = CovenantQuery {
            select: SnippetSelectClause::All,
            from: "users".to_string(),
            where_clause: Some(Condition {
                kind: ConditionKind::Equals {
                    field: "status".to_string(),
                    value: InputSource::Lit(Literal::String("active".to_string())),
                },
                span: Span::default(),
            }),
            order: None,
            limit: None,
            span: Span::default(),
        };

        let sql = generate_sql_from_covenant(&query, "test_db");
        assert_eq!(sql, "SELECT * FROM users WHERE status = 'active'");
    }
}
