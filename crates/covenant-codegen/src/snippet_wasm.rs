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
    GlobalSection, GlobalType, ImportSection, Instruction, MemArg, MemorySection, MemoryType,
    Module, TypeSection, ValType,
};
use covenant_ast::{
    BindSource, BindStep, CallStep, ComputeStep, EffectsSection, ForStep, FunctionSignature,
    InputSource, IfStep, Literal, MatchPattern, MatchStep, Operation, QueryContent,
    QueryStep, ReturnStep, ReturnType, ReturnValue, Section, SignatureKind, Snippet, SnippetKind,
    Step, StepKind, StructConstruction, Type, TypeKind,
};
use covenant_checker::SymbolTable;
use crate::CodegenError;
use crate::data_graph::DataGraph;
use crate::gai_codegen::{self, GraphLayout, GaiFunctionIndices, GAI_FUNCTION_COUNT};

// ===== Memory Layout Types =====

/// Layout information for struct fields
#[derive(Debug, Clone)]
pub struct FieldLayout {
    /// Offset from struct base pointer
    pub offset: u32,
    /// Size in bytes (for future per-type sizing)
    #[allow(dead_code)]
    pub size: u32,
    /// WASM type for this field (for future per-type sizing)
    #[allow(dead_code)]
    pub wasm_type: WasmType,
}

/// Layout information for structs
#[derive(Debug, Clone)]
pub struct StructLayout {
    /// Total size in bytes (for future per-type sizing)
    #[allow(dead_code)]
    pub size: u32,
    /// Alignment requirement (for future per-type sizing)
    #[allow(dead_code)]
    pub alignment: u32,
    /// Field layouts by name
    pub fields: HashMap<String, FieldLayout>,
}

/// WASM type representation (currently only I64 is used; others reserved for future per-type layout)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum WasmType {
    I32,
    I64,
    F32,
    F64,
    /// Pointer to memory (represented as i32)
    Ptr,
}

#[allow(dead_code)]
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

    /// Prepend raw bytes (e.g., graph data segment) before any strings.
    /// All existing string offsets are invalidated (call before adding strings).
    pub fn prepend_raw(&mut self, raw: &[u8]) {
        let mut new_data = raw.to_vec();
        new_data.append(&mut self.data);
        self.data = new_data;
        // Adjust all cached string offsets
        let offset_delta = raw.len() as u32;
        for offset in self.string_offsets.values_mut() {
            *offset += offset_delta;
        }
    }

    /// Current data length (useful for computing offsets)
    pub fn len(&self) -> usize {
        self.data.len()
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
    /// Maps local variable names to their struct type name (for field access)
    local_types: HashMap<String, String>,
    /// Runtime function indices (set after imports are processed)
    runtime: RuntimeFunctions,
    /// Generic extern-abstract imports: snippet ID → ExternImport
    extern_imports: HashMap<String, ExternImport>,
    /// GAI function indices (set when data snippets are present)
    gai_indices: Option<GaiFunctionIndices>,
    /// Graph layout (set when data snippets are present)
    graph_layout: Option<GraphLayout>,
    /// Set of function names/IDs that have no WASM return value (Unit return type)
    void_functions: std::collections::HashSet<String>,
}

/// Describes a registered extern-abstract import
#[derive(Debug, Clone)]
struct ExternImport {
    /// WASM function index for this import
    func_index: u32,
    /// Parameter types from the signature (used to determine unpacking)
    param_types: Vec<ExternParamKind>,
}

/// How an extern parameter maps to WASM calling convention
#[derive(Debug, Clone, Copy, PartialEq)]
enum ExternParamKind {
    /// String: i64 fat pointer on stack, unpacked to (i32 ptr, i32 len) for host call
    String,
    /// Int: i64 on stack, wrapped to i32 for host call
    Int,
    /// Bool: i64 on stack, wrapped to i32 for host call
    Bool,
    /// List/Map/Any: i64 fat pointer (ptr+len of serialized data), same as String convention
    FatPointer,
}

