//! Graph Access Interface (GAI) Code Generation
//!
//! Takes a DataGraph and generates:
//! 1. A data segment layout with embedded graph data (string pool + offset tables)
//! 2. Internal WASM functions for querying the embedded data
//!
//! The GAI provides an abstraction between query code and the underlying
//! data representation. Query functions call GAI functions, which read
//! from the data segment. This allows the representation to change
//! without affecting query code.

use crate::data_graph::{DataGraph, DataRelation};

/// Layout offsets computed during data segment generation
#[derive(Debug, Clone)]
pub struct GraphLayout {
    /// Base offset of the graph data in the WASM data segment
    pub base_offset: u32,
    /// Number of nodes in the graph
    pub node_count: u32,
    /// Number of outgoing relations
    pub outgoing_count: u32,
    /// Number of incoming relations
    pub incoming_count: u32,
    /// Number of distinct relation types
    pub rel_type_count: u32,

    // --- Section offsets (relative to base_offset) ---

    /// Offset to the string pool (all node IDs, content, notes as contiguous bytes)
    pub string_pool_offset: u32,
    /// Total size of the string pool
    pub string_pool_size: u32,

    /// Offset to the node ID table: [(pool_offset: u32, len: u32), ...] per node
    pub node_id_table_offset: u32,

    /// Offset to the kind table: [(pool_offset: u32, len: u32), ...] per node
    pub kind_table_offset: u32,

    /// Offset to the content table: [(pool_offset: u32, len: u32), ...] per node
    pub content_table_offset: u32,

    /// Offset to the notes table: [(pool_offset: u32, count: u16, <padding>), ...] per node
    /// Each note entry in pool is: [(pool_offset: u32, len: u32), ...]
    pub notes_index_offset: u32,
    /// Offset to flattened notes entries: [(pool_offset: u32, len: u32), ...]
    pub notes_entries_offset: u32,
    /// Total number of note entries
    pub notes_entry_count: u32,

    /// Offset to outgoing relations table: [(to_idx: u16, rel_type: u8, pad: u8), ...] sorted by from_idx
    pub outgoing_table_offset: u32,

    /// Offset to incoming relations table: [(from_idx: u16, rel_type: u8, pad: u8), ...] sorted by to_idx
    pub incoming_table_offset: u32,

    /// Offset to adjacency index: [(out_start: u16, out_count: u16, in_start: u16, in_count: u16), ...] per node
    pub adjacency_index_offset: u32,

    /// Offset to relation type string table: [(pool_offset: u32, len: u32), ...] per type
    pub rel_type_table_offset: u32,

    /// Offset to metadata table: [(pool_offset: u32, count: u16, <padding>), ...] per node
    pub metadata_index_offset: u32,
    /// Offset to flattened metadata entries: [(key_offset: u32, key_len: u32, val_offset: u32, val_len: u32), ...]
    pub metadata_entries_offset: u32,
    /// Total number of metadata entries
    pub metadata_entry_count: u32,

    /// Total size of the graph data segment
    pub total_size: u32,
}

