//! Covenant Codegen - WASM code generation
//!
//! Compiles pure Covenant functions to WebAssembly.

mod ir;
mod wasm;
mod snippet_wasm;
pub mod data_graph;
pub mod embeddable;
pub mod gai_codegen;

pub use ir::*;
pub use wasm::*;
pub use snippet_wasm::SnippetWasmCompiler;
pub use embeddable::{EmbeddableSymbol, build_embeddable_symbols};

use covenant_ast::Program;
use covenant_checker::SymbolTable;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("cannot compile effectful function '{name}' - only pure functions supported")]
    EffectfulFunction { name: String },

    #[error("unsupported type: {ty}")]
    UnsupportedType { ty: String },

    #[error("unsupported expression")]
    UnsupportedExpression,

    #[error("undefined function: {name}")]
    UndefinedFunction { name: String },

    #[error("serialization failed: {0}")]
    SerializationFailed(String),
}

/// Compile a program to WASM
pub fn compile(program: &Program, symbols: &SymbolTable) -> Result<Vec<u8>, CodegenError> {
    match program {
        Program::Legacy { declarations, .. } => {
            let mut compiler = WasmCompiler::new(symbols);
            compiler.compile_legacy(declarations)
        }
        Program::Snippets { snippets, .. } => {
            let mut compiler = SnippetWasmCompiler::new(symbols);
            compiler.compile_snippets(snippets)
        }
    }
}

/// Compile only pure functions to WASM
pub fn compile_pure(program: &Program, symbols: &SymbolTable) -> Result<Vec<u8>, CodegenError> {
    compile(program, symbols)
}

/// Compile a program to WASM with embedded symbol metadata
///
/// This function builds embeddable symbols from the SymbolGraph and EffectCheckResult,
/// then embeds them as JSON in the WASM data section alongside the normal data graph.
pub fn compile_with_symbols(
    program: &Program,
    symbols: &SymbolTable,
    symbol_graph: &covenant_symbols::SymbolGraph,
    effect_result: &covenant_checker::EffectCheckResult,
) -> Result<Vec<u8>, CodegenError> {
    match program {
        Program::Legacy { declarations, .. } => {
            // Legacy programs don't support symbol embedding
            let mut compiler = WasmCompiler::new(symbols);
            compiler.compile_legacy(declarations)
        }
        Program::Snippets { snippets, .. } => {
            let embeddable = build_embeddable_symbols(symbol_graph, effect_result);
            let mut compiler = SnippetWasmCompiler::new(symbols);
            compiler.compile_snippets_with_symbols(snippets, &embeddable)
        }
    }
}