/// Runtime function indices for core operations
#[derive(Debug, Default, Clone)]
struct RuntimeFunctions {
    /// Memory allocation: mem.alloc(size) -> ptr
    mem_alloc: Option<u32>,
    /// Database query execution: db.execute_query(sql_ptr, sql_len, param_count) -> result_ptr
    db_execute_query: Option<u32>,
    /// HTTP fetch: http.fetch(url_ptr, url_len) -> response_ptr
    http_fetch: Option<u32>,
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
            local_types: HashMap::new(),
            runtime: RuntimeFunctions::default(),
            extern_imports: HashMap::new(),
            gai_indices: None,
            graph_layout: None,
            void_functions: std::collections::HashSet::new(),
        }
    }

    /// Compile snippets to WASM
    pub fn compile_snippets(&mut self, snippets: &[Snippet]) -> Result<Vec<u8>, CodegenError> {
        let mut module = Module::new();

        // Register struct layouts from struct snippets
        for snippet in snippets {
            if snippet.kind == SnippetKind::Struct {
                self.register_struct_layout(snippet);
            }
        }

        // Collect all function snippets (both pure and effectful)
        let functions: Vec<&Snippet> = snippets
            .iter()
            .filter(|s| s.kind == SnippetKind::Function)
            .collect();

        // Check for data snippets - if present, build the graph and embed it
        let has_data_snippets = snippets.iter().any(|s| s.kind == SnippetKind::Data);

        if has_data_snippets {
            let graph = DataGraph::from_snippets(snippets);
            if graph.node_count() > 0 {
                // Generate graph data segment at offset 0
                let (seg_data, layout) = gai_codegen::generate_graph_segment(&graph, 0);
                self.graph_layout = Some(layout);
                // Pre-fill the data segment with graph data so that subsequent
                // add_string() calls get correct offsets (after graph data)
                self.data_segment.prepend_raw(&seg_data);
            }
        }

        if functions.is_empty() && self.graph_layout.is_none() {
            // Return minimal valid WASM module
            return Ok(module.finish());
        }

        // First pass: collect all effects and register imports
        let all_effects = collect_all_effects(&functions);
        self.register_effect_imports(&all_effects);

        // Register all extern-abstract imports (stdlib + user-defined)
        self.register_extern_abstracts();
        self.register_user_extern_abstracts(snippets);

        // Pre-scan for string literals to determine if we need memory
        let has_strings = functions.iter().any(|s| snippet_has_string_literals(s));

        // Determine how many GAI functions we need
        let gai_count = if self.graph_layout.is_some() { GAI_FUNCTION_COUNT } else { 0 };

        // Build type section (imports first, then user functions, then GAI functions)
        let mut types = TypeSection::new();

        // Add import types
        for (_, _, params, results) in &self.imports.imports {
            types.function(params.clone(), results.clone());
        }

        // Add function types for user functions
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

        // Add GAI function types
        if gai_count > 0 {
            for (params, results) in gai_codegen::gai_function_types() {
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
                // Track Unit-returning functions (no WASM return value)
                let has_wasm_return = sig.returns.as_ref()
                    .and_then(|r| self.return_type_to_valtype(r))
                    .is_some();
                if !has_wasm_return {
                    self.void_functions.insert(sig.name.clone());
                    self.void_functions.insert(snippet.id.clone());
                }
            }
        }

        // Compute GAI function indices (after user functions)
        let gai_base_idx = import_count + functions.len() as u32;
        if gai_count > 0 {
            self.gai_indices = Some(GaiFunctionIndices {
                node_count: gai_base_idx,
                get_node_id: gai_base_idx + 1,
                get_node_content: gai_base_idx + 2,
                get_outgoing_count: gai_base_idx + 3,
                get_outgoing_rel: gai_base_idx + 4,
                get_incoming_count: gai_base_idx + 5,
                get_incoming_rel: gai_base_idx + 6,
                find_by_id: gai_base_idx + 7,
                content_contains: gai_base_idx + 8,
                get_rel_type_name: gai_base_idx + 9,
            });
        }

        // Function section (user functions + GAI functions)
        let mut func_section = FunctionSection::new();
        // User function type indices start after imports
        for i in 0..functions.len() {
            func_section.function(import_count + i as u32);
        }
        // GAI function type indices start after user function types
        let gai_type_base = import_count + functions.len() as u32;
        for i in 0..gai_count {
            func_section.function(gai_type_base + i);
        }
        module.section(&func_section);

        // Memory section - always export memory when compiling functions or data
        let needs_memory = !functions.is_empty()
            || self.graph_layout.is_some()
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
            // Heap pointer starts after all data (graph + strings)
            let heap_start = self.data_segment.len() as i32;
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
        // Export GAI functions with cov_ prefix for external access
        if let Some(ref gai) = self.gai_indices {
            exports.export("cov_node_count", ExportKind::Func, gai.node_count);
            exports.export("cov_get_node_id", ExportKind::Func, gai.get_node_id);
            exports.export("cov_get_node_content", ExportKind::Func, gai.get_node_content);
            exports.export("cov_get_outgoing_count", ExportKind::Func, gai.get_outgoing_count);
            exports.export("cov_get_outgoing_rel", ExportKind::Func, gai.get_outgoing_rel);
            exports.export("cov_get_incoming_count", ExportKind::Func, gai.get_incoming_count);
            exports.export("cov_get_incoming_rel", ExportKind::Func, gai.get_incoming_rel);
            exports.export("cov_find_by_id", ExportKind::Func, gai.find_by_id);
            exports.export("cov_content_contains", ExportKind::Func, gai.content_contains);
            exports.export("cov_get_rel_type_name", ExportKind::Func, gai.get_rel_type_name);
        }
        // Export memory if present
        if needs_memory {
            exports.export("memory", ExportKind::Memory, 0);
        }
        module.section(&exports);

        // Code section (user functions + GAI functions)
        let mut codes = CodeSection::new();
        for snippet in &functions {
            let wasm_func = self.compile_function_snippet(snippet)?;
            codes.function(&wasm_func);
        }
        // Add GAI function bodies
        if let Some(ref layout) = self.graph_layout {
            let gai_funcs = gai_codegen::generate_gai_functions(layout);
            for gai_func in gai_funcs {
                codes.function(&gai_func);
            }
        }
        module.section(&codes);

        // Data section (graph data + string constants, already combined in data_segment)
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
                        "db",
                        "execute_query",
                        vec![ValType::I32, ValType::I32, ValType::I32], // sql_ptr, sql_len, param_count
                        vec![ValType::I32],                             // result_ptr
                    ));
                }
                "network" => {
                    self.runtime.http_fetch = Some(self.imports.add_import(
                        "http",
                        "fetch",
                        vec![ValType::I32, ValType::I32], // url_ptr, url_len
                        vec![ValType::I32],              // response_ptr
                    ));
                }
                _ => {
                    // Effects like "filesystem", "console", etc. are now handled
                    // via extern-abstract registration — no special-casing needed.
                }
            }
        }
    }

    /// Ensure the memory allocator import is registered
    fn ensure_mem_alloc(&mut self) {
        if self.runtime.mem_alloc.is_none() {
            self.runtime.mem_alloc = Some(self.imports.add_import(
                "mem",
                "alloc",
                vec![ValType::I32], // size
                vec![ValType::I32], // ptr
            ));
        }
    }

    /// Register all extern-abstract snippets from stdlib sources.
    /// Parses each source, finds ExternAbstract snippets, and registers them as WASM imports.
    fn register_extern_abstracts(&mut self) {
        const STDLIB_SOURCES: &[&str] = &[
            include_str!("../../../runtime/std/console/console.cov"),
            include_str!("../../../runtime/std/filesystem/fs.cov"),
            include_str!("../../../runtime/std/path/path.cov"),
            include_str!("../../../runtime/std/text/text.cov"),
            include_str!("../../../runtime/std/text/regex.cov"),
            include_str!("../../../runtime/std/list/list.cov"),
        ];

        // Ensure mem.alloc is available for any extern that returns String/List
        self.ensure_mem_alloc();

        let mut snippets_to_register = Vec::new();
        for source in STDLIB_SOURCES {
            if let Ok(covenant_ast::Program::Snippets { snippets, .. }) = covenant_parser::parse(source) {
                for snippet in snippets {
                    if snippet.kind == SnippetKind::ExternAbstract {
                        snippets_to_register.push(snippet);
                    }
                }
            }
        }
        for snippet in snippets_to_register {
            self.register_single_extern(&snippet);
        }
    }

    /// Register a single extern-abstract snippet as a WASM import.
    /// Splits snippet ID on last dot to derive (module, function) for the import.
    fn register_single_extern(&mut self, snippet: &Snippet) {
        let id = &snippet.id;

        // Split ID on last dot: "text.concat" → ("text", "concat")
        // "std.text.regex_test" → ("std.text", "regex_test")
        // "to_uppercase" (no dot) → ("extern", "to_uppercase")
        let (module, func_name) = match id.rfind('.') {
            Some(pos) => (&id[..pos], &id[pos + 1..]),
            None => ("extern", id.as_str()),
        };

        // Get function signature to determine param types
        let sig = match find_function_signature(snippet) {
            Some(s) => s,
            None => return,
        };

        // Map param types to WASM types and ExternParamKind
        let mut wasm_params = Vec::new();
        let mut param_kinds = Vec::new();

        for param in &sig.params {
            let kind = type_to_extern_param_kind(&param.ty);
            match kind {
                ExternParamKind::String | ExternParamKind::FatPointer => {
                    // Fat pointer: unpacked to (i32 ptr, i32 len)
                    wasm_params.push(ValType::I32);
                    wasm_params.push(ValType::I32);
                }
                ExternParamKind::Int | ExternParamKind::Bool => {
                    wasm_params.push(ValType::I32);
                }
            }
            param_kinds.push(kind);
        }

        // Return type: Unit maps to no return value; everything else is i64
        let wasm_results = if self.extern_returns_unit(&sig) {
            vec![]
        } else {
            vec![ValType::I64]
        };

        let func_index = self.imports.add_import(module, func_name, wasm_params, wasm_results);
        let ext_import = ExternImport {
            func_index,
            param_types: param_kinds,
        };
        self.extern_imports.insert(id.clone(), ext_import.clone());
        // Also register by function name (from signature) for short-name calls
        if sig.name != *id {
            self.extern_imports.entry(sig.name.clone()).or_insert(ext_import);
        }
    }

    /// Also register user-defined extern-abstract and extern snippets from the program
    fn register_user_extern_abstracts(&mut self, snippets: &[Snippet]) {
        for snippet in snippets {
            if snippet.kind == SnippetKind::ExternAbstract || snippet.kind == SnippetKind::Extern {
                self.register_single_extern(snippet);
            }
        }
    }

    /// Register a struct snippet's layout for field access
    fn register_struct_layout(&mut self, snippet: &Snippet) {
        // Find struct signature
        for section in &snippet.sections {
            if let Section::Signature(sig) = section {
                if let SignatureKind::Struct(struct_sig) = &sig.kind {
                    let mut fields = HashMap::new();
                    for (i, field) in struct_sig.fields.iter().enumerate() {
                        fields.insert(field.name.clone(), FieldLayout {
                            offset: (i as u32) * 8,
                            size: 8,
                            wasm_type: WasmType::I64,
                        });
                    }
                    self.struct_layouts.insert(struct_sig.name.clone(), StructLayout {
                        size: (struct_sig.fields.len() as u32) * 8,
                        alignment: 8,
                        fields,
                    });
                }
            }
        }
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
        self.local_types.clear();

        // Add parameters as locals and track their struct types
        for param in &sig.params {
            self.locals.insert(param.name.clone(), self.local_count);
            self.local_count += 1;
            // If the parameter type is a known struct, register it in local_types
            if let TypeKind::Named(path) = &param.ty.kind {
                let type_name = path.name().to_string();
                if self.struct_layouts.contains_key(&type_name) {
                    self.local_types.insert(param.name.clone(), type_name);
                }
            }
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

        // If function returns a value (non-Unit), we need something on the stack for the
        // implicit return. Push a dummy value (0) in case all paths returned early via
        // explicit returns. Unit-returning functions have no WASM return value.
        let has_wasm_return = sig.returns.as_ref()
            .and_then(|r| self.return_type_to_valtype(r))
            .is_some();
        if has_wasm_return {
            wasm_func.instruction(&Instruction::I64Const(0));
        }

        // Add end instruction
        wasm_func.instruction(&Instruction::End);

        Ok(wasm_func)
    }

    /// Count the number of step bindings that need locals
    fn count_step_bindings(&self, steps: &[Step]) -> u32 {
        let mut count = 0;
        for step in steps {
            if step.output_binding != "_" {
                count += 1;
            }
            // Count nested steps and special cases
            match &step.kind {
                StepKind::Compute(_) => {}
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
                    // Count: index local, length local, item local, base local
                    count += 4;
                    count += self.count_step_bindings(&for_step.steps);
                }
                StepKind::Call(call) => {
                    // Extern calls need a temp local per argument for fat pointer unpacking
                    count += call.args.len() as u32;
                }
                StepKind::Construct(_) => {
                    // Struct construction needs a temp local for the pointer
                    count += 1;
                }
                StepKind::Return(ret) => {
                    // Return with struct construction needs a temp local for the pointer
                    if matches!(&ret.value, ReturnValue::Struct(_)) {
                        count += 1;
                    }
                }
                _ => {}
            }
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
                } else {
                    func.instruction(&Instruction::Drop);
                }
            }
            StepKind::Call(call) => {
                self.compile_call_step(call, func)?;
                let has_return = self.call_has_return_value(&call.fn_name);
                if step.output_binding != "_" && has_return {
                    let local = self.allocate_local(&step.output_binding);
                    func.instruction(&Instruction::LocalSet(local));
                } else if step.output_binding == "_" && has_return {
                    // Function returns a value but result is discarded - pop it
                    func.instruction(&Instruction::Drop);
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
                } else {
                    func.instruction(&Instruction::Drop);
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
                } else {
                    func.instruction(&Instruction::Drop);
                }
            }
            StepKind::Construct(construct) => {
                // Register struct layout and track variable type for field access
                let type_name = match &construct.ty.kind {
                    TypeKind::Named(path) => path.name().to_string(),
                    _ => format!("{:?}", construct.ty.kind),
                };
                if !self.struct_layouts.contains_key(&type_name) {
                    let layout = Self::compute_struct_layout(construct);
                    self.struct_layouts.insert(type_name.clone(), layout);
                }
                if step.output_binding != "_" {
                    self.local_types.insert(step.output_binding.clone(), type_name);
                }
                self.compile_construct_step(construct, func)?;
                // Store result if not discarded
                if step.output_binding != "_" {
                    let local = self.allocate_local(&step.output_binding);
                    func.instruction(&Instruction::LocalSet(local));
                } else {
                    func.instruction(&Instruction::Drop);
                }
            }
            StepKind::Insert(_) | StepKind::Update(_) | StepKind::Delete(_) |
            StepKind::Transaction(_) | StepKind::Traverse(_) => {
                // These require database/meta effects and runtime support.
                // Generate a placeholder value (i64 0) for the binding.
                // At runtime, the host would intercept these via effect imports.
                if step.output_binding != "_" {
                    func.instruction(&Instruction::I64Const(0));
                    let local = self.allocate_local(&step.output_binding);
                    func.instruction(&Instruction::LocalSet(local));
                }
                // If discarded (as="_"), don't push anything onto the stack
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
    ///
    /// Collections are stored as fat pointers: (ptr << 32) | len
    /// The memory layout at ptr is: [count:i32][item0:i64][item1:i64]...
    fn compile_for_step(&mut self, for_step: &ForStep, func: &mut Function) -> Result<(), CodegenError> {
        // Get the collection (copy the index to avoid borrow issues)
        let collection_local = *self.locals.get(&for_step.collection)
            .ok_or_else(|| CodegenError::UndefinedFunction { name: for_step.collection.clone() })?;

        // Allocate locals for loop state
        let index_local = self.allocate_local(&format!("__for_idx_{}", for_step.var));
        let len_local = self.allocate_local(&format!("__for_len_{}", for_step.var));
        let item_local = self.allocate_local(&for_step.var);
        let base_local = self.allocate_local(&format!("__for_base_{}", for_step.var));

        // Extract array base pointer from fat pointer (high 32 bits)
        // base = collection >> 32
        func.instruction(&Instruction::LocalGet(collection_local));
        func.instruction(&Instruction::I64Const(32));
        func.instruction(&Instruction::I64ShrU);
        func.instruction(&Instruction::LocalSet(base_local));

        // Start outer block - we'll skip the loop entirely if base is null
        func.instruction(&Instruction::Block(BlockType::Empty)); // outer block for break/skip

        // Check if base is null (0) - if so, skip the loop entirely
        func.instruction(&Instruction::LocalGet(base_local));
        func.instruction(&Instruction::I64Eqz);
        func.instruction(&Instruction::BrIf(0)); // Skip loop if base is null

        // Read array count from memory at base (first 4 bytes)
        // len = i32.load(base) extended to i64
        func.instruction(&Instruction::LocalGet(base_local));
        func.instruction(&Instruction::I32WrapI64);
        func.instruction(&Instruction::I32Load(MemArg {
            offset: 0,
            align: 2, // 4-byte alignment
            memory_index: 0,
        }));
        func.instruction(&Instruction::I64ExtendI32U);
        func.instruction(&Instruction::LocalSet(len_local));

        // Initialize index to 0
        func.instruction(&Instruction::I64Const(0));
        func.instruction(&Instruction::LocalSet(index_local));

        // Start loop block
        func.instruction(&Instruction::Block(BlockType::Empty)); // inner block for break
        func.instruction(&Instruction::Loop(BlockType::Empty)); // loop block

        // Check if index >= length (exit condition)
        func.instruction(&Instruction::LocalGet(index_local));
        func.instruction(&Instruction::LocalGet(len_local));
        func.instruction(&Instruction::I64GeS); // index >= length means exit
        func.instruction(&Instruction::BrIf(1)); // Break out if done (to inner block)

        // Get current item from array: item = i64.load(base + 4 + index * 8)
        // Calculate address: base + 4 + index * 8
        func.instruction(&Instruction::LocalGet(base_local));
        func.instruction(&Instruction::I64Const(4)); // skip count field
        func.instruction(&Instruction::I64Add);
        func.instruction(&Instruction::LocalGet(index_local));
        func.instruction(&Instruction::I64Const(8)); // each item is 8 bytes
        func.instruction(&Instruction::I64Mul);
        func.instruction(&Instruction::I64Add);
        // Load item (i64 fat pointer for strings/values)
        func.instruction(&Instruction::I32WrapI64); // address as i32
        func.instruction(&Instruction::I64Load(MemArg {
            offset: 0,
            align: 3, // 8-byte alignment
            memory_index: 0,
        }));
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

        // End loop and inner block
        func.instruction(&Instruction::End); // end loop
        func.instruction(&Instruction::End); // end inner block

        // End outer block (null check skip target)
        func.instruction(&Instruction::End); // end outer block

        Ok(())
    }

    /// Compute the memory layout for a struct based on its field list.
    /// All fields are i64 (8 bytes each), stored sequentially.
    fn compute_struct_layout(construct: &StructConstruction) -> StructLayout {
        let mut fields = HashMap::new();
        for (i, field) in construct.fields.iter().enumerate() {
            fields.insert(field.name.clone(), FieldLayout {
                offset: (i as u32) * 8,
                size: 8,
                wasm_type: WasmType::I64,
            });
        }
        StructLayout {
            size: (construct.fields.len() as u32) * 8,
            alignment: 8,
            fields,
        }
    }

    /// Compile a struct construction step using linear memory allocation.
    ///
    /// Allocates space on the heap (bump allocator via global 0), stores each
    /// field at its computed offset, and leaves the struct pointer (as i64) on the stack.
    ///
    /// All locals are i64, so the pointer is stored as i64 (zero-extended from i32).
    /// For memory operations, we wrap i64 back to i32.
    fn compile_construct_step(
        &mut self,
        construct: &StructConstruction,
        func: &mut Function,
    ) -> Result<(), CodegenError> {
        let struct_size = (construct.fields.len() as u32) * 8;
        let ptr_local = self.allocate_local("__struct_ptr");

        // Bump-allocate: ptr = heap_ptr; heap_ptr += size
        // GlobalGet(0) returns i32, extend to i64 for our local
        func.instruction(&Instruction::GlobalGet(0));
        func.instruction(&Instruction::I64ExtendI32U);
        func.instruction(&Instruction::LocalTee(ptr_local));
        // Compute new heap_ptr: wrap back to i32, add size, set global
        func.instruction(&Instruction::I32WrapI64);
        func.instruction(&Instruction::I32Const(struct_size as i32));
        func.instruction(&Instruction::I32Add);
        func.instruction(&Instruction::GlobalSet(0));

        // Store each field at its offset
        for (i, field) in construct.fields.iter().enumerate() {
            let offset = (i as u32) * 8;
            // Get ptr as i32 for memory address
            func.instruction(&Instruction::LocalGet(ptr_local));
            func.instruction(&Instruction::I32WrapI64);
            self.compile_input(&field.value, func)?;
            func.instruction(&Instruction::I64Store(MemArg {
                offset: offset as u64,
                align: 3, // 2^3 = 8 byte alignment
                memory_index: 0,
            }));
        }

        // Leave struct pointer as i64 on stack (already stored as i64 in local)
        func.instruction(&Instruction::LocalGet(ptr_local));
        Ok(())
    }


    /// Compile a query step
    ///
    /// Queries are compiled differently based on dialect:
    /// - Covenant queries are compiled to runtime calls
    /// - SQL dialect queries have their SQL stored in data segment
    fn compile_query_step(&mut self, query: &QueryStep, func: &mut Function) -> Result<(), CodegenError> {
        // Route based on query target
        if query.target == "project" {
            // Project queries use embedded GAI functions, not external database
            return self.compile_project_query(query, func);
        }

        // For non-project targets (database queries), use existing SQL compilation
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

    /// Compile a project query (target="project") using GAI functions
    fn compile_project_query(&mut self, _query: &QueryStep, func: &mut Function) -> Result<(), CodegenError> {
        // TODO: Implement full project query compilation using GAI functions
        // For now, return an empty result (null pointer = 0)
        //
        // Full implementation will:
        // 1. Call cov_node_count() to get total nodes
        // 2. Iterate through nodes using cov_get_node_id(idx)
        // 3. Filter nodes based on where clause using:
        //    - cov_content_contains() for content searches
        //    - cov_find_by_id() for ID lookups
        //    - cov_get_outgoing_rel() / cov_get_incoming_rel() for relation queries
        // 4. Collect matching nodes into a result array
        // 5. Apply ordering and limit
        // 6. Return pointer to result array

        if let Some(ref _gai) = self.gai_indices {
            // GAI functions are available - future implementation will use them
            // For now, return empty result
            func.instruction(&Instruction::I32Const(0));
            func.instruction(&Instruction::I64ExtendI32U);
        } else {
            // No GAI functions available (no data snippets compiled)
            // Return empty result
            func.instruction(&Instruction::I32Const(0));
            func.instruction(&Instruction::I64ExtendI32U);
        }

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

            // Bitwise operations
            Operation::BitAnd => { func.instruction(&Instruction::I64And); }
            Operation::BitOr => { func.instruction(&Instruction::I64Or); }
            Operation::BitXor => { func.instruction(&Instruction::I64Xor); }
            Operation::BitNot => {
                // NOT = XOR with -1 (all bits set)
                func.instruction(&Instruction::I64Const(-1));
                func.instruction(&Instruction::I64Xor);
            }
            Operation::BitShl => { func.instruction(&Instruction::I64Shl); }
            Operation::BitShr => { func.instruction(&Instruction::I64ShrS); }
            Operation::BitUshr => { func.instruction(&Instruction::I64ShrU); }

            // Numeric operations
            Operation::Abs => {
                // abs(x) = if x < 0 then -x else x
                let tmp = self.allocate_local("__abs_tmp");
                func.instruction(&Instruction::LocalTee(tmp));
                func.instruction(&Instruction::I64Const(0));
                func.instruction(&Instruction::I64LtS);
                func.instruction(&Instruction::If(BlockType::Result(ValType::I64)));
                func.instruction(&Instruction::I64Const(0));
                func.instruction(&Instruction::LocalGet(tmp));
                func.instruction(&Instruction::I64Sub);
                func.instruction(&Instruction::Else);
                func.instruction(&Instruction::LocalGet(tmp));
                func.instruction(&Instruction::End);
            }
            Operation::Min => {
                // min(a, b) = if a < b then a else b
                let b = self.allocate_local("__min_b");
                let a = self.allocate_local("__min_a");
                func.instruction(&Instruction::LocalSet(b));
                func.instruction(&Instruction::LocalSet(a));
                func.instruction(&Instruction::LocalGet(a));
                func.instruction(&Instruction::LocalGet(b));
                func.instruction(&Instruction::I64LtS);
                func.instruction(&Instruction::If(BlockType::Result(ValType::I64)));
                func.instruction(&Instruction::LocalGet(a));
                func.instruction(&Instruction::Else);
                func.instruction(&Instruction::LocalGet(b));
                func.instruction(&Instruction::End);
            }
            Operation::Max => {
                // max(a, b) = if a > b then a else b
                let b = self.allocate_local("__max_b");
                let a = self.allocate_local("__max_a");
                func.instruction(&Instruction::LocalSet(b));
                func.instruction(&Instruction::LocalSet(a));
                func.instruction(&Instruction::LocalGet(a));
                func.instruction(&Instruction::LocalGet(b));
                func.instruction(&Instruction::I64GtS);
                func.instruction(&Instruction::If(BlockType::Result(ValType::I64)));
                func.instruction(&Instruction::LocalGet(a));
                func.instruction(&Instruction::Else);
                func.instruction(&Instruction::LocalGet(b));
                func.instruction(&Instruction::End);
            }
            Operation::Sign => {
                // sign(x) = if x < 0 then -1 else if x > 0 then 1 else 0
                let tmp = self.allocate_local("__sign_tmp");
                func.instruction(&Instruction::LocalTee(tmp));
                func.instruction(&Instruction::I64Const(0));
                func.instruction(&Instruction::I64LtS);
                func.instruction(&Instruction::If(BlockType::Result(ValType::I64)));
                func.instruction(&Instruction::I64Const(-1));
                func.instruction(&Instruction::Else);
                func.instruction(&Instruction::LocalGet(tmp));
                func.instruction(&Instruction::I64Const(0));
                func.instruction(&Instruction::I64GtS);
                func.instruction(&Instruction::I64ExtendI32U);
                func.instruction(&Instruction::End);
            }
            Operation::Clamp => {
                // clamp(x, lo, hi) = max(lo, min(x, hi))
                let hi = self.allocate_local("__clamp_hi");
                let lo = self.allocate_local("__clamp_lo");
                let x = self.allocate_local("__clamp_x");
                func.instruction(&Instruction::LocalSet(hi));
                func.instruction(&Instruction::LocalSet(lo));
                func.instruction(&Instruction::LocalSet(x));

                // min(x, hi)
                func.instruction(&Instruction::LocalGet(x));
                func.instruction(&Instruction::LocalGet(hi));
                func.instruction(&Instruction::I64LtS);
                func.instruction(&Instruction::If(BlockType::Result(ValType::I64)));
                func.instruction(&Instruction::LocalGet(x));
                func.instruction(&Instruction::Else);
                func.instruction(&Instruction::LocalGet(hi));
                func.instruction(&Instruction::End);

                // max(lo, result)
                let mid = self.allocate_local("__clamp_mid");
                func.instruction(&Instruction::LocalSet(mid));
                func.instruction(&Instruction::LocalGet(lo));
                func.instruction(&Instruction::LocalGet(mid));
                func.instruction(&Instruction::I64GtS);
                func.instruction(&Instruction::If(BlockType::Result(ValType::I64)));
                func.instruction(&Instruction::LocalGet(lo));
                func.instruction(&Instruction::Else);
                func.instruction(&Instruction::LocalGet(mid));
                func.instruction(&Instruction::End);
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

    /// Try to compile a call to an extern-abstract function.
    /// Returns Some(import_index) if this is a registered extern function, None otherwise.
    fn try_compile_runtime_call(&mut self, call: &CallStep, func: &mut Function) -> Result<Option<u32>, CodegenError> {
        // Look up the function name in registered extern imports
        let ext = match self.extern_imports.get(&call.fn_name) {
            Some(ext) => ext.clone(),
            None => return Ok(None),
        };

        // Compile each argument and unpack according to its type
        for (i, arg) in call.args.iter().enumerate() {
            self.compile_input(&arg.source, func)?;

            let param_kind = ext.param_types.get(i).copied().unwrap_or(ExternParamKind::FatPointer);
            match param_kind {
                ExternParamKind::String | ExternParamKind::FatPointer => {
                    // i64 fat pointer on stack → unpack to (i32 ptr, i32 len)
                    let temp = self.allocate_local(&format!("__ext_arg_{}", i));
                    func.instruction(&Instruction::LocalSet(temp));
                    // ptr = fat_ptr >> 32
                    func.instruction(&Instruction::LocalGet(temp));
                    func.instruction(&Instruction::I64Const(32));
                    func.instruction(&Instruction::I64ShrU);
                    func.instruction(&Instruction::I32WrapI64);
                    // len = fat_ptr & 0xFFFFFFFF
                    func.instruction(&Instruction::LocalGet(temp));
                    func.instruction(&Instruction::I32WrapI64);
                }
                ExternParamKind::Int | ExternParamKind::Bool => {
                    // i64 on stack → wrap to i32
                    func.instruction(&Instruction::I32WrapI64);
                }
            }
        }

        Ok(Some(ext.func_index))
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
            ReturnValue::Struct(s) => {
                // Allocate struct on heap and return pointer as i64
                let struct_size = (s.fields.len() as u32) * 8;
                let ptr_local = self.allocate_local("__ret_struct_ptr");

                // Bump-allocate: ptr = heap_ptr; heap_ptr += size
                func.instruction(&Instruction::GlobalGet(0));
                func.instruction(&Instruction::I64ExtendI32U);
                func.instruction(&Instruction::LocalTee(ptr_local));
                func.instruction(&Instruction::I32WrapI64);
                func.instruction(&Instruction::I32Const(struct_size as i32));
                func.instruction(&Instruction::I32Add);
                func.instruction(&Instruction::GlobalSet(0));

                // Store each field
                for (i, field) in s.fields.iter().enumerate() {
                    let offset = (i as u32) * 8;
                    func.instruction(&Instruction::LocalGet(ptr_local));
                    func.instruction(&Instruction::I32WrapI64);
                    self.compile_input(&field.value, func)?;
                    func.instruction(&Instruction::I64Store(MemArg {
                        offset: offset as u64,
                        align: 3,
                        memory_index: 0,
                    }));
                }

                // Leave struct pointer on stack
                func.instruction(&Instruction::LocalGet(ptr_local));
            }
            ReturnValue::Variant(v) => {
                // For variants, use a tag-based encoding:
                // Return a tagged value (tag in high bits, payload in low bits)
                // Simple approach: hash the variant name to get a tag value
                let tag: i64 = v.ty.bytes().map(|b| b as i64).sum();
                func.instruction(&Instruction::I64Const(tag));
            }
        }
        Ok(())
    }

    /// Compile an if step
    fn compile_if_step(&mut self, if_step: &IfStep, func: &mut Function) -> Result<(), CodegenError> {
        // Load condition value (variable, field access, or literal)
        self.compile_input(&if_step.condition, func)?;

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
            BindSource::Field { of, field } => {
                // Get struct pointer from local variable
                let local = *self.locals.get(of)
                    .ok_or_else(|| CodegenError::UndefinedFunction { name: of.clone() })?;

                // Look up field offset from struct layout
                if let Some(type_name) = self.local_types.get(of).cloned() {
                    if let Some(layout) = self.struct_layouts.get(&type_name) {
                        if let Some(field_layout) = layout.fields.get(field) {
                            let offset = field_layout.offset as u64;
                            func.instruction(&Instruction::LocalGet(local));
                            func.instruction(&Instruction::I32WrapI64); // i64 -> i32 ptr
                            // Load i64 value from (ptr + offset)
                            func.instruction(&Instruction::I64Load(MemArg {
                                offset,
                                align: 3, // 2^3 = 8 byte alignment
                                memory_index: 0,
                            }));
                        } else {
                            // Unknown field - return the value itself as opaque i64
                            func.instruction(&Instruction::LocalGet(local));
                        }
                    } else {
                        // Unknown struct layout - return the value itself as opaque i64
                        func.instruction(&Instruction::LocalGet(local));
                    }
                } else {
                    // Unknown type for variable - treat as opaque value
                    // For extern-returned values, the host runtime manages the layout
                    func.instruction(&Instruction::LocalGet(local));
                }
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
            InputSource::Field { of, field } => {
                // Get struct pointer from local variable
                let local = *self.locals.get(of)
                    .ok_or_else(|| CodegenError::UndefinedFunction { name: of.clone() })?;

                // Look up field offset from struct layout
                if let Some(type_name) = self.local_types.get(of).cloned() {
                    if let Some(layout) = self.struct_layouts.get(&type_name) {
                        if let Some(field_layout) = layout.fields.get(field) {
                            let offset = field_layout.offset as u64;
                            func.instruction(&Instruction::LocalGet(local));
                            func.instruction(&Instruction::I32WrapI64); // i64 -> i32 ptr
                            // Load i64 value from (ptr + offset)
                            func.instruction(&Instruction::I64Load(MemArg {
                                offset,
                                align: 3, // 2^3 = 8 byte alignment
                                memory_index: 0,
                            }));
                        } else {
                            // Unknown field - return the value itself as opaque i64
                            func.instruction(&Instruction::LocalGet(local));
                        }
                    } else {
                        // Unknown struct layout - return the value itself as opaque i64
                        func.instruction(&Instruction::LocalGet(local));
                    }
                } else {
                    // Unknown type for variable - treat as opaque value
                    func.instruction(&Instruction::LocalGet(local));
                }
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

    /// Check if a function signature returns Unit (no WASM return value)
    fn extern_returns_unit(&self, sig: &FunctionSignature) -> bool {
        match &sig.returns {
            Some(ret) => self.return_type_to_valtype(ret).is_none(),
            None => true, // No return type means void/Unit
        }
    }

    /// Check if a called function (by name) has a WASM return value.
    /// Returns true if the function pushes a value onto the WASM stack.
    fn call_has_return_value(&self, fn_name: &str) -> bool {
        // Check extern imports - look up the import's result types
        if let Some(ext) = self.extern_imports.get(fn_name) {
            return self.imports.imports.get(ext.func_index as usize)
                .map(|(_, _, _, results)| !results.is_empty())
                .unwrap_or(true);
        }
        // Check if this is a known void (Unit-returning) user function
        if self.void_functions.contains(fn_name) {
            return false;
        }
        // Default: assume it returns a value (safe fallback for user functions with returns)
        true
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
                "Unit" => None, // Unit means no return value in WASM
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


/// Map a Covenant type to the extern parameter calling convention
fn type_to_extern_param_kind(ty: &Type) -> ExternParamKind {
    match &ty.kind {
        TypeKind::Named(path) => {
            let name = path.segments.last().map(|s| s.as_str()).unwrap_or("");
            match name {
                "String" => ExternParamKind::String,
                "Int" => ExternParamKind::Int,
                "Bool" => ExternParamKind::Bool,
                // Any, List, Map, etc. use fat pointer convention
                _ => ExternParamKind::FatPointer,
            }
        }
        TypeKind::List(_) => ExternParamKind::FatPointer,
        _ => ExternParamKind::FatPointer,
    }
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