/// Generates the data segment bytes for a DataGraph
pub fn generate_graph_segment(graph: &DataGraph, base_offset: u32) -> (Vec<u8>, GraphLayout) {
    let mut data = Vec::new();
    let mut layout = GraphLayout {
        base_offset,
        node_count: graph.nodes.len() as u32,
        outgoing_count: graph.outgoing.len() as u32,
        incoming_count: graph.incoming.len() as u32,
        rel_type_count: graph.relation_types.len() as u32,
        string_pool_offset: 0,
        string_pool_size: 0,
        node_id_table_offset: 0,
        kind_table_offset: 0,
        content_table_offset: 0,
        notes_index_offset: 0,
        notes_entries_offset: 0,
        notes_entry_count: 0,
        outgoing_table_offset: 0,
        incoming_table_offset: 0,
        adjacency_index_offset: 0,
        rel_type_table_offset: 0,
        metadata_index_offset: 0,
        metadata_entries_offset: 0,
        metadata_entry_count: 0,
        total_size: 0,
    };

    // === Phase 1: Build string pool ===
    // Collect all strings and assign offsets within the pool
    let mut string_pool = Vec::new();

    // Helper: add a string to pool, return (offset_in_pool, len)
    let add_to_pool = |pool: &mut Vec<u8>, s: &str| -> (u32, u32) {
        let offset = pool.len() as u32;
        let len = s.len() as u32;
        pool.extend_from_slice(s.as_bytes());
        (offset, len)
    };

    // Node IDs
    let mut node_id_entries: Vec<(u32, u32)> = Vec::new();
    for node in &graph.nodes {
        let entry = add_to_pool(&mut string_pool, &node.id);
        node_id_entries.push(entry);
    }

    // Node kinds
    let mut kind_entries: Vec<(u32, u32)> = Vec::new();
    for node in &graph.nodes {
        let entry = add_to_pool(&mut string_pool, &node.kind);
        kind_entries.push(entry);
    }

    // Node content
    let mut content_entries: Vec<(u32, u32)> = Vec::new();
    for node in &graph.nodes {
        let entry = add_to_pool(&mut string_pool, &node.content);
        content_entries.push(entry);
    }

    // Node notes (flattened)
    let mut notes_index: Vec<(u32, u16)> = Vec::new(); // (start_in_notes_entries, count)
    let mut notes_entries: Vec<(u32, u32)> = Vec::new();
    for node in &graph.nodes {
        let start = notes_entries.len() as u32;
        let count = node.notes.len() as u16;
        for note in &node.notes {
            let entry = add_to_pool(&mut string_pool, note);
            notes_entries.push(entry);
        }
        notes_index.push((start, count));
    }
    layout.notes_entry_count = notes_entries.len() as u32;

    // Node metadata (flattened)
    let mut metadata_index: Vec<(u32, u16)> = Vec::new(); // (start_in_metadata_entries, count)
    let mut metadata_entries: Vec<(u32, u32, u32, u32)> = Vec::new(); // (key_off, key_len, val_off, val_len)
    for node in &graph.nodes {
        let start = metadata_entries.len() as u32;
        let count = node.metadata.len() as u16;
        for (key, val) in &node.metadata {
            let key_entry = add_to_pool(&mut string_pool, key);
            let val_entry = add_to_pool(&mut string_pool, val);
            metadata_entries.push((key_entry.0, key_entry.1, val_entry.0, val_entry.1));
        }
        metadata_index.push((start, count));
    }
    layout.metadata_entry_count = metadata_entries.len() as u32;

    // Relation type strings
    let mut rel_type_entries: Vec<(u32, u32)> = Vec::new();
    for rt in &graph.relation_types {
        let entry = add_to_pool(&mut string_pool, rt);
        rel_type_entries.push(entry);
    }

    // === Phase 2: Write sections in order ===

    // String pool
    layout.string_pool_offset = data.len() as u32;
    data.extend_from_slice(&string_pool);
    layout.string_pool_size = string_pool.len() as u32;
    // Align to 4 bytes
    align_to(&mut data, 4);

    // Node ID table: [(pool_offset: u32, len: u32), ...] per node
    layout.node_id_table_offset = data.len() as u32;
    for (offset, len) in &node_id_entries {
        data.extend_from_slice(&offset.to_le_bytes());
        data.extend_from_slice(&len.to_le_bytes());
    }

    // Kind table: [(pool_offset: u32, len: u32), ...] per node
    layout.kind_table_offset = data.len() as u32;
    for (offset, len) in &kind_entries {
        data.extend_from_slice(&offset.to_le_bytes());
        data.extend_from_slice(&len.to_le_bytes());
    }

    // Content table: [(pool_offset: u32, len: u32), ...] per node
    layout.content_table_offset = data.len() as u32;
    for (offset, len) in &content_entries {
        data.extend_from_slice(&offset.to_le_bytes());
        data.extend_from_slice(&len.to_le_bytes());
    }

    // Notes index: [(entries_start: u32, count: u16, pad: u16), ...] per node
    layout.notes_index_offset = data.len() as u32;
    for (start, count) in &notes_index {
        data.extend_from_slice(&start.to_le_bytes());
        data.extend_from_slice(&count.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes()); // padding
    }

    // Notes entries: [(pool_offset: u32, len: u32), ...]
    layout.notes_entries_offset = data.len() as u32;
    for (offset, len) in &notes_entries {
        data.extend_from_slice(&offset.to_le_bytes());
        data.extend_from_slice(&len.to_le_bytes());
    }

    // Metadata index: [(entries_start: u32, count: u16, pad: u16), ...] per node
    layout.metadata_index_offset = data.len() as u32;
    for (start, count) in &metadata_index {
        data.extend_from_slice(&start.to_le_bytes());
        data.extend_from_slice(&count.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes()); // padding
    }

    // Metadata entries: [(key_off: u32, key_len: u32, val_off: u32, val_len: u32), ...]
    layout.metadata_entries_offset = data.len() as u32;
    for (ko, kl, vo, vl) in &metadata_entries {
        data.extend_from_slice(&ko.to_le_bytes());
        data.extend_from_slice(&kl.to_le_bytes());
        data.extend_from_slice(&vo.to_le_bytes());
        data.extend_from_slice(&vl.to_le_bytes());
    }

    // Outgoing relations table: [(to_idx: u16, rel_type: u8, pad: u8), ...]
    layout.outgoing_table_offset = data.len() as u32;
    for rel in &graph.outgoing {
        write_relation_entry(&mut data, rel, &graph.rel_type_to_idx, true);
    }

    // Incoming relations table: [(from_idx: u16, rel_type: u8, pad: u8), ...]
    layout.incoming_table_offset = data.len() as u32;
    for rel in &graph.incoming {
        write_relation_entry(&mut data, rel, &graph.rel_type_to_idx, false);
    }

    // Adjacency index: [(out_start: u16, out_count: u16, in_start: u16, in_count: u16), ...] per node
    layout.adjacency_index_offset = data.len() as u32;
    for adj in &graph.adjacency {
        data.extend_from_slice(&(adj.out_start as u16).to_le_bytes());
        data.extend_from_slice(&(adj.out_count as u16).to_le_bytes());
        data.extend_from_slice(&(adj.in_start as u16).to_le_bytes());
        data.extend_from_slice(&(adj.in_count as u16).to_le_bytes());
    }

    // Relation type string table: [(pool_offset: u32, len: u32), ...]
    layout.rel_type_table_offset = data.len() as u32;
    for (offset, len) in &rel_type_entries {
        data.extend_from_slice(&offset.to_le_bytes());
        data.extend_from_slice(&len.to_le_bytes());
    }

    layout.total_size = data.len() as u32;
    (data, layout)
}

