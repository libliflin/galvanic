//! AST-to-IR lowering for galvanic.
//!
//! Translates a parsed `SourceFile` into the minimal IR needed for ARM64
//! code generation. Each lowering function corresponds to a FLS section.
//!
//! # FLS constraint compliance (fls-constraints.md)
//!
//! This module emits **runtime instructions** for all non-const code.
//! Compile-time evaluation is only permitted in const contexts (FLS
//! §6.1.2:37–45). Since galvanic does not yet support `const` items,
//! ALL code paths emit runtime IR — no interpreter, no constant folding.
//!
//! The litmus test: if replacing a literal with a function parameter would
//! break the implementation, it's an interpreter, not a compiler.
//!
//! # FLS traceability
//!
//! - FLS §9: Functions — `lower_fn` maps each `FnDef` to an `IrFn`.
//! - FLS §6.19: Return expressions — tail expressions lower to `Instr::Ret`.
//! - FLS §2.4.4.1: Integer literal expressions — `LoadImm` materializes them.
//! - FLS §4.4: Unit type — absent tail / unit type lowers to `IrValue::Unit`.
//! - FLS §6.5.5: Arithmetic operators — `BinOp` instructions for +, -, *.
//! - FLS §6.1.2:37–45: Non-const code emits runtime instructions.
//! - FLS §18.1: Program structure — `lower` produces one `Module` per file.

use crate::ast::{BinOp, Expr, ExprKind, ItemKind, SourceFile, TyKind};
use crate::ir::{IrBinOp, Instr, IrFn, IrTy, IrValue, Module};

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors that can occur during lowering.
#[derive(Debug)]
pub enum LowerError {
    /// A language feature used by the program is not yet implemented.
    Unsupported(String),
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LowerError::Unsupported(msg) => write!(f, "not yet supported: {msg}"),
        }
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

/// Lower a parsed source file to the IR.
///
/// FLS §18.1: A source file is a sequence of items. Each `fn` item is
/// lowered to an `IrFn`. Other item kinds are unsupported.
pub fn lower(src: &SourceFile, source: &str) -> Result<Module, LowerError> {
    let mut fns = Vec::new();

    for item in &src.items {
        match &item.kind {
            ItemKind::Fn(fn_def) => {
                fns.push(lower_fn(fn_def, source)?);
            }
            ItemKind::Struct(_) | ItemKind::Enum(_) => {
                return Err(LowerError::Unsupported(
                    "struct/enum items".into(),
                ));
            }
        }
    }

    Ok(Module { fns })
}

// ── Function lowering ────────────────────────────────────────────────────────

/// Lower a single function definition to an `IrFn`.
///
/// FLS §9: Functions.
fn lower_fn(fn_def: &crate::ast::FnDef, source: &str) -> Result<IrFn, LowerError> {
    let name = fn_def.name.text(source).to_owned();

    // Functions with parameters require runtime stack frames / registers
    // for parameter passing. Not yet implemented.
    if !fn_def.params.is_empty() {
        return Err(LowerError::Unsupported(
            "functions with parameters (runtime parameter passing not yet implemented)".into(),
        ));
    }

    // FLS §9: "If no return type is specified, the return type is `()`."
    let ret_ty = match &fn_def.ret_ty {
        None => IrTy::Unit,
        Some(ty) => lower_ty(ty, source)?,
    };

    let body = match &fn_def.body {
        None => {
            return Err(LowerError::Unsupported(
                "extern / bodyless functions".into(),
            ));
        }
        Some(block) => lower_block_return(block, &ret_ty, source)?,
    };

    Ok(IrFn { name, ret_ty, body })
}

// ── Type lowering ────────────────────────────────────────────────────────────

/// Lower a type expression to an `IrTy`.
///
/// FLS §4: Types. Only `i32` and `()` are supported.
fn lower_ty(ty: &crate::ast::Ty, source: &str) -> Result<IrTy, LowerError> {
    match &ty.kind {
        TyKind::Unit => Ok(IrTy::Unit),
        TyKind::Path(segments) if segments.len() == 1 => {
            match segments[0].text(source) {
                "i32" => Ok(IrTy::I32),
                name => Err(LowerError::Unsupported(format!("type `{name}`"))),
            }
        }
        _ => Err(LowerError::Unsupported("complex type".into())),
    }
}

// ── Block / expression lowering ──────────────────────────────────────────────

