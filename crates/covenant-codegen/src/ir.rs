//! Intermediate representation for codegen

use covenant_ast::BinaryOp;

/// Intermediate representation instruction
#[derive(Debug, Clone)]
pub enum IrInst {
    /// Push integer constant
    I64Const(i64),
    /// Push float constant
    F64Const(f64),
    /// Load local variable
    LocalGet(u32),
    /// Store to local variable
    LocalSet(u32),
    /// Binary operation on i64
    I64BinOp(IrBinOp),
    /// Binary operation on f64
    F64BinOp(IrBinOp),
    /// Comparison on i64
    I64Compare(IrCmpOp),
    /// Comparison on f64
    F64Compare(IrCmpOp),
    /// Call function by index
    Call(u32),
    /// Return from function
    Return,
    /// If-then-else
    If {
        then_branch: Vec<IrInst>,
        else_branch: Vec<IrInst>,
    },
    /// Loop
    Loop(Vec<IrInst>),
    /// Branch (break)
    Br(u32),
    /// Conditional branch
    BrIf(u32),
    /// Drop value from stack
    Drop,
}

/// Binary operations
#[derive(Debug, Clone, Copy)]
pub enum IrBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

/// Comparison operations
#[derive(Debug, Clone, Copy)]
pub enum IrCmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl IrBinOp {
    pub fn from_binary_op(op: BinaryOp) -> Option<Self> {
        match op {
            BinaryOp::Add => Some(IrBinOp::Add),
            BinaryOp::Sub => Some(IrBinOp::Sub),
            BinaryOp::Mul => Some(IrBinOp::Mul),
            BinaryOp::Div => Some(IrBinOp::Div),
            BinaryOp::Mod => Some(IrBinOp::Rem),
            _ => None,
        }
    }
}

impl IrCmpOp {
    pub fn from_binary_op(op: BinaryOp) -> Option<Self> {
        match op {
            BinaryOp::Eq => Some(IrCmpOp::Eq),
            BinaryOp::Ne => Some(IrCmpOp::Ne),
            BinaryOp::Lt => Some(IrCmpOp::Lt),
            BinaryOp::Le => Some(IrCmpOp::Le),
            BinaryOp::Gt => Some(IrCmpOp::Gt),
            BinaryOp::Ge => Some(IrCmpOp::Ge),
            _ => None,
        }
    }
}

/// A compiled function in IR form
#[derive(Debug)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<IrType>,
    pub results: Vec<IrType>,
    pub locals: Vec<IrType>,
    pub body: Vec<IrInst>,
    pub export: bool,
}

/// IR type (subset of WASM types)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrType {
    I64,
    F64,
    I32, // For booleans
}
