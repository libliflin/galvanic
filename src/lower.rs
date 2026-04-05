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
//! - FLS §8.1: Let statements — processed into a constant environment.
//! - FLS §6.3: Path expressions — variable references resolved from env.
//! - FLS §18.1: Program structure — `lower` produces one `Module` per file.
//!
//! # Scope (milestone 3)
//!
//! Extends milestone 2 to support `let` bindings and simple variable
//! references. The approach remains pure constant-folding: let initializers
//! must evaluate to a compile-time constant, and path expressions substitute
//! the bound constant. No stack allocation or virtual registers are needed.

use std::collections::HashMap;

use crate::ast::{BinOp, Block, Expr, ExprKind, ItemKind, SourceFile, StmtKind, TyKind};
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
        Some(block) => lower_block_return(block, &ret_ty, source)?,
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
/// Processes `let` statements in order, building a constant environment, then
/// lowers the tail expression (or unit) to the single `Ret` instruction.
///
/// FLS §6.4: Block expressions.
/// FLS §8.1: Let statements — each `let x = expr;` binds `x` to the
///   constant-folded value of `expr` in the local environment.
/// FLS §6.19: Return expressions — the tail is the block's return value.
fn lower_block_return(
    block: &Block,
    ret_ty: &IrTy,
    source: &str,
) -> Result<Vec<Instr>, LowerError> {
    // FLS §8.1: Build a constant environment from let statements.
    // Maps each bound identifier (source text) to its compile-time constant.
    let mut env: HashMap<String, IrValue> = HashMap::new();

    for stmt in &block.stmts {
        match &stmt.kind {
            // FLS §8.1: Let statement with initializer.
            StmtKind::Let { name, init, .. } => {
                // A let without an initializer produces an uninitialized place.
                // We cannot constant-fold an uninitialized value; reject it.
                // (Use-before-init is caught by the borrow checker in full Rust.)
                let init_expr = init.as_ref().ok_or_else(|| {
                    LowerError::Unsupported("let binding without initializer".into())
                })?;
                let val = lower_expr(init_expr, ret_ty, source, &env)?;
                let binding_name = name.text(source).to_owned();
                env.insert(binding_name, val);
            }
            StmtKind::Expr(_) | StmtKind::Empty => {
                return Err(LowerError::Unsupported(
                    "expression statements in function body".into(),
                ));
            }
        }
    }

    let value = match &block.tail {
        None => IrValue::Unit,
        Some(expr) => lower_expr(expr, ret_ty, source, &env)?,
    };

    Ok(vec![Instr::Ret(value)])
}

/// Lower an expression to an `IrValue`.
///
/// `env` maps in-scope let bindings to their compile-time constant values.
///
/// FLS §6.2: Literal expressions.
/// FLS §6.3: Path expressions — single-segment paths resolved from `env`.
/// FLS §6.5: Arithmetic operator expressions.
fn lower_expr(
    expr: &Expr,
    expected_ty: &IrTy,
    source: &str,
    env: &HashMap<String, IrValue>,
) -> Result<IrValue, LowerError> {
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

        // FLS §6.3: Path expression — look up a single-segment variable name.
        //
        // Multi-segment paths (e.g. `std::i32::MAX`) are not yet supported;
        // they require name resolution and are deferred to a later milestone.
        (ExprKind::Path(segments), _) if segments.len() == 1 => {
            let name = segments[0].text(source);
            env.get(name).copied().ok_or_else(|| {
                LowerError::Unsupported(format!(
                    "variable `{name}` not found (uninitialized, out of scope, or not a compile-time constant)"
                ))
            })
        }

        // FLS §6.5: Arithmetic binary operations on constant i32 operands.
        //
        // Both sub-expressions are lowered first; if they both reduce to
        // `IrValue::I32` the operation is folded at compile time. This covers
        // the milestone-2 target `fn main() -> i32 { 1 + 2 }`, and now also
        // expressions involving let-bound variables.
        //
        // FLS §6.23: Arithmetic overflow. The FLS states overflow behaviour is
        // implementation-defined. We use wrapping semantics here (matching
        // rustc's debug-mode behaviour) and document that choice.
        (ExprKind::Binary { op, lhs, rhs }, IrTy::I32) => {
            let lhs_val = lower_expr(lhs, expected_ty, source, env)?;
            let rhs_val = lower_expr(rhs, expected_ty, source, env)?;
            match (op, lhs_val, rhs_val) {
                (BinOp::Add, IrValue::I32(a), IrValue::I32(b)) => {
                    // FLS §6.5.5: Addition operator `+`.
                    Ok(IrValue::I32(a.wrapping_add(b)))
                }
                (BinOp::Sub, IrValue::I32(a), IrValue::I32(b)) => {
                    // FLS §6.5.5: Subtraction operator `-`.
                    Ok(IrValue::I32(a.wrapping_sub(b)))
                }
                (BinOp::Mul, IrValue::I32(a), IrValue::I32(b)) => {
                    // FLS §6.5.5: Multiplication operator `*`.
                    Ok(IrValue::I32(a.wrapping_mul(b)))
                }
                _ => Err(LowerError::Unsupported(
                    "non-constant or unsupported binary expression".into(),
                )),
            }
        }

        // Any other combination is not yet supported.
        _ => Err(LowerError::Unsupported("expression".into())),
    }
}