/// Lower a function body block to a list of IR instructions.
///
/// Emits real runtime instructions. No compile-time evaluation.
///
/// FLS §6.4: Block expressions.
/// FLS §6.19: Return expressions — the tail is the block's return value.
/// FLS §6.1.2:37–45: Non-const function bodies must emit runtime code.
fn lower_block_return(
    block: &crate::ast::Block,
    ret_ty: &IrTy,
    source: &str,
) -> Result<Vec<Instr>, LowerError> {
    // Statements require runtime stack allocation, variable storage, and
    // control flow — none of which are implemented yet.
    if !block.stmts.is_empty() {
        return Err(LowerError::Unsupported(
            "statements in function body (runtime variable storage not yet implemented)".into(),
        ));
    }

    match &block.tail {
        // No tail expression: unit return.
        None => Ok(vec![Instr::Ret(IrValue::Unit)]),

        Some(tail) => {
            let mut instrs = Vec::new();
            let mut next_reg: u8 = 0;
            let result = lower_tail_expr(tail, &mut instrs, &mut next_reg, ret_ty, source)?;
            instrs.push(Instr::Ret(result));
            Ok(instrs)
        }
    }
}

/// Lower a tail expression to runtime IR instructions.
///
/// Returns the IrValue holding the result. Emits LoadImm/BinOp
/// instructions into `instrs` as needed.
///
/// FLS §6.1.2:37–45: All code here emits runtime instructions.
fn lower_tail_expr(
    expr: &Expr,
    instrs: &mut Vec<Instr>,
    next_reg: &mut u8,
    ret_ty: &IrTy,
    _source: &str,
) -> Result<IrValue, LowerError> {
    match (&expr.kind, ret_ty) {
        // FLS §2.4.4.1: Integer literal — materialize as a runtime immediate.
        (ExprKind::LitInt(n), IrTy::I32) => {
            if *n > i32::MAX as u128 {
                return Err(LowerError::Unsupported(format!(
                    "integer literal {n} out of range for i32"
                )));
            }
            let n = *n as i32;
            let r = alloc_reg(next_reg)?;
            instrs.push(Instr::LoadImm(r, n));
            Ok(IrValue::Reg(r))
        }

        // FLS §4.4: Unit literal `()`.
        (ExprKind::Unit, IrTy::Unit) => Ok(IrValue::Unit),

        // FLS §6.5.5: Arithmetic binary operations — emit runtime instructions.
        //
        // Both operands are lowered recursively, producing LoadImm/BinOp
        // instructions. The result is in a virtual register.
        (ExprKind::Binary { op, lhs, rhs }, IrTy::I32)
            if matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul) =>
        {
            let lhs_val = lower_tail_expr(lhs, instrs, next_reg, ret_ty, _source)?;
            let rhs_val = lower_tail_expr(rhs, instrs, next_reg, ret_ty, _source)?;

            let lhs_reg = val_to_reg(lhs_val, instrs, next_reg)?;
            let rhs_reg = val_to_reg(rhs_val, instrs, next_reg)?;

            let dst = alloc_reg(next_reg)?;
            let ir_op = match op {
                BinOp::Add => IrBinOp::Add,
                BinOp::Sub => IrBinOp::Sub,
                BinOp::Mul => IrBinOp::Mul,
                _ => unreachable!("matched above"),
            };
            instrs.push(Instr::BinOp { op: ir_op, dst, lhs: lhs_reg, rhs: rhs_reg });
            Ok(IrValue::Reg(dst))
        }

        // Anything else: not yet supported as runtime codegen.
        _ => Err(LowerError::Unsupported(
            "expression kind in non-const context (runtime codegen not yet implemented)".into(),
        )),
    }
}

/// Allocate the next virtual register.
fn alloc_reg(next_reg: &mut u8) -> Result<u8, LowerError> {
    let r = *next_reg;
    *next_reg = next_reg.checked_add(1).ok_or_else(|| {
        LowerError::Unsupported("exceeded 256 virtual registers".into())
    })?;
    Ok(r)
}

/// Ensure a value is in a register. If it already is, return the register.
/// If it's a constant, emit a LoadImm to put it in one.
fn val_to_reg(val: IrValue, instrs: &mut Vec<Instr>, next_reg: &mut u8) -> Result<u8, LowerError> {
    match val {
        IrValue::Reg(r) => Ok(r),
        IrValue::I32(n) => {
            let r = alloc_reg(next_reg)?;
            instrs.push(Instr::LoadImm(r, n));
            Ok(r)
        }
        IrValue::Unit => Err(LowerError::Unsupported(
            "unit value used as arithmetic operand".into(),
        )),
    }
}