/// Write a single relation entry (4 bytes: target_idx u16, rel_type u8, pad u8)
fn write_relation_entry(
    data: &mut Vec<u8>,
    rel: &DataRelation,
    rel_type_to_idx: &std::collections::HashMap<String, u8>,
    is_outgoing: bool,
) {
    let target_idx = if is_outgoing { rel.to_idx } else { rel.from_idx } as u16;
    let rel_type = rel_type_to_idx.get(&rel.rel_type).copied().unwrap_or(0);
    data.extend_from_slice(&target_idx.to_le_bytes());
    data.push(rel_type);
    data.push(0); // padding
}

/// Align data to the given boundary
fn align_to(data: &mut Vec<u8>, alignment: usize) {
    let padding = (alignment - (data.len() % alignment)) % alignment;
    data.extend(std::iter::repeat(0u8).take(padding));
}

// ===== GAI Function Generation =====

use wasm_encoder::{Function, Instruction, ValType, MemArg};

/// The set of GAI function indices (relative to the module's function section)
#[derive(Debug, Clone)]
pub struct GaiFunctionIndices {
    /// _gai_node_count() -> i32
    pub node_count: u32,
    /// _gai_get_node_id(idx: i32) -> i64  (fat ptr)
    pub get_node_id: u32,
    /// _gai_get_node_kind(idx: i32) -> i64  (fat ptr)
    pub get_node_kind: u32,
    /// _gai_get_node_content(idx: i32) -> i64  (fat ptr)
    pub get_node_content: u32,
    /// _gai_get_outgoing_count(node_idx: i32) -> i32
    pub get_outgoing_count: u32,
    /// _gai_get_outgoing_rel(node_idx: i32, rel_offset: i32) -> i64
    /// Returns packed: (target_idx: u16 | rel_type: u8) as i64, or -1 if out of bounds
    pub get_outgoing_rel: u32,
    /// _gai_get_incoming_count(node_idx: i32) -> i32
    pub get_incoming_count: u32,
    /// _gai_get_incoming_rel(node_idx: i32, rel_offset: i32) -> i64
    /// Returns packed: (source_idx: u16 | rel_type: u8) as i64, or -1 if out of bounds
    pub get_incoming_rel: u32,
    /// _gai_find_by_id(id_ptr: i32, id_len: i32) -> i32  (node index or -1)
    pub find_by_id: u32,
    /// _gai_content_contains(node_idx: i32, term_ptr: i32, term_len: i32) -> i32  (bool)
    pub content_contains: u32,
    /// _gai_get_rel_type_name(type_idx: i32) -> i64  (fat ptr)
    pub get_rel_type_name: u32,
    /// cov_alloc(size: i32) -> i32  (bump allocator for runtime string parameters)
    pub alloc: u32,
}

/// Number of GAI functions
pub const GAI_FUNCTION_COUNT: u32 = 12;

/// Generate GAI function type signatures.
/// Returns Vec of (params, results) for the type section.
pub fn gai_function_types() -> Vec<(Vec<ValType>, Vec<ValType>)> {
    vec![
        // 0: _gai_node_count() -> i32
        (vec![], vec![ValType::I32]),
        // 1: _gai_get_node_id(idx: i32) -> i64
        (vec![ValType::I32], vec![ValType::I64]),
        // 2: _gai_get_node_kind(idx: i32) -> i64
        (vec![ValType::I32], vec![ValType::I64]),
        // 3: _gai_get_node_content(idx: i32) -> i64
        (vec![ValType::I32], vec![ValType::I64]),
        // 4: _gai_get_outgoing_count(node_idx: i32) -> i32
        (vec![ValType::I32], vec![ValType::I32]),
        // 5: _gai_get_outgoing_rel(node_idx: i32, rel_offset: i32) -> i64
        (vec![ValType::I32, ValType::I32], vec![ValType::I64]),
        // 6: _gai_get_incoming_count(node_idx: i32) -> i32
        (vec![ValType::I32], vec![ValType::I32]),
        // 7: _gai_get_incoming_rel(node_idx: i32, rel_offset: i32) -> i64
        (vec![ValType::I32, ValType::I32], vec![ValType::I64]),
        // 8: _gai_find_by_id(id_ptr: i32, id_len: i32) -> i32
        (vec![ValType::I32, ValType::I32], vec![ValType::I32]),
        // 9: _gai_content_contains(node_idx: i32, term_ptr: i32, term_len: i32) -> i32
        (vec![ValType::I32, ValType::I32, ValType::I32], vec![ValType::I32]),
        // 10: _gai_get_rel_type_name(type_idx: i32) -> i64
        (vec![ValType::I32], vec![ValType::I64]),
        // 11: cov_alloc(size: i32) -> i32 (bump allocator)
        (vec![ValType::I32], vec![ValType::I32]),
    ]
}

