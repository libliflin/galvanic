//! AST-to-IR lowering for galvanic.
//!
//! Translates a parsed `SourceFile` into the minimal IR needed for ARM64
//! code generation. Each lowering function corresponds to a FLS section.
//!
//! # FLS traceability
//!
//! - FLS §9: Functions — `lower_fn` maps each `FnDef` to an `IrFn`.
//! - FLS §6.19: Return expressions — tail expressions lower to `Instr::Ret`.
//! - FLS §6.2: Literal expressions — `LitInt` lowers to `IrValue::I32`.
//! - FLS §4.4: Unit type — absent tail / unit type lowers to `IrValue::Unit`.
//! - FLS §18.1: Program structure — `lower` produces one `Module` per file.
//!
//! # Scope (milestone 1)
//!
//! Only the minimum subset needed to lower `fn main() -> i32 { 0 }` is
//! implemented. Each new milestone will extend this pass by exactly what
//! that milestone's target program requires.

use crate::ast::{Block, Expr, ExprKind, ItemKind, SourceFile, TyKind};
use crate::ir::{Instr, IrFn, IrTy, IrValue, Module};

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

// ── Entry point ───────────────────────────────────────────────────────────────

/// Lower a parsed source file to the IR.
///
/// FLS §18.1: A source file is a sequence of items. Each `fn` item is
/// lowered to an `IrFn`. Other item kinds (struct, enum) do not produce
/// code directly and are unsupported at this milestone.
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

// ── Function lowering ─────────────────────────────────────────────────────────

/// Lower a single function definition to an `IrFn`.
///
/// FLS §9: Functions.
fn lower_fn(fn_def: &crate::ast::FnDef, source: &str) -> Result<IrFn, LowerError> {
    let name = fn_def.name.text(source).to_owned();

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
        Some(block) => lower_block_return(block, &ret_ty)?,
    };

    Ok(IrFn { name, ret_ty, body })
}

// ── Type lowering ─────────────────────────────────────────────────────────────

/// Lower a type expression to an `IrTy`.
///
/// FLS §4: Types. Only `i32` and `()` are supported at this milestone.
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

// ── Block / expression lowering ───────────────────────────────────────────────

/// Lower a function body block to a list of IR instructions.
///
/// For milestone 1 the block must have no statements — only a tail
/// expression (or no tail, for unit-returning functions). The tail lowers
/// to the single `Ret` instruction.
///
/// FLS §6.4: Block expressions. FLS §6.19: Return expressions.
fn lower_block_return(block: &Block, ret_ty: &IrTy) -> Result<Vec<Instr>, LowerError> {
    if !block.stmts.is_empty() {
        return Err(LowerError::Unsupported(
            "statements in function body".into(),
        ));
    }

    let value = match &block.tail {
        None => IrValue::Unit,
        Some(expr) => lower_expr(expr, ret_ty)?,
    };

    Ok(vec![Instr::Ret(value)])
}

/// Lower an expression to an `IrValue`.
///
/// Only constant integer and unit literals are supported at this milestone.
///
/// FLS §6.2: Literal expressions.
fn lower_expr(expr: &Expr, expected_ty: &IrTy) -> Result<IrValue, LowerError> {
    match (&expr.kind, expected_ty) {
        // FLS §2.4.4.1: Integer literal narrowed to i32.
        (ExprKind::LitInt(n), IrTy::I32) => {
            // FLS §2.4.4.1: The value must fit in the target type.
            if *n > i32::MAX as u128 {
                return Err(LowerError::Unsupported(format!(
                    "integer literal {n} out of range for i32"
                )));
            }
            Ok(IrValue::I32(*n as i32))
        }

        // FLS §4.4: Unit literal `()`.
        (ExprKind::Unit, IrTy::Unit) => Ok(IrValue::Unit),

        // Any other combination is not yet supported.
        _ => Err(LowerError::Unsupported("expression".into())),
    }
}
