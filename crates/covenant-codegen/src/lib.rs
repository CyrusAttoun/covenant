//! Covenant Codegen - WASM code generation
//!
//! Compiles pure Covenant functions to WebAssembly.

mod ir;
mod wasm;
mod snippet_wasm;

pub use ir::*;
pub use wasm::*;
pub use snippet_wasm::SnippetWasmCompiler;

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