/// Generate the body of _gai_node_count
pub fn gen_gai_node_count(layout: &GraphLayout) -> Function {
    let mut func = Function::new(vec![]);
    func.instruction(&Instruction::I32Const(layout.node_count as i32));
    func.instruction(&Instruction::End);
    func
}

/// Generate the body of _gai_get_node_id(idx: i32) -> i64
/// Reads from node_id_table: base + node_id_table_offset + idx * 8
/// Returns fat pointer: (base + string_pool_offset + pool_offset) << 32 | len
pub fn gen_gai_get_node_id(layout: &GraphLayout) -> Function {
    gen_table_lookup_fat_ptr(layout, layout.node_id_table_offset)
}

/// Generate the body of _gai_get_node_kind(idx: i32) -> i64
pub fn gen_gai_get_node_kind(layout: &GraphLayout) -> Function {
    gen_table_lookup_fat_ptr(layout, layout.kind_table_offset)
}

/// Generate the body of _gai_get_node_content(idx: i32) -> i64
pub fn gen_gai_get_node_content(layout: &GraphLayout) -> Function {
    gen_table_lookup_fat_ptr(layout, layout.content_table_offset)
}

/// Generic: lookup a (pool_offset: u32, len: u32) entry in a table and return fat ptr
fn gen_table_lookup_fat_ptr(layout: &GraphLayout, table_offset: u32) -> Function {
    // Local 0: idx (param)
    let mut func = Function::new(vec![
        (1, ValType::I32), // local 1: entry_addr
        (1, ValType::I32), // local 2: pool_offset
        (1, ValType::I32), // local 3: len
    ]);

    // entry_addr = base_offset + table_offset + idx * 8
    func.instruction(&Instruction::LocalGet(0)); // idx
    func.instruction(&Instruction::I32Const(8));
    func.instruction(&Instruction::I32Mul);
    func.instruction(&Instruction::I32Const((layout.base_offset + table_offset) as i32));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalSet(1)); // entry_addr

    // pool_offset = load_u32(entry_addr)
    func.instruction(&Instruction::LocalGet(1));
    func.instruction(&Instruction::I32Load(MemArg {
        offset: 0,
        align: 2, // 4-byte aligned
        memory_index: 0,
    }));
    func.instruction(&Instruction::LocalSet(2)); // pool_offset

    // len = load_u32(entry_addr + 4)
    func.instruction(&Instruction::LocalGet(1));
    func.instruction(&Instruction::I32Load(MemArg {
        offset: 4,
        align: 2,
        memory_index: 0,
    }));
    func.instruction(&Instruction::LocalSet(3)); // len

    // Return fat pointer: ((base + string_pool_offset + pool_offset) << 32) | len
    // High 32 bits: absolute address of string
    func.instruction(&Instruction::LocalGet(2)); // pool_offset
    func.instruction(&Instruction::I32Const((layout.base_offset + layout.string_pool_offset) as i32));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::I64ExtendI32U);
    func.instruction(&Instruction::I64Const(32));
    func.instruction(&Instruction::I64Shl);

    // Low 32 bits: length
    func.instruction(&Instruction::LocalGet(3)); // len
    func.instruction(&Instruction::I64ExtendI32U);
    func.instruction(&Instruction::I64Or);

    func.instruction(&Instruction::End);
    func
}

/// Generate _gai_get_outgoing_count(node_idx: i32) -> i32
pub fn gen_gai_get_outgoing_count(layout: &GraphLayout) -> Function {
    gen_adjacency_count(layout, 2) // out_count is at offset +2 in adjacency entry
}

/// Generate _gai_get_incoming_count(node_idx: i32) -> i32
pub fn gen_gai_get_incoming_count(layout: &GraphLayout) -> Function {
    gen_adjacency_count(layout, 6) // in_count is at offset +6 in adjacency entry
}

/// Generic: read a u16 count field from the adjacency index
fn gen_adjacency_count(layout: &GraphLayout, field_offset: u32) -> Function {
    // Local 0: node_idx (param)
    let mut func = Function::new(vec![]);

    // addr = base + adjacency_index_offset + node_idx * 8 + field_offset
    func.instruction(&Instruction::LocalGet(0)); // node_idx
    func.instruction(&Instruction::I32Const(8)); // each adjacency entry is 8 bytes
    func.instruction(&Instruction::I32Mul);
    func.instruction(&Instruction::I32Const((layout.base_offset + layout.adjacency_index_offset + field_offset) as i32));
    func.instruction(&Instruction::I32Add);

    // Load u16 and extend to i32
    func.instruction(&Instruction::I32Load16U(MemArg {
        offset: 0,
        align: 1,
        memory_index: 0,
    }));

    func.instruction(&Instruction::End);
    func
}

/// Generate _gai_get_outgoing_rel(node_idx: i32, rel_offset: i32) -> i64
/// Returns packed: ((target_idx as u32) << 8) | rel_type_idx, as i64
/// Returns -1 if out of bounds
pub fn gen_gai_get_outgoing_rel(layout: &GraphLayout) -> Function {
    gen_get_rel(layout, 0, layout.outgoing_table_offset) // out_start at offset 0
}

/// Generate _gai_get_incoming_rel(node_idx: i32, rel_offset: i32) -> i64
pub fn gen_gai_get_incoming_rel(layout: &GraphLayout) -> Function {
    gen_get_rel(layout, 4, layout.incoming_table_offset) // in_start at offset 4
}

