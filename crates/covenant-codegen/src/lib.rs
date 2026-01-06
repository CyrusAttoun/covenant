//! Covenant Codegen - WASM code generation
//!
//! Compiles pure Covenant functions to WebAssembly.

mod ir;
mod wasm;

pub use ir::*;
pub use wasm::*;

use covenant_ast::*;
use covenant_checker::{SymbolTable, SymbolKind, ResolvedType};
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
    let mut compiler = WasmCompiler::new(symbols);
    compiler.compile(program)
}

/// Compile only pure functions to WASM
pub fn compile_pure(program: &Program, symbols: &SymbolTable) -> Result<Vec<u8>, CodegenError> {
    let mut compiler = WasmCompiler::new(symbols);
    compiler.compile_pure_only(program)
}