/// Generic: get a relation entry by node index and offset within that node's edges
fn gen_get_rel(layout: &GraphLayout, adj_start_offset: u32, table_offset: u32) -> Function {
    // Params: local 0 = node_idx, local 1 = rel_offset
    let mut func = Function::new(vec![
        (1, ValType::I32), // local 2: adj_addr
        (1, ValType::I32), // local 3: start
        (1, ValType::I32), // local 4: count
        (1, ValType::I32), // local 5: entry_addr
    ]);

    // adj_addr = base + adjacency_index_offset + node_idx * 8
    func.instruction(&Instruction::LocalGet(0)); // node_idx
    func.instruction(&Instruction::I32Const(8));
    func.instruction(&Instruction::I32Mul);
    func.instruction(&Instruction::I32Const((layout.base_offset + layout.adjacency_index_offset) as i32));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalSet(2)); // adj_addr

    // start = load_u16(adj_addr + adj_start_offset)
    func.instruction(&Instruction::LocalGet(2));
    func.instruction(&Instruction::I32Const(adj_start_offset as i32));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::I32Load16U(MemArg {
        offset: 0,
        align: 1,
        memory_index: 0,
    }));
    func.instruction(&Instruction::LocalSet(3)); // start

    // count = load_u16(adj_addr + adj_start_offset + 2)
    func.instruction(&Instruction::LocalGet(2));
    func.instruction(&Instruction::I32Const(adj_start_offset as i32 + 2));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::I32Load16U(MemArg {
        offset: 0,
        align: 1,
        memory_index: 0,
    }));
    func.instruction(&Instruction::LocalSet(4)); // count

    // Bounds check: if rel_offset >= count, return -1
    func.instruction(&Instruction::LocalGet(1)); // rel_offset
    func.instruction(&Instruction::LocalGet(4)); // count
    func.instruction(&Instruction::I32GeU);
    func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    func.instruction(&Instruction::I64Const(-1));
    func.instruction(&Instruction::Return);
    func.instruction(&Instruction::End); // end if

    // entry_addr = base + table_offset + (start + rel_offset) * 4
    func.instruction(&Instruction::LocalGet(3)); // start
    func.instruction(&Instruction::LocalGet(1)); // rel_offset
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::I32Const(4)); // each relation entry is 4 bytes
    func.instruction(&Instruction::I32Mul);
    func.instruction(&Instruction::I32Const((layout.base_offset + table_offset) as i32));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalSet(5)); // entry_addr

    // Read entry: target_idx (u16) at offset 0, rel_type (u8) at offset 2
    // Pack as: ((target_idx as i64) << 8) | rel_type
    func.instruction(&Instruction::LocalGet(5));
    func.instruction(&Instruction::I32Load16U(MemArg {
        offset: 0,
        align: 1,
        memory_index: 0,
    }));
    func.instruction(&Instruction::I64ExtendI32U);
    func.instruction(&Instruction::I64Const(8));
    func.instruction(&Instruction::I64Shl);

    func.instruction(&Instruction::LocalGet(5));
    func.instruction(&Instruction::I32Load8U(MemArg {
        offset: 2,
        align: 0,
        memory_index: 0,
    }));
    func.instruction(&Instruction::I64ExtendI32U);
    func.instruction(&Instruction::I64Or);

    func.instruction(&Instruction::End);
    func
}

/// Generate _gai_find_by_id(id_ptr: i32, id_len: i32) -> i32
/// Linear scan over node ID table, comparing strings byte-by-byte.
/// Returns node index or -1 if not found.
pub fn gen_gai_find_by_id(layout: &GraphLayout) -> Function {
    // Params: local 0 = id_ptr, local 1 = id_len
    let mut func = Function::new(vec![
        (1, ValType::I32), // local 2: i (loop counter)
        (1, ValType::I32), // local 3: entry_addr
        (1, ValType::I32), // local 4: node_str_ptr (absolute)
        (1, ValType::I32), // local 5: node_str_len
        (1, ValType::I32), // local 6: j (byte compare counter)
        (1, ValType::I32), // local 7: match_flag
    ]);

    // i = 0
    func.instruction(&Instruction::I32Const(0));
    func.instruction(&Instruction::LocalSet(2));

    // Loop over all nodes
    func.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty)); // block (break target)
    func.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty)); // loop

    // if i >= node_count, break
    func.instruction(&Instruction::LocalGet(2));
    func.instruction(&Instruction::I32Const(layout.node_count as i32));
    func.instruction(&Instruction::I32GeU);
    func.instruction(&Instruction::BrIf(1)); // break to outer block

    // entry_addr = base + node_id_table_offset + i * 8
    func.instruction(&Instruction::LocalGet(2));
    func.instruction(&Instruction::I32Const(8));
    func.instruction(&Instruction::I32Mul);
    func.instruction(&Instruction::I32Const((layout.base_offset + layout.node_id_table_offset) as i32));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalSet(3));

    // node_str_len = load_u32(entry_addr + 4)
    func.instruction(&Instruction::LocalGet(3));
    func.instruction(&Instruction::I32Load(MemArg { offset: 4, align: 2, memory_index: 0 }));
    func.instruction(&Instruction::LocalSet(5));

    // Quick length check: if id_len != node_str_len, skip
    func.instruction(&Instruction::LocalGet(1)); // id_len
    func.instruction(&Instruction::LocalGet(5)); // node_str_len
    func.instruction(&Instruction::I32Ne);
    func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    // Skip to next iteration
    func.instruction(&Instruction::LocalGet(2));
    func.instruction(&Instruction::I32Const(1));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalSet(2));
    func.instruction(&Instruction::Br(1)); // continue loop
    func.instruction(&Instruction::End); // end if

    // node_str_ptr = base + string_pool_offset + load_u32(entry_addr)
    func.instruction(&Instruction::LocalGet(3));
    func.instruction(&Instruction::I32Load(MemArg { offset: 0, align: 2, memory_index: 0 }));
    func.instruction(&Instruction::I32Const((layout.base_offset + layout.string_pool_offset) as i32));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalSet(4));

    // Byte-by-byte comparison
    // j = 0, match_flag = 1
    func.instruction(&Instruction::I32Const(0));
    func.instruction(&Instruction::LocalSet(6));
    func.instruction(&Instruction::I32Const(1));
    func.instruction(&Instruction::LocalSet(7));

    // Inner loop: compare bytes
    func.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty)); // inner break
    func.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty)); // inner loop

    // if j >= id_len, break (all bytes matched)
    func.instruction(&Instruction::LocalGet(6));
    func.instruction(&Instruction::LocalGet(1));
    func.instruction(&Instruction::I32GeU);
    func.instruction(&Instruction::BrIf(1)); // break inner

    // Compare: load byte from id_ptr + j
    func.instruction(&Instruction::LocalGet(0)); // id_ptr
    func.instruction(&Instruction::LocalGet(6)); // j
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::I32Load8U(MemArg { offset: 0, align: 0, memory_index: 0 }));

    // Load byte from node_str_ptr + j
    func.instruction(&Instruction::LocalGet(4)); // node_str_ptr
    func.instruction(&Instruction::LocalGet(6)); // j
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::I32Load8U(MemArg { offset: 0, align: 0, memory_index: 0 }));

    // If not equal, set match_flag = 0, break
    func.instruction(&Instruction::I32Ne);
    func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    func.instruction(&Instruction::I32Const(0));
    func.instruction(&Instruction::LocalSet(7)); // match_flag = 0
    func.instruction(&Instruction::Br(2)); // break inner block
    func.instruction(&Instruction::End); // end if

    // j++
    func.instruction(&Instruction::LocalGet(6));
    func.instruction(&Instruction::I32Const(1));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalSet(6));
    func.instruction(&Instruction::Br(0)); // continue inner loop
    func.instruction(&Instruction::End); // end inner loop
    func.instruction(&Instruction::End); // end inner block

    // If match_flag == 1, return i
    func.instruction(&Instruction::LocalGet(7));
    func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    func.instruction(&Instruction::LocalGet(2));
    func.instruction(&Instruction::Return);
    func.instruction(&Instruction::End); // end if

    // i++
    func.instruction(&Instruction::LocalGet(2));
    func.instruction(&Instruction::I32Const(1));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalSet(2));
    func.instruction(&Instruction::Br(0)); // continue outer loop
    func.instruction(&Instruction::End); // end loop
    func.instruction(&Instruction::End); // end block

    // Not found: return -1
    func.instruction(&Instruction::I32Const(-1));
    func.instruction(&Instruction::End);
    func
}

/// Generate _gai_content_contains(node_idx: i32, term_ptr: i32, term_len: i32) -> i32
/// Substring search: checks if the node's content contains the given term.
/// Returns 1 if found, 0 otherwise. Uses naive O(n*m) algorithm.
pub fn gen_gai_content_contains(layout: &GraphLayout) -> Function {
    // Params: local 0 = node_idx, local 1 = term_ptr, local 2 = term_len
    let mut func = Function::new(vec![
        (1, ValType::I32), // local 3: content_ptr (absolute)
        (1, ValType::I32), // local 4: content_len
        (1, ValType::I32), // local 5: i (outer position)
        (1, ValType::I32), // local 6: j (inner byte compare)
        (1, ValType::I32), // local 7: matched
        (1, ValType::I32), // local 8: entry_addr
    ]);

    // If term_len == 0, return 1 (empty string is always contained)
    func.instruction(&Instruction::LocalGet(2));
    func.instruction(&Instruction::I32Eqz);
    func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    func.instruction(&Instruction::I32Const(1));
    func.instruction(&Instruction::Return);
    func.instruction(&Instruction::End);

    // entry_addr = base + content_table_offset + node_idx * 8
    func.instruction(&Instruction::LocalGet(0)); // node_idx
    func.instruction(&Instruction::I32Const(8));
    func.instruction(&Instruction::I32Mul);
    func.instruction(&Instruction::I32Const((layout.base_offset + layout.content_table_offset) as i32));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalSet(8));

    // content_len = load_u32(entry_addr + 4)
    func.instruction(&Instruction::LocalGet(8));
    func.instruction(&Instruction::I32Load(MemArg { offset: 4, align: 2, memory_index: 0 }));
    func.instruction(&Instruction::LocalSet(4));

    // content_ptr = base + string_pool_offset + load_u32(entry_addr)
    func.instruction(&Instruction::LocalGet(8));
    func.instruction(&Instruction::I32Load(MemArg { offset: 0, align: 2, memory_index: 0 }));
    func.instruction(&Instruction::I32Const((layout.base_offset + layout.string_pool_offset) as i32));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalSet(3));

    // If content_len < term_len, return 0
    func.instruction(&Instruction::LocalGet(4));
    func.instruction(&Instruction::LocalGet(2));
    func.instruction(&Instruction::I32LtU);
    func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    func.instruction(&Instruction::I32Const(0));
    func.instruction(&Instruction::Return);
    func.instruction(&Instruction::End);

    // Outer loop: i from 0 to content_len - term_len
    func.instruction(&Instruction::I32Const(0));
    func.instruction(&Instruction::LocalSet(5)); // i = 0

    func.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));
    func.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));

    // if i > content_len - term_len, break
    func.instruction(&Instruction::LocalGet(5));
    func.instruction(&Instruction::LocalGet(4)); // content_len
    func.instruction(&Instruction::LocalGet(2)); // term_len
    func.instruction(&Instruction::I32Sub);
    func.instruction(&Instruction::I32Const(1));
    func.instruction(&Instruction::I32Add); // content_len - term_len + 1
    func.instruction(&Instruction::I32GeU);
    func.instruction(&Instruction::BrIf(1));

    // Inner comparison: j = 0, matched = 1
    func.instruction(&Instruction::I32Const(0));
    func.instruction(&Instruction::LocalSet(6));
    func.instruction(&Instruction::I32Const(1));
    func.instruction(&Instruction::LocalSet(7));

    func.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));
    func.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));

    // if j >= term_len, break (all matched)
    func.instruction(&Instruction::LocalGet(6));
    func.instruction(&Instruction::LocalGet(2));
    func.instruction(&Instruction::I32GeU);
    func.instruction(&Instruction::BrIf(1));

    // Compare content[i+j] vs term[j]
    func.instruction(&Instruction::LocalGet(3)); // content_ptr
    func.instruction(&Instruction::LocalGet(5)); // i
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalGet(6)); // j
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::I32Load8U(MemArg { offset: 0, align: 0, memory_index: 0 }));

    func.instruction(&Instruction::LocalGet(1)); // term_ptr
    func.instruction(&Instruction::LocalGet(6)); // j
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::I32Load8U(MemArg { offset: 0, align: 0, memory_index: 0 }));

    func.instruction(&Instruction::I32Ne);
    func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    func.instruction(&Instruction::I32Const(0));
    func.instruction(&Instruction::LocalSet(7)); // matched = 0
    func.instruction(&Instruction::Br(2)); // break inner
    func.instruction(&Instruction::End);

    // j++
    func.instruction(&Instruction::LocalGet(6));
    func.instruction(&Instruction::I32Const(1));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalSet(6));
    func.instruction(&Instruction::Br(0)); // continue inner
    func.instruction(&Instruction::End); // end inner loop
    func.instruction(&Instruction::End); // end inner block

    // If matched, return 1
    func.instruction(&Instruction::LocalGet(7));
    func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    func.instruction(&Instruction::I32Const(1));
    func.instruction(&Instruction::Return);
    func.instruction(&Instruction::End);

    // i++
    func.instruction(&Instruction::LocalGet(5));
    func.instruction(&Instruction::I32Const(1));
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::LocalSet(5));
    func.instruction(&Instruction::Br(0)); // continue outer
    func.instruction(&Instruction::End); // end outer loop
    func.instruction(&Instruction::End); // end outer block

    // Not found
    func.instruction(&Instruction::I32Const(0));
    func.instruction(&Instruction::End);
    func
}

/// Generate _gai_get_rel_type_name(type_idx: i32) -> i64 (fat ptr)
pub fn gen_gai_get_rel_type_name(layout: &GraphLayout) -> Function {
    gen_table_lookup_fat_ptr(layout, layout.rel_type_table_offset)
}

/// Generate cov_alloc(size: i32) -> i32
/// Bump allocator that returns the current heap pointer and advances it by size.
/// Uses global 0 as the heap pointer (convention established in snippet_wasm.rs).
pub fn gen_gai_alloc() -> Function {
    // Param: local 0 = size
    let mut func = Function::new(vec![
        (1, ValType::I32), // local 1: result (original heap pointer)
    ]);

    // result = global[0] (current heap pointer)
    func.instruction(&Instruction::GlobalGet(0));
    func.instruction(&Instruction::LocalSet(1));

    // global[0] = global[0] + size
    func.instruction(&Instruction::GlobalGet(0));
    func.instruction(&Instruction::LocalGet(0)); // size param
    func.instruction(&Instruction::I32Add);
    func.instruction(&Instruction::GlobalSet(0));

    // return result
    func.instruction(&Instruction::LocalGet(1));
    func.instruction(&Instruction::End);
    func
}

/// Generate all GAI function bodies in order
pub fn generate_gai_functions(layout: &GraphLayout) -> Vec<Function> {
    vec![
        gen_gai_node_count(layout),
        gen_gai_get_node_id(layout),
        gen_gai_get_node_kind(layout),
        gen_gai_get_node_content(layout),
        gen_gai_get_outgoing_count(layout),
        gen_gai_get_outgoing_rel(layout),
        gen_gai_get_incoming_count(layout),
        gen_gai_get_incoming_rel(layout),
        gen_gai_find_by_id(layout),
        gen_gai_content_contains(layout),
        gen_gai_get_rel_type_name(layout),
        gen_gai_alloc(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_graph::DataGraph;
    use covenant_ast::*;

    fn make_data_snippet(id: &str, content: &str, relations: Vec<RelationDecl>) -> Snippet {
        let mut sections = Vec::new();
        if !content.is_empty() {
            sections.push(Section::Content(ContentSection {
                content: content.to_string(),
                span: Span::default(),
            }));
        }
        if !relations.is_empty() {
            sections.push(Section::Relations(RelationsSection {
                relations,
                span: Span::default(),
            }));
        }
        Snippet {
            id: id.to_string(),
            kind: SnippetKind::Data,
            notes: vec![],
            sections,
            implements: None,
            platform: None,
            span: Span::default(),
        }
    }

    fn rel_to(target: &str, rel_type: &str) -> RelationDecl {
        RelationDecl {
            kind: RelationKind::To,
            target: target.to_string(),
            rel_type: Some(rel_type.to_string()),
            span: Span::default(),
        }
    }

    #[test]
    fn test_generate_graph_segment_basic() {
        let snippets = vec![
            make_data_snippet("node.a", "Hello world", vec![
                rel_to("node.b", "contains"),
            ]),
            make_data_snippet("node.b", "Child node", vec![]),
        ];

        let graph = DataGraph::from_snippets(&snippets);
        let (data, layout) = generate_graph_segment(&graph, 0);

        // Basic layout checks
        assert_eq!(layout.node_count, 2);
        assert_eq!(layout.base_offset, 0);
        assert!(layout.total_size > 0);
        assert_eq!(data.len(), layout.total_size as usize);

        // Check that string pool contains our strings
        let pool = &data[layout.string_pool_offset as usize..(layout.string_pool_offset + layout.string_pool_size) as usize];
        assert!(std::str::from_utf8(pool).unwrap().contains("node.a"));
        assert!(std::str::from_utf8(pool).unwrap().contains("node.b"));
        assert!(std::str::from_utf8(pool).unwrap().contains("Hello world"));
        assert!(std::str::from_utf8(pool).unwrap().contains("Child node"));
    }

    #[test]
    fn test_generate_graph_segment_with_offset() {
        let snippets = vec![
            make_data_snippet("x", "test", vec![]),
        ];

        let graph = DataGraph::from_snippets(&snippets);
        let (data, layout) = generate_graph_segment(&graph, 1024);

        assert_eq!(layout.base_offset, 1024);
        assert_eq!(layout.node_count, 1);
        assert!(data.len() > 0);
    }

    #[test]
    fn test_gai_function_types_count() {
        let types = gai_function_types();
        assert_eq!(types.len(), GAI_FUNCTION_COUNT as usize);
    }

    #[test]
    fn test_generate_gai_functions_count() {
        let snippets = vec![
            make_data_snippet("a", "content", vec![]),
        ];
        let graph = DataGraph::from_snippets(&snippets);
        let (_, layout) = generate_graph_segment(&graph, 0);
        let funcs = generate_gai_functions(&layout);
        assert_eq!(funcs.len(), GAI_FUNCTION_COUNT as usize);
    }

    #[test]
    fn test_relation_entries_in_segment() {
        let snippets = vec![
            make_data_snippet("a", "Node A", vec![
                rel_to("b", "contains"),
                rel_to("b", "describes"),
            ]),
            make_data_snippet("b", "Node B", vec![]),
        ];

        let graph = DataGraph::from_snippets(&snippets);
        let (data, layout) = generate_graph_segment(&graph, 0);

        // Should have outgoing relations (a->b contains, a->b describes, b->a contained_by, b->a described_by)
        assert!(layout.outgoing_count >= 2);

        // Each relation entry is 4 bytes
        let out_section_size = (layout.outgoing_count * 4) as usize;
        let out_start = layout.outgoing_table_offset as usize;
        assert!(out_start + out_section_size <= data.len());
    }

    #[test]
    fn test_adjacency_index_in_segment() {
        let snippets = vec![
            make_data_snippet("a", "A", vec![rel_to("b", "contains")]),
            make_data_snippet("b", "B", vec![]),
        ];

        let graph = DataGraph::from_snippets(&snippets);
        let (data, layout) = generate_graph_segment(&graph, 0);

        // Adjacency index: 8 bytes per node (4 u16 fields)
        let adj_size = (layout.node_count * 8) as usize;
        let adj_start = layout.adjacency_index_offset as usize;
        assert!(adj_start + adj_size <= data.len());

        // Read adjacency for node 0 (a): should have outgoing
        let _a_out_start = u16::from_le_bytes([data[adj_start], data[adj_start + 1]]);
        let a_out_count = u16::from_le_bytes([data[adj_start + 2], data[adj_start + 3]]);
        assert!(a_out_count >= 1, "Node a should have at least 1 outgoing edge");
    }
}
