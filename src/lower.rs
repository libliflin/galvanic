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
//! - FLS §8.1: Let statements — `lower_stmt` allocates a stack slot and stores.
//! - FLS §6.3: Path expressions — local variable references load from stack.
//! - FLS §6.19: Return expressions — tail expressions lower to `Instr::Ret`.
//! - FLS §2.4.4.1: Integer literal expressions — `LoadImm` materializes them.
//! - FLS §4.4: Unit type — absent tail / unit type lowers to `IrValue::Unit`.
//! - FLS §6.5.5: Arithmetic operators — `BinOp` instructions for +, -, *.
//! - FLS §6.1.2:37–45: Non-const code emits runtime instructions.
//! - FLS §18.1: Program structure — `lower` produces one `Module` per file.

use std::collections::HashMap;

use crate::ast::{BinOp, Block, Expr, ExprKind, ItemKind, ParamKind, Pat, SelfKind, SourceFile, Stmt, StmtKind, StructKind, TyKind};
use crate::ir::{F32BinOp, F64BinOp, FCmpOp, IrBinOp, Instr, IrFn, IrTy, IrValue, Module};

/// Enum variant registry: maps enum name → (variant name → (discriminant, field_names)).
///
/// FLS §15: Enumerations. Each variant is assigned an integer discriminant
/// starting at 0.
///
/// `field_names` is the ordered list of field names:
/// - Unit variants: empty `vec![]`.
/// - Tuple variants: positional placeholders `vec![""; N]` (names unused).
/// - Named-field variants: actual field names in declaration order.
///
/// `field_names.len()` is the field count for all variant kinds.
/// Named variants use the field names to map construction / pattern fields to
/// the correct stack slot (`base + 1 + declaration_index`).
///
/// Cache-line note: `Vec<String>` adds one heap pointer per variant entry.
/// At this milestone the registry is build-time only and not on a hot path.
type EnumVariantInfo = (i32, Vec<String>);
type EnumDefs = HashMap<String, HashMap<String, EnumVariantInfo>>;

// ── FLS citations added in this module ───────────────────────────────────────
// FLS §6.12.1: Call expressions — `lower_expr` handles `ExprKind::Call`.
// FLS §9: Functions with parameters — `lower_fn` spills x0..x{n-1} to stack.
// FLS §6.15.2: Infinite loop expressions — `lower_expr` handles `ExprKind::Loop`.
// FLS §6.15.3: While loop expressions — `lower_expr` handles `ExprKind::While`.
// FLS §6.15.5: Continue expressions — `lower_expr` handles `ExprKind::Continue`.
// FLS §6.15.6: Break expressions — `lower_expr` handles `ExprKind::Break`.
// FLS §6.5.3: Comparison operator expressions — `lower_expr` handles Lt/Le/Gt/Ge/Eq/Ne.
// FLS §6.5.6: Bit operator expressions — `lower_expr` handles BitAnd/BitOr/BitXor.
// FLS §6.5.7: Shift operator expressions — `lower_expr` handles Shl/Shr.
// FLS §6.18: Match expressions — `lower_expr` handles `ExprKind::Match`.

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

// ── Call-detection helpers ────────────────────────────────────────────────────

/// Return `true` if the expression tree contains at least one `Call` node.
///
/// Used during lowering to determine whether an intermediate register value
/// needs to be spilled to a stack slot before lowering the other sub-expression.
/// ARM64 calling convention: `x0`–`x17` are caller-saved — any `bl` instruction
/// in the RHS of a binary expression will clobber a live register holding the
/// LHS result. Spilling prevents this.
///
/// For example, `fib(n-1) + fib(n-2)`: the first call puts its result in some
/// register `r`. The second call (`bl fib`) re-uses that same register range
/// internally, overwriting `r`. Without a spill, the add would use the wrong
/// value for the LHS.
///
/// FLS §6.12.1: Call expressions invoke a function at runtime and follow the
/// ARM64 calling convention (caller-saved: x0–x17).
fn expr_contains_call(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Call { .. } | ExprKind::MethodCall { .. } => true,
        ExprKind::Binary { lhs, rhs, .. } => expr_contains_call(lhs) || expr_contains_call(rhs),
        ExprKind::Unary { operand, .. } => expr_contains_call(operand),
        ExprKind::Cast { expr: inner, .. } => expr_contains_call(inner),
        ExprKind::CompoundAssign { target, value, .. } => {
            expr_contains_call(target) || expr_contains_call(value)
        }
        ExprKind::Block(block) => block_contains_call(block),
        ExprKind::If { cond, then_block, else_expr } => {
            expr_contains_call(cond)
                || block_contains_call(then_block)
                || else_expr.as_ref().is_some_and(|e| expr_contains_call(e))
        }
        ExprKind::IfLet { scrutinee, then_block, else_expr, .. } => {
            expr_contains_call(scrutinee)
                || block_contains_call(then_block)
                || else_expr.as_ref().is_some_and(|e| expr_contains_call(e))
        }
        ExprKind::While { cond, body, .. } => {
            expr_contains_call(cond) || block_contains_call(body)
        }
        ExprKind::WhileLet { scrutinee, body, .. } => {
            expr_contains_call(scrutinee) || block_contains_call(body)
        }
        ExprKind::Loop { body, .. } => block_contains_call(body),
        ExprKind::Break { value: opt_val, .. } => opt_val.as_ref().is_some_and(|e| expr_contains_call(e)),
        ExprKind::Return(opt_val) => opt_val.as_ref().is_some_and(|e| expr_contains_call(e)),
        ExprKind::Range { start, end, .. } => {
            start.as_ref().is_some_and(|e| expr_contains_call(e))
                || end.as_ref().is_some_and(|e| expr_contains_call(e))
        }
        ExprKind::For { iter, body, .. } => {
            expr_contains_call(iter) || block_contains_call(body)
        }
        ExprKind::Match { scrutinee, arms } => {
            expr_contains_call(scrutinee)
                || arms.iter().any(|a| expr_contains_call(&a.body))
        }
        ExprKind::StructLit { fields, base, .. } => {
            fields.iter().any(|(_, v)| expr_contains_call(v))
                || base.as_ref().is_some_and(|b| expr_contains_call(b))
        }
        ExprKind::EnumVariantLit { fields, .. } => {
            fields.iter().any(|(_, v)| expr_contains_call(v))
        }
        ExprKind::FieldAccess { receiver, .. } => expr_contains_call(receiver),
        ExprKind::Array(elems) => elems.iter().any(expr_contains_call),
        ExprKind::ArrayRepeat { value, count } => expr_contains_call(value) || expr_contains_call(count),
        ExprKind::Tuple(elems) => elems.iter().any(expr_contains_call),
        ExprKind::Index { base, index } => expr_contains_call(base) || expr_contains_call(index),
        // FLS §6.14: A closure expression itself does not call anything
        // at the point where it appears — it defines a function and
        // materialises its address. The body runs only when the closure
        // is invoked, not when the closure expression is evaluated.
        ExprKind::Closure { .. } => false,
        // Leaves: literals, paths, unit — none contain calls.
        _ => false,
    }
}

/// Return `true` if any statement or tail expression in the block contains a call.
///
/// FLS §6.4: Block expressions are sequences of statements followed by an
/// optional tail expression. A call anywhere in the sequence makes the block
/// "call-containing" for register-spill purposes.
fn block_contains_call(block: &Block) -> bool {
    block.stmts.iter().any(stmt_contains_call)
        || block.tail.as_ref().is_some_and(|e| expr_contains_call(e))
}

fn stmt_contains_call(stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Expr(e) => expr_contains_call(e),
        StmtKind::Let { init, .. } => init.as_ref().is_some_and(|e| expr_contains_call(e)),
        StmtKind::Empty => false,
        // FLS §3, §9: Inner item definitions are not expressions; they cannot
        // themselves contain a runtime call in the enclosing scope.
        StmtKind::Item(_) => false,
    }
}

// ── Break-with-value detection ───────────────────────────────────────────────

/// Return `true` if the block contains a `break <value>` expression that
/// belongs to the *current* loop level (not a nested loop).
///
/// FLS §6.15.6: Only `loop` expressions support break-with-value; `while`
/// and `for` loops do not yield a value via `break`.
fn block_contains_break_with_value(block: &Block) -> bool {
    block.stmts.iter().any(stmt_contains_break_with_value)
        || block.tail.as_ref().is_some_and(|e| expr_contains_break_with_value(e))
}

fn stmt_contains_break_with_value(stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Expr(e) => expr_contains_break_with_value(e),
        StmtKind::Let { init, .. } => {
            init.as_ref().is_some_and(|e| expr_contains_break_with_value(e))
        }
        StmtKind::Empty => false,
        StmtKind::Item(_) => false,
    }
}

/// Return `true` if the expression contains a `break <value>` at the current
/// loop level. Does **not** recurse into nested loop bodies, because `break`
/// statements inside nested loops belong to those loops, not the outer one.
fn expr_contains_break_with_value(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Break { value: Some(_), .. } => true,
        // Recurse into non-loop control-flow.
        ExprKind::If { cond, then_block, else_expr } => {
            expr_contains_break_with_value(cond)
                || block_contains_break_with_value(then_block)
                || else_expr.as_ref().is_some_and(|e| expr_contains_break_with_value(e))
        }
        ExprKind::IfLet { scrutinee, then_block, else_expr, .. } => {
            expr_contains_break_with_value(scrutinee)
                || block_contains_break_with_value(then_block)
                || else_expr.as_ref().is_some_and(|e| expr_contains_break_with_value(e))
        }
        ExprKind::Block(b) => block_contains_break_with_value(b),
        ExprKind::Match { scrutinee, arms } => {
            expr_contains_break_with_value(scrutinee)
                || arms.iter().any(|a| expr_contains_break_with_value(&a.body))
        }
        ExprKind::StructLit { fields, base, .. } => {
            fields.iter().any(|(_, v)| expr_contains_break_with_value(v))
                || base.as_ref().is_some_and(|b| expr_contains_break_with_value(b))
        }
        ExprKind::EnumVariantLit { fields, .. } => {
            fields.iter().any(|(_, v)| expr_contains_break_with_value(v))
        }
        ExprKind::FieldAccess { receiver, .. } => expr_contains_break_with_value(receiver),
        ExprKind::Array(elems) => elems.iter().any(expr_contains_break_with_value),
        ExprKind::ArrayRepeat { value, count } => {
            expr_contains_break_with_value(value) || expr_contains_break_with_value(count)
        }
        ExprKind::Tuple(elems) => elems.iter().any(expr_contains_break_with_value),
        ExprKind::Index { base, index } => {
            expr_contains_break_with_value(base) || expr_contains_break_with_value(index)
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            expr_contains_break_with_value(receiver)
                || args.iter().any(expr_contains_break_with_value)
        }
        // Do NOT recurse into nested loops — their `break` belongs to them.
        ExprKind::Loop { .. } | ExprKind::While { .. } | ExprKind::WhileLet { .. } | ExprKind::For { .. } => false,
        // A closure body is a separate function — its `break` expressions belong to loops
        // inside the closure, not to any enclosing loop of the closure expression itself.
        ExprKind::Closure { .. } => false,
        _ => false,
    }
}

/// Return `true` if the block contains a `break 'target_label <value>`
/// expression at *any* nesting depth.
///
/// FLS §6.15.6: A labeled break exits the loop identified by the label,
/// regardless of how many inner loops are between the break and its target.
/// We must recurse into nested loops to find such breaks — but stop at any
/// loop whose own label matches `target_label`, since that would shadow it.
fn block_contains_labeled_break_with_value(block: &Block, target_label: &str) -> bool {
    block
        .stmts
        .iter()
        .any(|s| stmt_contains_labeled_break_with_value(s, target_label))
        || block
            .tail
            .as_ref()
            .is_some_and(|e| expr_contains_labeled_break_with_value(e, target_label))
}

fn stmt_contains_labeled_break_with_value(stmt: &Stmt, target_label: &str) -> bool {
    match &stmt.kind {
        StmtKind::Expr(e) => expr_contains_labeled_break_with_value(e, target_label),
        StmtKind::Let { init, .. } => {
            init.as_ref()
                .is_some_and(|e| expr_contains_labeled_break_with_value(e, target_label))
        }
        StmtKind::Empty => false,
        StmtKind::Item(_) => false,
    }
}

/// Return `true` if `expr` contains `break 'target_label <value>` at any depth,
/// recursing into nested loops (but not into loops that shadow the label).
fn expr_contains_labeled_break_with_value(expr: &Expr, target_label: &str) -> bool {
    match &expr.kind {
        ExprKind::Break { label: Some(lbl), value: Some(_) } if lbl.as_str() == target_label => {
            true
        }
        ExprKind::If { cond, then_block, else_expr } => {
            expr_contains_labeled_break_with_value(cond, target_label)
                || block_contains_labeled_break_with_value(then_block, target_label)
                || else_expr
                    .as_ref()
                    .is_some_and(|e| expr_contains_labeled_break_with_value(e, target_label))
        }
        ExprKind::IfLet { scrutinee, then_block, else_expr, .. } => {
            expr_contains_labeled_break_with_value(scrutinee, target_label)
                || block_contains_labeled_break_with_value(then_block, target_label)
                || else_expr
                    .as_ref()
                    .is_some_and(|e| expr_contains_labeled_break_with_value(e, target_label))
        }
        ExprKind::Block(b) => block_contains_labeled_break_with_value(b, target_label),
        ExprKind::Match { scrutinee, arms } => {
            expr_contains_labeled_break_with_value(scrutinee, target_label)
                || arms
                    .iter()
                    .any(|a| expr_contains_labeled_break_with_value(&a.body, target_label))
        }
        ExprKind::StructLit { fields, base, .. } => {
            fields
                .iter()
                .any(|(_, v)| expr_contains_labeled_break_with_value(v, target_label))
                || base
                    .as_ref()
                    .is_some_and(|b| expr_contains_labeled_break_with_value(b, target_label))
        }
        ExprKind::EnumVariantLit { fields, .. } => fields
            .iter()
            .any(|(_, v)| expr_contains_labeled_break_with_value(v, target_label)),
        ExprKind::FieldAccess { receiver, .. } => {
            expr_contains_labeled_break_with_value(receiver, target_label)
        }
        ExprKind::Array(elems) => elems
            .iter()
            .any(|e| expr_contains_labeled_break_with_value(e, target_label)),
        ExprKind::ArrayRepeat { value, count } => {
            expr_contains_labeled_break_with_value(value, target_label)
                || expr_contains_labeled_break_with_value(count, target_label)
        }
        ExprKind::Tuple(elems) => elems
            .iter()
            .any(|e| expr_contains_labeled_break_with_value(e, target_label)),
        ExprKind::Index { base, index } => {
            expr_contains_labeled_break_with_value(base, target_label)
                || expr_contains_labeled_break_with_value(index, target_label)
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            expr_contains_labeled_break_with_value(receiver, target_label)
                || args
                    .iter()
                    .any(|a| expr_contains_labeled_break_with_value(a, target_label))
        }
        // Recurse into nested loops, but stop if this loop's own label shadows the target.
        ExprKind::Loop { body, label } => {
            if label.as_deref() == Some(target_label) {
                false
            } else {
                block_contains_labeled_break_with_value(body, target_label)
            }
        }
        ExprKind::While { body, label, .. } => {
            if label.as_deref() == Some(target_label) {
                false
            } else {
                block_contains_labeled_break_with_value(body, target_label)
            }
        }
        ExprKind::WhileLet { body, label, .. } => {
            if label.as_deref() == Some(target_label) {
                false
            } else {
                block_contains_labeled_break_with_value(body, target_label)
            }
        }
        ExprKind::For { body, label, .. } => {
            if label.as_deref() == Some(target_label) {
                false
            } else {
                block_contains_labeled_break_with_value(body, target_label)
            }
        }
        // Closures are separate functions — their breaks don't target outer loops.
        ExprKind::Closure { .. } => false,
        _ => false,
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

/// Lower a parsed source file to the IR.
///
/// FLS §18.1: A source file is a sequence of items. Each `fn` item is
/// lowered to an `IrFn`. Struct items (FLS §14) are collected into a
/// Evaluate a constant expression to an `i32` at compile time.
///
/// FLS §6.1.2:37–45: Constant initializers are evaluated at compile time.
/// Supported forms:
/// - Integer literals (FLS §2.4.4.1)
/// - Arithmetic negation `-expr` (FLS §6.5.3)
/// - Binary arithmetic: `+`, `-`, `*`, `/`, `%` (FLS §6.5.5)
/// - Bitwise: `&`, `|`, `^`, `<<`, `>>` (FLS §6.5.6, §6.5.7)
/// - References to already-known const names (FLS §7.1:10)
/// - Calls to `const fn` items with i32 arguments (FLS §9:41–43)
///
/// Returns `None` for unsupported forms (non-integer types, overflow, or
/// division/remainder by zero). The caller silently skips const items whose
/// initializers cannot be evaluated.
///
/// `const_fns` maps function names to their `FnDef`s for const fn calls.
///
/// Cache-line note: called only during the compile-time const collection pass,
/// not on any runtime hot path.
fn eval_const_expr(
    expr: &Expr,
    source: &str,
    known: &HashMap<String, i32>,
    const_fns: &HashMap<String, &crate::ast::FnDef>,
) -> Option<i32> {
    match &expr.kind {
        // FLS §2.4.4.1: Integer literal — must fit in i32.
        ExprKind::LitInt(n) => {
            if *n <= i32::MAX as u128 { Some(*n as i32) } else { None }
        }
        // FLS §6.5.3: Arithmetic negation in const context.
        ExprKind::Unary { op: crate::ast::UnaryOp::Neg, operand } => {
            eval_const_expr(operand, source, known, const_fns)?.checked_neg()
        }
        // FLS §7.1:10: A single-segment path that names a known const is
        // replaced by its value.
        ExprKind::Path(segs) if segs.len() == 1 => {
            known.get(segs[0].text(source)).copied()
        }
        // FLS §6.5: Binary arithmetic and bitwise operators in const context.
        ExprKind::Binary { op, lhs, rhs } => {
            let l = eval_const_expr(lhs, source, known, const_fns)?;
            let r = eval_const_expr(rhs, source, known, const_fns)?;
            match op {
                BinOp::Add    => l.checked_add(r),
                BinOp::Sub    => l.checked_sub(r),
                BinOp::Mul    => l.checked_mul(r),
                // FLS §6.23: Division by zero is an error; skip the const.
                BinOp::Div    => if r != 0 { l.checked_div(r) } else { None },
                BinOp::Rem    => if r != 0 { Some(l.wrapping_rem(r)) } else { None },
                BinOp::BitAnd => Some(l & r),
                BinOp::BitOr  => Some(l | r),
                BinOp::BitXor => Some(l ^ r),
                BinOp::Shl    => if (0..32).contains(&r) { l.checked_shl(r as u32) } else { None },
                BinOp::Shr    => if (0..32).contains(&r) { Some(l >> r) } else { None },
                // Assign, logical, and comparison operators are not valid in
                // const arithmetic initializers (they do not produce an integer).
                _ => None,
            }
        }
        // FLS §9:41–43: A call to a `const fn` in a const context is evaluated
        // at compile time. Only direct single-segment path calls are supported;
        // method calls and function pointer calls are not const-evaluable here.
        ExprKind::Call { callee, args } => {
            if let ExprKind::Path(segs) = &callee.kind
                && segs.len() == 1
            {
                let fn_name = segs[0].text(source);
                if let Some(fn_def) = const_fns.get(fn_name) {
                    // Evaluate each argument at compile time.
                    let arg_vals: Option<Vec<i32>> = args
                        .iter()
                        .map(|a| eval_const_expr(a, source, known, const_fns))
                        .collect();
                    let arg_vals = arg_vals?;
                    return eval_const_fn_body(fn_def, &arg_vals, source, known, const_fns, 0);
                }
            }
            None
        }
        // FLS §6.7: Parenthesized expressions — strip the grouping.
        // The parser does not emit a separate Group node; parenthesized
        // sub-expressions are already the inner node.  This arm is a
        // defensive no-op but does not hurt.
        _ => None,
    }
}

/// Evaluate a `const fn` body with the given argument values.
///
/// FLS §9:41–43: A `const fn` called from a const context is evaluated at
/// compile time by substituting argument values and executing the body as a
/// constant expression. Only bodies consisting of simple `let` bindings and
/// a tail expression are supported; loops, conditionals, and other control
/// flow are not yet const-evaluable.
///
/// `depth` guards against runaway mutual recursion between const fns; if
/// it exceeds 16 the evaluation yields `None` (unresolved). FLS §7.1 does
/// not specify a step limit for const evaluation; the limit here is an
/// implementation-defined safety bound.
fn eval_const_fn_body(
    fn_def: &crate::ast::FnDef,
    args: &[i32],
    source: &str,
    global_known: &HashMap<String, i32>,
    const_fns: &HashMap<String, &crate::ast::FnDef>,
    depth: u8,
) -> Option<i32> {
    // Recursion guard — FLS §7.1 AMBIGUOUS: no spec-mandated step limit.
    if depth > 16 {
        return None;
    }
    let body = fn_def.body.as_ref()?;
    // Build a local scope with global consts + parameter bindings.
    let mut local: HashMap<String, i32> = global_known.clone();
    for (param, &val) in fn_def.params.iter().zip(args.iter()) {
        // FLS §9.2: Simple `name: ty` parameters only (no destructuring in
        // const fn for now). FLS §9 AMBIGUOUS: The spec does not restrict
        // parameter patterns in const fn specifically.
        if let crate::ast::ParamKind::Ident(span) = param.kind {
            local.insert(span.text(source).to_owned(), val);
        } else {
            return None; // complex parameter patterns not const-evaluable
        }
    }
    // Execute let statements — only simple identifier patterns with const
    // initializers are supported.
    for stmt in &body.stmts {
        match &stmt.kind {
            StmtKind::Let { pat: Pat::Ident(span), init, .. } => {
                let init_val =
                    eval_const_expr(init.as_ref()?, source, &local, const_fns)?;
                local.insert(span.text(source).to_owned(), init_val);
            }
            StmtKind::Let { .. } => {
                return None; // complex parameter patterns not const-evaluable
            }
            StmtKind::Empty => {}
            _ => return None, // expression statements not const-evaluable
        }
    }
    // Evaluate the tail expression.
    eval_const_expr(body.tail.as_ref()?, source, &local, const_fns)
}

/// Evaluate a float const initializer at compile time.
///
/// FLS §7.1, §6.1.2:37–45: Const items may be initialised with float literal
/// expressions. Evaluates to `Some(f64)` when the expression is a float literal,
/// a negation of one, a reference to another known f64 const, or binary
/// arithmetic on any of the above. Returns `None` for any expression that
/// cannot be reduced at compile time.
///
/// `i32_known` is consulted for integer consts referenced inside a float
/// initialiser (e.g. `const C: f64 = MAX as f64;` is not handled here — only
/// pure-float expressions are folded).
///
/// Cache-line note: called only during the compile-time const collection pass,
/// not on any runtime hot path.
fn eval_float_const_expr(
    expr: &Expr,
    source: &str,
    f64_known: &HashMap<String, f64>,
) -> Option<f64> {
    match &expr.kind {
        // FLS §2.4.4.2: Float literal — parse the text value.
        ExprKind::LitFloat => {
            let text = expr.span.text(source);
            // Strip type suffixes (_f64, _f32) before parsing.
            let stripped = text.trim_end_matches("_f64").trim_end_matches("_f32");
            stripped.parse::<f64>().ok()
        }
        // FLS §6.5.3: Arithmetic negation in const context.
        ExprKind::Unary { op: crate::ast::UnaryOp::Neg, operand } => {
            Some(-eval_float_const_expr(operand, source, f64_known)?)
        }
        // FLS §7.1:10: A single-segment path that names a known f64 const.
        ExprKind::Path(segs) if segs.len() == 1 => {
            f64_known.get(segs[0].text(source)).copied()
        }
        // FLS §6.5: Binary arithmetic on f64 operands in const context.
        ExprKind::Binary { op, lhs, rhs } => {
            let l = eval_float_const_expr(lhs, source, f64_known)?;
            let r = eval_float_const_expr(rhs, source, f64_known)?;
            match op {
                BinOp::Add => Some(l + r),
                BinOp::Sub => Some(l - r),
                BinOp::Mul => Some(l * r),
                // FLS §6.23: Division by zero in const context — produce NaN,
                // which is not None (we do not want to silently skip the const).
                BinOp::Div => Some(l / r),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Evaluate a f32 const initializer at compile time.
///
/// FLS §7.1, §4.2: Like `eval_float_const_expr` but narrows to f32 and
/// consults the f32 const map.
///
/// Cache-line note: called only during compile-time const collection.
fn eval_f32_const_expr(
    expr: &Expr,
    source: &str,
    f32_known: &HashMap<String, f32>,
) -> Option<f32> {
    match &expr.kind {
        ExprKind::LitFloat => {
            let text = expr.span.text(source);
            // Only literals with an explicit `_f32` suffix produce f32 consts.
            if !text.ends_with("_f32") {
                return None;
            }
            let stripped = text.trim_end_matches("_f32");
            stripped.parse::<f32>().ok()
        }
        ExprKind::Unary { op: crate::ast::UnaryOp::Neg, operand } => {
            Some(-eval_f32_const_expr(operand, source, f32_known)?)
        }
        ExprKind::Path(segs) if segs.len() == 1 => {
            f32_known.get(segs[0].text(source)).copied()
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let l = eval_f32_const_expr(lhs, source, f32_known)?;
            let r = eval_f32_const_expr(rhs, source, f32_known)?;
            match op {
                BinOp::Add => Some(l + r),
                BinOp::Sub => Some(l - r),
                BinOp::Mul => Some(l * r),
                BinOp::Div => Some(l / r),
                _ => None,
            }
        }
        _ => None,
    }
}

/// definition table and used during function lowering for struct literal
/// and field-access expressions. Enum items with unit variants (FLS §15)
/// are collected into an enum definition table for path-expression and
/// path-pattern lowering.
pub fn lower(src: &SourceFile, source: &str) -> Result<Module, LowerError> {
    // First pass: collect struct and enum definitions.
    //
    // FLS §14: Struct definitions declare the field names and their types.
    // We store field names in declaration order; field access uses this
    // order to compute the stack-slot offset.
    //
    // FLS §15: Enum definitions declare the variant names and their kinds.
    // Unit variants are assigned discriminants 0, 1, 2, ... in declaration
    // order (FLS §15: explicit discriminants are not yet supported).
    //
    // Cache-line note: each i32 field or discriminant occupies one 8-byte
    // stack slot. A struct with N fields occupies N consecutive slots.
    let mut struct_defs: HashMap<String, Vec<String>> = HashMap::new();
    let mut enum_defs: EnumDefs = HashMap::new();
    // FLS §14.2: Tuple struct field counts. Maps struct name → field count.
    // Used to recognize constructor calls `Point(a, b)` during let-binding lowering.
    let mut tuple_struct_defs: HashMap<String, usize> = HashMap::new();
    // FLS §4.2, §14.2: Per-field float type for tuple structs.
    // `None` = integer/bool field; `Some(IrTy::F64)` = f64; `Some(IrTy::F32)` = f32.
    // Used to select StoreF64/StoreF32 vs Store during construction and
    // LoadF64Slot/LoadF32Slot vs Load when loading fields to pass as arguments.
    let mut tuple_struct_float_field_types: HashMap<String, Vec<Option<IrTy>>> = HashMap::new();
    // FLS §4.2, §15: Per-field float type for enum tuple variants.
    // Maps enum_name → variant_name → [field_float_types].
    // `None` = integer/bool field; `Some(IrTy::F64)` = f64; `Some(IrTy::F32)` = f32.
    // Used to select StoreF64/StoreF32 vs Store during variant construction and
    // LoadF64Slot/LoadF32Slot vs Load when binding fields in TupleStruct patterns.
    let mut enum_variant_float_field_types: HashMap<String, HashMap<String, Vec<Option<IrTy>>>> = HashMap::new();
    // FLS §6.11, §6.13, §4.11: Track per-field struct type names for nested struct
    // construction and chained field access. `None` = scalar field, `Some(name)` =
    // field whose type is another named struct (requiring multiple stack slots).
    //
    // Cache-line note: struct fields of struct type occupy their nested struct's total
    // slot count instead of a single slot, allowing precise offset computation.
    let mut struct_raw_field_types: HashMap<String, Vec<Option<String>>> = HashMap::new();
    // FLS §4.2, §6.11, §6.13: Per-field float type for named structs.
    // `None` = not a float field; `Some(IrTy::F64)` = f64; `Some(IrTy::F32)` = f32.
    // Used to choose StoreF64/LoadF64Slot vs StoreF32/LoadF32Slot vs Store/Load.
    let mut struct_float_field_types: HashMap<String, Vec<Option<IrTy>>> = HashMap::new();

    for item in &src.items {
        match &item.kind {
            ItemKind::Struct(s) => {
                let struct_name = s.name.text(source).to_owned();
                match &s.kind {
                    StructKind::Named(fields) => {
                        let field_names = fields
                            .iter()
                            .map(|f| f.name.text(source).to_owned())
                            .collect();
                        // FLS §6.13: Record the type of each field so we can compute
                        // nested slot offsets for chained field access (`s.b.x`).
                        // Scalar primitive types map to `None`; user-defined struct types
                        // map to `Some(type_name)` and occupy multiple consecutive slots.
                        let field_types: Vec<Option<String>> = fields
                            .iter()
                            .map(|f| match &f.ty.kind {
                                TyKind::Path(segs) if segs.len() == 1 => {
                                    let ty_name = segs[0].text(source);
                                    // Primitive scalar types — each occupies exactly one slot.
                                    match ty_name {
                                        "i8" | "i16" | "i32" | "i64" | "i128"
                                        | "u8" | "u16" | "u32" | "u64" | "u128"
                                        | "isize" | "usize" | "f32" | "f64"
                                        | "bool" | "char" => None,
                                        // Any other single-segment type is assumed to be a
                                        // user-defined struct — look it up in struct_defs later.
                                        other => Some(other.to_owned()),
                                    }
                                }
                                _ => None, // reference types, tuple types, etc. — treat as scalar
                            })
                            .collect();
                        // FLS §4.2: Record which scalar fields are f64 or f32 so struct
                        // literal stores and field-access loads can use the correct register
                        // bank (d-registers for f64, s-registers for f32).
                        let float_types: Vec<Option<IrTy>> = fields
                            .iter()
                            .map(|f| match &f.ty.kind {
                                TyKind::Path(segs) if segs.len() == 1 => {
                                    match segs[0].text(source) {
                                        "f64" => Some(IrTy::F64),
                                        "f32" => Some(IrTy::F32),
                                        _ => None,
                                    }
                                }
                                _ => None,
                            })
                            .collect();
                        struct_defs.insert(struct_name.clone(), field_names);
                        struct_raw_field_types.insert(struct_name.clone(), field_types);
                        struct_float_field_types.insert(struct_name, float_types);
                    }
                    StructKind::Unit => {
                        struct_defs.insert(struct_name.clone(), vec![]);
                        struct_raw_field_types.insert(struct_name.clone(), vec![]);
                        struct_float_field_types.insert(struct_name, vec![]);
                    }
                    StructKind::Tuple(fields) => {
                        // FLS §14.2: Tuple struct. Record field count so that
                        // constructor calls `Point(a, b)` can allocate the right
                        // number of consecutive stack slots.
                        let float_types: Vec<Option<IrTy>> = fields
                            .iter()
                            .map(|f| match &f.ty.kind {
                                TyKind::Path(segs) if segs.len() == 1 => {
                                    match segs[0].text(source) {
                                        "f64" => Some(IrTy::F64),
                                        "f32" => Some(IrTy::F32),
                                        _ => None,
                                    }
                                }
                                _ => None,
                            })
                            .collect();
                        tuple_struct_defs.insert(struct_name.clone(), fields.len());
                        tuple_struct_float_field_types.insert(struct_name, float_types);
                    }
                }
            }
            ItemKind::Enum(e) => {
                // FLS §15: Collect variants with auto-discriminants and field names.
                // Unit variants: empty field list. Tuple variants: positional
                // placeholder names. Named-field variants: actual declaration-order names.
                let enum_name = e.name.text(source).to_owned();
                let mut variants: HashMap<String, EnumVariantInfo> = HashMap::new();
                // FLS §4.2, §15: Per-variant float field type tracking.
                // Maps variant_name → [field_float_types] for tuple variants.
                let mut variant_float_types: HashMap<String, Vec<Option<IrTy>>> = HashMap::new();
                for (discriminant, variant) in e.variants.iter().enumerate() {
                    use crate::ast::EnumVariantKind;
                    let variant_name = variant.name.text(source).to_owned();
                    match &variant.kind {
                        EnumVariantKind::Unit => {
                            variants.insert(variant_name, (discriminant as i32, vec![]));
                        }
                        EnumVariantKind::Tuple(fields) => {
                            // Positional placeholders — names unused; only count matters.
                            variants.insert(
                                variant_name.clone(),
                                (discriminant as i32, vec!["".to_owned(); fields.len()]),
                            );
                            // FLS §4.2: Record f64/f32 field positions for this variant.
                            let float_tys: Vec<Option<IrTy>> = fields
                                .iter()
                                .map(|f| match &f.ty.kind {
                                    TyKind::Path(segs) if segs.len() == 1 => {
                                        match segs[0].text(source) {
                                            "f64" => Some(IrTy::F64),
                                            "f32" => Some(IrTy::F32),
                                            _ => None,
                                        }
                                    }
                                    _ => None,
                                })
                                .collect();
                            variant_float_types.insert(variant_name, float_tys);
                        }
                        // FLS §15.3: Named-field variant. Store names in declaration order
                        // so that construction and patterns can map name → slot index.
                        // FLS §4.2: Track f64/f32 field positions for named variants too,
                        // so construction and pattern binding can use float stores/loads.
                        EnumVariantKind::Named(fields) => {
                            let names: Vec<String> = fields
                                .iter()
                                .map(|f| f.name.text(source).to_owned())
                                .collect();
                            variants.insert(variant_name.clone(), (discriminant as i32, names));
                            let float_tys: Vec<Option<IrTy>> = fields
                                .iter()
                                .map(|f| match &f.ty.kind {
                                    TyKind::Path(segs) if segs.len() == 1 => {
                                        match segs[0].text(source) {
                                            "f64" => Some(IrTy::F64),
                                            "f32" => Some(IrTy::F32),
                                            _ => None,
                                        }
                                    }
                                    _ => None,
                                })
                                .collect();
                            if float_tys.iter().any(|t| t.is_some()) {
                                variant_float_types.insert(variant_name, float_tys);
                            }
                        }
                    }
                }
                enum_defs.insert(enum_name.clone(), variants);
                if !variant_float_types.is_empty() {
                    enum_variant_float_field_types.insert(enum_name, variant_float_types);
                }
            }
            ItemKind::Fn(_) | ItemKind::Impl(_) | ItemKind::Trait(_) | ItemKind::Const(_) | ItemKind::Static(_) | ItemKind::TypeAlias(_) => {}
        }
    }

    // Collect type alias IrTy mappings: maps alias name → resolved IrTy.
    //
    // FLS §4.10: A type alias defines a new name for an existing type.
    // Every occurrence of the alias in a type position is replaced by the
    // aliased type. Galvanic resolves aliases to their IrTy during this
    // first pass so that later type resolution calls (lower_ty) can handle
    // aliased names the same as primitive type names.
    //
    // Limitation: only aliases that expand to primitive types are resolved
    // here. Aliases to struct or enum types are not yet supported (future
    // milestone).
    //
    // Cache-line note: the alias map is populated once during the first pass
    // and read-only during the second pass. Not on any hot path.
    let mut type_alias_irtys: HashMap<String, IrTy> = HashMap::new();
    for item in &src.items {
        if let ItemKind::TypeAlias(ta) = &item.kind {
            let alias_name = ta.name.text(source).to_owned();
            // Resolve using already-accumulated aliases (handles chained aliases).
            if let Ok(irt) = lower_ty(&ta.ty, source, &type_alias_irtys) {
                type_alias_irtys.insert(alias_name, irt);
            }
        }
    }

    // Collect constant item values: maps const name → i32 value.
    //
    // FLS §7.1: Constant items are compile-time values substituted at every
    // use site. The initializer must be a constant expression (FLS §6.1.2:
    // 37–45). Galvanic evaluates const initializers via `eval_const_expr`,
    // which handles integer literals, arithmetic, and references to other
    // already-resolved consts.
    //
    // FLS §7.1:10: "Every use of a constant is replaced with its value
    // (or a copy of it)." Galvanic implements this by emitting `LoadImm`
    // when a path expression resolves to a known const name.
    //
    // Multi-pass strategy: a const whose initializer references another const
    // can only be evaluated after that other const is known. We loop until no
    // new values are discovered (fixed-point). The loop runs at most N+1 times
    // for N const items (with a well-founded dependency graph every pass
    // resolves at least one new const, or we stop on no progress).
    //
    // FLS §7.1 AMBIGUOUS: The spec does not specify the order in which const
    // items are evaluated relative to one another; it only requires each
    // initializer to be a constant expression. Galvanic's fixed-point pass
    // naturally handles forward references.
    //
    // Cache-line note: this HashMap is built once and shared read-only
    // across all `lower_fn` calls — not on any hot runtime path.
    // Collect `const fn` definitions for use in the const evaluator.
    //
    // FLS §9:41–43: A `const fn` may be called from a const context and
    // evaluated at compile time. We collect all top-level `const fn` items
    // into a map so that `eval_const_expr` can resolve calls to them.
    //
    // Cache-line note: this map is read-only after construction; built once,
    // not on any hot path.
    let const_fns: HashMap<String, &crate::ast::FnDef> = src
        .items
        .iter()
        .filter_map(|item| {
            if let ItemKind::Fn(fn_def) = &item.kind
                && fn_def.is_const
            {
                return Some((fn_def.name.text(source).to_owned(), fn_def.as_ref()));
            }
            None
        })
        .collect();

    let mut const_vals: HashMap<String, i32> = HashMap::new();
    // FLS §6.1.2:37–45: Evaluate all const initializers via the compile-time
    // evaluator. Repeat until no new consts are discovered (handles consts
    // that reference other consts defined later in the file).
    loop {
        let prev_len = const_vals.len();
        for item in &src.items {
            if let ItemKind::Const(c) = &item.kind {
                let name = c.name.text(source).to_owned();
                if !const_vals.contains_key(&name)
                    && let Some(val) =
                        eval_const_expr(&c.value, source, &const_vals, &const_fns)
                {
                    const_vals.insert(name, val);
                }
            }
        }
        // If no new consts were resolved this pass, we have reached fixed-point.
        // Remaining unresolved consts have unsupported initializer forms.
        if const_vals.len() == prev_len {
            break;
        }
    }

    // Collect f64 const items.
    //
    // FLS §7.1, §4.2: Const items with float types are evaluated at compile
    // time. The resulting f64 value is substituted at every use site via
    // `LoadF64Const` (not `LoadImm`). Iterates to fixed-point to handle
    // consts that reference other float consts.
    //
    // Cache-line note: each float const load emits `ldr d{N}, [pc, #offset]`
    // (4 bytes) + a per-function rodata literal pool entry (8 bytes for f64).
    let mut const_f64_vals: HashMap<String, f64> = HashMap::new();
    loop {
        let prev_len = const_f64_vals.len();
        for item in &src.items {
            if let ItemKind::Const(c) = &item.kind {
                let name = c.name.text(source).to_owned();
                if !const_f64_vals.contains_key(&name) {
                    // Only evaluate if the type annotation (if present) is f64
                    // or if the initializer is a float expression without _f32 suffix.
                    let is_f64_typed = if let crate::ast::TyKind::Path(segs) = &c.ty.kind {
                        segs.len() == 1 && segs[0].text(source) == "f64"
                    } else {
                        false
                    };
                    if is_f64_typed
                        && let Some(val) = eval_float_const_expr(&c.value, source, &const_f64_vals)
                    {
                        const_f64_vals.insert(name, val);
                    }
                }
            }
        }
        if const_f64_vals.len() == prev_len {
            break;
        }
    }

    // Collect f32 const items.
    //
    // FLS §7.1, §4.2: Same as f64 but for f32 consts. Float literals with
    // `_f32` suffix or type-annotated as `f32` are collected here.
    //
    // Cache-line note: each f32 const load emits `ldr s{N}, [pc, #offset]`
    // (4 bytes) + a per-function rodata literal pool entry (4 bytes for f32).
    let mut const_f32_vals: HashMap<String, f32> = HashMap::new();
    loop {
        let prev_len = const_f32_vals.len();
        for item in &src.items {
            if let ItemKind::Const(c) = &item.kind {
                let name = c.name.text(source).to_owned();
                if !const_f32_vals.contains_key(&name) {
                    let is_f32_typed = if let crate::ast::TyKind::Path(segs) = &c.ty.kind {
                        segs.len() == 1 && segs[0].text(source) == "f32"
                    } else {
                        false
                    };
                    if is_f32_typed
                        && let Some(val) = eval_f32_const_expr(&c.value, source, &const_f32_vals)
                    {
                        const_f32_vals.insert(name, val);
                    }
                }
            }
        }
        if const_f32_vals.len() == prev_len {
            break;
        }
    }

    // Collect static item names and their data-section entries.
    //
    // FLS §7.2: Static items are allocated in the data section with a fixed
    // address. Every use of a static emits a LoadStatic / LoadStaticF64 /
    // LoadStaticF32 (ADRP + ADD + LDR) rather than a LoadImm, because
    // FLS §7.2:15 requires all references to go through the same memory address.
    //
    // Cache-line note: each StaticData entry will become a `.quad` (f64/int,
    // 8 bytes) or `.word` (f32, 4 bytes) in the `.data` section.
    let mut static_data: Vec<crate::ir::StaticData> = Vec::new();
    let mut static_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut static_f64_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut static_f32_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in &src.items {
        if let ItemKind::Static(s) = &item.kind {
            let name = s.name.text(source).to_owned();
            if let ExprKind::LitInt(n) = &s.value.kind
                && *n <= i32::MAX as u128
            {
                static_data.push(crate::ir::StaticData {
                    name: name.clone(),
                    value: crate::ir::StaticValue::Int(*n as i32),
                });
                static_names.insert(name);
            } else if matches!(&s.value.kind, ExprKind::LitFloat) {
                // FLS §7.2, §4.2: f64/f32 float literal static initializers.
                // Determine type from optional suffix; default to f64.
                let text = s.value.span.text(source);
                if text.ends_with("f32") {
                    // Strip the "f32" suffix and any preceding underscore separator
                    // (e.g. "2.0_f32" → "2.0_" → "2.0"). FLS §2.4.4.2 permits an
                    // optional `_` between the numeric part and the type suffix.
                    let raw = text.trim_end_matches("f32").trim_end_matches('_');
                    let val: f32 = raw.parse().unwrap_or(0.0);
                    static_data.push(crate::ir::StaticData {
                        name: name.clone(),
                        value: crate::ir::StaticValue::F32(val),
                    });
                    static_f32_names.insert(name);
                } else {
                    let raw = text.trim_end_matches("f64").trim_end_matches('_');
                    let val: f64 = raw.parse().unwrap_or(0.0);
                    static_data.push(crate::ir::StaticData {
                        name: name.clone(),
                        value: crate::ir::StaticValue::F64(val),
                    });
                    static_f64_names.insert(name);
                }
            }
        }
    }

    // Collect free function names for function pointer support.
    //
    // FLS §4.9: When a single-segment path expression resolves to a function
    // item (not a local variable), it materializes the function's address via
    // `LoadFnAddr` (ADRP + ADD). This set is used to distinguish function-name
    // paths from undefined variables.
    //
    // Cache-line note: populated once at compile time; not on any hot path.
    let mut fn_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in &src.items {
        if let ItemKind::Fn(fn_def) = &item.kind {
            fn_names.insert(fn_def.name.text(source).to_owned());
        }
    }

    // Build method self-kind registry: mangled name → SelfKind.
    //
    // FLS §10.1: `&mut self` methods must propagate mutations back to the caller.
    // At the call site we need to know whether a method is `&mut self` so we
    // can emit `CallMut` (write-back) instead of a plain `Call`.
    //
    // Cache-line note: this HashMap is populated once at compile time and
    // read during method-call lowering. It is not on any hot runtime path.
    let mut method_self_kinds: HashMap<String, SelfKind> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Impl(impl_def) = &item.kind {
            let type_name = impl_def.ty.text(source);
            for method in &impl_def.methods {
                if let Some(kind) = method.self_param {
                    let method_name = method.name.text(source);
                    let mangled = format!("{type_name}__{method_name}");
                    method_self_kinds.insert(mangled, kind);
                }
            }
        }
    }

    // Build `&mut self` scalar-return registry: set of mangled names for `&mut self`
    // methods that return a scalar (non-unit, non-struct, non-enum) type.
    //
    // FLS §10.1: `&mut self` methods may return any type. When the return type is
    // a scalar (i32, bool, u32, etc.), the callee uses `RetFieldsAndValue` to pack
    // both the modified fields and the return value. The call site uses `CallMutReturn`
    // to capture the scalar from x{N} after writing back the fields.
    //
    // Cache-line note: populated once at compile time; not on any hot runtime path.
    let mut mut_self_scalar_return_fns: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in &src.items {
        if let ItemKind::Impl(impl_def) = &item.kind {
            let type_name = impl_def.ty.text(source);
            for method in &impl_def.methods {
                if method.self_param != Some(SelfKind::RefMut) {
                    continue;
                }
                let method_name = method.name.text(source);
                let mangled = format!("{type_name}__{method_name}");
                // Check if return type is a known scalar primitive (not void, not struct, not enum).
                if let Some(ret_ty_node) = &method.ret_ty
                    && let TyKind::Path(segs) = &ret_ty_node.kind
                    && segs.len() == 1
                {
                    let ret_name = segs[0].text(source);
                    // Scalar types handled by lower_ty: i32, bool, u32, etc.
                    // Exclude struct/enum types (handled by other registries).
                    if !struct_defs.contains_key(ret_name)
                        && !enum_defs.contains_key(ret_name)
                        && matches!(
                            ret_name,
                            "i32" | "i8" | "i16" | "i64" | "isize"
                                | "u8" | "u16" | "u32" | "u64" | "usize"
                                | "bool"
                        )
                    {
                        mut_self_scalar_return_fns.insert(mangled);
                    }
                }
            }
        }
    }

    // Build struct-returning associated function registry: mangled name → struct type name.
    //
    // FLS §10.1: Associated functions (no self parameter) that return a struct type
    // use a special write-back calling convention: the callee stores field values in
    // x0..x{N-1} via `RetFields`; the call site writes them to consecutive stack slots
    // via `CallMut`. This registry lets the call site identify such functions.
    //
    // Cache-line note: populated once at compile time, not on any hot path.
    let mut struct_return_fns: HashMap<String, String> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Impl(impl_def) = &item.kind {
            let type_name = impl_def.ty.text(source);
            for method in &impl_def.methods {
                if method.self_param.is_some() {
                    continue; // Only associated functions (no self).
                }
                let method_name = method.name.text(source);
                let mangled = format!("{type_name}__{method_name}");
                if let Some(ret_ty) = &method.ret_ty
                    && let TyKind::Path(segs) = &ret_ty.kind
                    && segs.len() == 1
                {
                    let ret_name = segs[0].text(source);
                    if struct_defs.contains_key(ret_name) {
                        // FLS §10.1: Associated function returning a struct type.
                        struct_return_fns.insert(mangled, ret_name.to_owned());
                    }
                }
            }
        }
    }

    // Build struct-returning `&self` instance method registry: mangled name → struct type name.
    //
    // FLS §10.1: `&self` instance methods that return a named struct type use
    // the same write-back calling convention as struct-returning associated
    // functions: the callee stores field values in x0..x{N-1} via `RetFields`;
    // the call site (in a `let` binding) writes them to the destination
    // variable's consecutive stack slots via `CallMut`.
    //
    // Only `&self` (SelfKind::Ref) methods are registered here. `&mut self`
    // methods with struct returns are not yet supported (they would need
    // both write-back of modified self fields AND return fields, which requires
    // a new calling convention).
    //
    // Cache-line note: populated once at compile time, not on any hot path.
    let mut struct_return_methods: HashMap<String, String> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Impl(impl_def) = &item.kind {
            let type_name = impl_def.ty.text(source);
            for method in &impl_def.methods {
                if method.self_param != Some(SelfKind::Ref) {
                    continue; // Only &self methods.
                }
                let method_name = method.name.text(source);
                let mangled = format!("{type_name}__{method_name}");
                if let Some(ret_ty) = &method.ret_ty
                    && let TyKind::Path(segs) = &ret_ty.kind
                    && segs.len() == 1
                {
                    let ret_name = segs[0].text(source);
                    if struct_defs.contains_key(ret_name) {
                        // FLS §10.1: &self method returning a struct type.
                        struct_return_methods.insert(mangled, ret_name.to_owned());
                    }
                }
            }
        }
    }

    // Build struct-returning free function registry: fn name → struct type name.
    //
    // FLS §9: Free functions that return a named struct type use the same
    // write-back calling convention as struct-returning associated functions:
    // the callee stores field values in x0..x{N-1} via `RetFields`; the call
    // site writes them to the destination variable's consecutive stack slots
    // via `CallMut`. This registry lets the call site identify such functions.
    //
    // Cache-line note: populated once at compile time, not on any hot path.
    let mut struct_return_free_fns: HashMap<String, String> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Fn(fn_def) = &item.kind {
            let fn_name = fn_def.name.text(source);
            if let Some(ret_ty) = &fn_def.ret_ty
                && let TyKind::Path(segs) = &ret_ty.kind
                && segs.len() == 1
            {
                let ret_name = segs[0].text(source);
                if struct_defs.contains_key(ret_name) {
                    // FLS §9: Free function returning a named struct.
                    struct_return_free_fns.insert(fn_name.to_owned(), ret_name.to_owned());
                }
            }
        }
    }

    // Build float-returning free function registries.
    //
    // FLS §4.2: Functions that return f64 or f32 use the float register bank
    // for their return value: f64 in d0, f32 in s0. The call site must capture
    // from the float register rather than x0.
    //
    // Cache-line note: populated once at compile time, not on any hot path.
    let mut f64_return_fns: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut f32_return_fns: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in &src.items {
        if let ItemKind::Fn(fn_def) = &item.kind {
            let fn_name = fn_def.name.text(source);
            if let Some(ret_ty) = &fn_def.ret_ty
                && let TyKind::Path(segs) = &ret_ty.kind
                && segs.len() == 1
            {
                match segs[0].text(source) {
                    "f64" => { f64_return_fns.insert(fn_name.to_owned()); }
                    "f32" => { f32_return_fns.insert(fn_name.to_owned()); }
                    _ => {}
                }
            }
        }
    }
    // FLS §10.1, §4.2: Methods (both &self and &mut self) that return f64 or
    // f32 also use the float register bank for the return value. Register their
    // mangled names so the call site captures from d0/s0.
    for item in &src.items {
        if let ItemKind::Impl(impl_def) = &item.kind {
            let type_name = impl_def.ty.text(source);
            for method in &impl_def.methods {
                let method_name = method.name.text(source);
                let mangled = format!("{type_name}__{method_name}");
                if let Some(ret_ty) = &method.ret_ty
                    && let TyKind::Path(segs) = &ret_ty.kind
                    && segs.len() == 1
                {
                    match segs[0].text(source) {
                        "f64" => { f64_return_fns.insert(mangled); }
                        "f32" => { f32_return_fns.insert(mangled); }
                        _ => {}
                    }
                }
            }
        }
    }

    // Build enum-returning free function registry: fn name → enum type name.
    //
    // FLS §9, §15: Free functions that return an enum type use a write-back
    // calling convention: the callee stores discriminant + fields in
    // x0..x{1+max_fields} via `RetFields`; the call site writes them to the
    // destination enum variable's stack slots via `CallMut`.
    //
    // Cache-line note: populated once at compile time, not on any hot path.
    let mut enum_return_fns: HashMap<String, String> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Fn(fn_def) = &item.kind {
            let fn_name = fn_def.name.text(source);
            if let Some(ret_ty) = &fn_def.ret_ty
                && let TyKind::Path(segs) = &ret_ty.kind
                && segs.len() == 1
            {
                let ret_name = segs[0].text(source);
                if enum_defs.contains_key(ret_name) {
                    enum_return_fns.insert(fn_name.to_owned(), ret_name.to_owned());
                }
            }
        }
    }

    // Build tuple-returning free function registry: fn name → number of tuple elements.
    //
    // FLS §6.10, §9: Free functions that return a tuple type use the same
    // write-back calling convention as struct-returning functions: the callee
    // stores each element in consecutive stack slots and returns them in
    // x0..x{N-1} via `RetFields`; the call site writes them back to the
    // destination tuple variable's consecutive stack slots via `CallMut`.
    //
    // FLS §6.10 AMBIGUOUS: The spec does not define a calling convention for
    // tuple-returning functions. Galvanic uses the same register-packing
    // convention as struct returns: element[0] in x0, element[1] in x1, etc.
    //
    // Cache-line note: populated once at compile time, not on any hot path.
    let mut tuple_return_free_fns: HashMap<String, usize> = HashMap::new();
    for item in &src.items {
        if let ItemKind::Fn(fn_def) = &item.kind
            && let Some(ret_ty) = &fn_def.ret_ty
            && let TyKind::Tuple(elems) = &ret_ty.kind
            && !elems.is_empty()
        {
            let fn_name = fn_def.name.text(source);
            tuple_return_free_fns.insert(fn_name.to_owned(), elems.len());
        }
    }

    // Compute struct sizes and field offsets for nested struct support.
    //
    // FLS §6.11: Struct expressions with struct-type fields require knowing the
    // total slot count for each field type to allocate contiguous stack slots.
    // FLS §6.13: Chained field access (`s.b.x`) requires computing the byte
    // offset of field `b` within `s`, which equals the sum of sizes of preceding
    // fields. For scalar fields this is 1 slot; for struct-type fields it is
    // the nested struct's total slot count.
    //
    // Algorithm: iteratively compute sizes until fixed point (handles forward
    // references between structs; cycles left at their default of field_count).
    //
    // FLS §4.11: Representation. Galvanic lays out struct fields in declaration
    // order with no padding (each slot is 8 bytes). This matches the ARM64 ABI
    // for small aggregate passing.
    //
    // Cache-line note: struct_sizes and struct_field_offsets are read-only after
    // construction and are not on any hot path.
    let mut struct_sizes: HashMap<String, usize> = HashMap::new();
    loop {
        let mut changed = false;
        for (sname, ftypes) in &struct_raw_field_types {
            if struct_sizes.contains_key(sname) {
                continue;
            }
            let mut total = 0usize;
            let mut can_compute = true;
            for ft in ftypes {
                match ft {
                    None => total += 1,
                    Some(inner) => {
                        if let Some(&sz) = struct_sizes.get(inner) {
                            total += sz;
                        } else {
                            can_compute = false;
                            break;
                        }
                    }
                }
            }
            if can_compute {
                struct_sizes.insert(sname.clone(), total);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // Any struct not yet resolved (unknown field type or cycle) defaults to
    // its declared field count — same as the pre-nested-struct behaviour.
    for (sname, field_names) in &struct_defs {
        struct_sizes.entry(sname.clone()).or_insert(field_names.len());
    }

    // Compute field slot offsets: for each struct, the slot offset of each field
    // relative to the struct's base slot.
    //
    // Example: `Outer { a: Inner { x: i32, y: i32 }, b: i32 }` has
    //   field_offsets["Outer"] = [0, 2]  (a starts at 0, b at 2)
    //   struct_sizes["Outer"] = 3        (2 for Inner + 1 for b)
    let mut struct_field_offsets: HashMap<String, Vec<usize>> = HashMap::new();
    for (sname, ftypes) in &struct_raw_field_types {
        let mut offsets = Vec::with_capacity(ftypes.len());
        let mut off = 0usize;
        for ft in ftypes {
            offsets.push(off);
            off += match ft {
                None => 1,
                Some(inner) => struct_sizes.get(inner).copied().unwrap_or(1),
            };
        }
        struct_field_offsets.insert(sname.clone(), offsets);
    }

    // Second pass: lower function items.
    //
    // FLS §6.17: Branch target labels must be unique within the assembly file.
    // GAS local labels (`.L{n}`) are file-scoped, so if two functions both emit
    // `.L0:` and `.L1:` the assembler will error or branch to the wrong target.
    // We pass a monotonically increasing `label_base` to each `lower_fn` call
    // so that every function's labels are globally unique.
    let mut fns = Vec::new();
    let mut trampolines: Vec<crate::ir::ClosureTrampoline> = Vec::new();
    let mut label_base: u32 = 0;
    for item in &src.items {
        match &item.kind {
            ItemKind::Fn(fn_def) => {
                let (ir_fn, closure_fns, fn_trampolines, next_label) = lower_fn(fn_def, source, &struct_defs, &tuple_struct_defs, &tuple_struct_float_field_types, &enum_defs, &enum_variant_float_field_types, &method_self_kinds, &mut_self_scalar_return_fns, &struct_return_fns, &struct_return_free_fns, &enum_return_fns, &struct_return_methods, &tuple_return_free_fns, &f64_return_fns, &f32_return_fns, &const_vals, &const_f64_vals, &const_f32_vals, &static_names, &static_f64_names, &static_f32_names, &fn_names, &struct_raw_field_types, &struct_field_offsets, &struct_sizes, &type_alias_irtys, &struct_float_field_types, None, label_base)?;
                label_base = next_label;
                fns.push(ir_fn);
                fns.extend(closure_fns);
                trampolines.extend(fn_trampolines);
            }
            ItemKind::Impl(impl_def) => {
                // FLS §11: Inherent impl and trait impl. Each method becomes a
                // mangled top-level function: `TypeName__method_name`.
                // For trait impls (`impl Trait for Type`), the struct type is
                // `impl_def.ty` and mangling is identical to inherent impls.
                // FLS §13: Trait method implementations lower identically to
                // inherent methods — static dispatch uses the same mangling.
                let type_name = impl_def.ty.text(source);
                for method in &impl_def.methods {
                    let method_name = method.name.text(source);
                    let mangled = format!("{type_name}__{method_name}");
                    // Always create MethodCtx so associated functions (no self_param)
                    // get the mangled name. impl_type and self_kind are None for
                    // associated functions.
                    //
                    // FLS §10.1: Associated functions do not have a self parameter.
                    let mctx = Some(MethodCtx {
                        impl_type: method.self_param.map(|_| type_name),
                        mangled_name: &mangled,
                        self_kind: method.self_param,
                    });
                    let (ir_fn, closure_fns, fn_trampolines, next_label) = lower_fn(
                        method,
                        source,
                        &struct_defs,
                        &tuple_struct_defs,
                        &tuple_struct_float_field_types,
                        &enum_defs,
                        &enum_variant_float_field_types,
                        &method_self_kinds,
                        &mut_self_scalar_return_fns,
                        &struct_return_fns,
                        &struct_return_free_fns,
                        &enum_return_fns,
                        &struct_return_methods,
                        &tuple_return_free_fns,
                        &f64_return_fns,
                        &f32_return_fns,
                        &const_vals,
                        &const_f64_vals,
                        &const_f32_vals,
                        &static_names,
                        &static_f64_names,
                        &static_f32_names,
                        &fn_names,
                        &struct_raw_field_types,
                        &struct_field_offsets,
                        &struct_sizes,
                        &type_alias_irtys,
                        &struct_float_field_types,
                        mctx,
                        label_base,
                    )?;
                    label_base = next_label;
                    fns.push(ir_fn);
                    fns.extend(closure_fns);
                    trampolines.extend(fn_trampolines);
                }
            }
            ItemKind::Struct(_) | ItemKind::Enum(_) => {} // already processed above
            // FLS §13: Trait definitions are parsed but produce no codegen.
            // Trait method implementations are emitted via `ItemKind::Impl`.
            ItemKind::Trait(_) => {}
            // FLS §7.1: Const items are collected in the first pass above.
            // They produce no runtime code of their own.
            ItemKind::Const(_) => {}
            // FLS §7.2: Static items are collected in the first pass above.
            // They produce no function code — only data section entries.
            ItemKind::Static(_) => {}
            // FLS §4.10: Type aliases are resolved in the first pass above.
            // They produce no runtime code.
            ItemKind::TypeAlias(_) => {}
        }
    }

    Ok(Module { fns, statics: static_data, trampolines })
}

// ── Function lowering ────────────────────────────────────────────────────────

/// Context passed to `lower_fn` for method lowering.
///
/// Bundles the parameters that distinguish a method from a free function.
/// FLS §10.1: Methods have a `self` parameter and a mangled name.
/// Associated functions (no `self`) also get a `MethodCtx` for the mangled
/// name; their `impl_type` and `self_kind` are both `None`.
struct MethodCtx<'a> {
    /// Struct type name this method belongs to (for self-field spilling).
    /// `None` for associated functions (no `self` parameter).
    impl_type: Option<&'a str>,
    /// Mangled function name (`TypeName__method_name`).
    mangled_name: &'a str,
    /// How `self` is received (value, shared ref, mutable ref).
    /// `None` for associated functions (no `self` parameter).
    self_kind: Option<SelfKind>,
}

/// Lower a single function (or method) definition to an `IrFn`.
///
/// FLS §9: Functions. FLS §10.1: Methods.
///
/// - `method`: if `Some`, this function is a method of the named struct type.
///   The struct's fields are spilled from leading registers before any explicit
///   parameters, and the function is emitted under the mangled name.
///
/// FLS §6.12.1: Functions with parameters receive arguments in x0–x{n-1}
/// per the ARM64 ABI. We spill each parameter to a stack slot so that
/// path expressions can reference them via `Load` — reusing the same
/// infrastructure as let-binding locals.
/// Collect all leaf patterns from a nested tuple parameter pattern in
/// left-to-right, depth-first order.
///
/// Each leaf is `Some(&Span)` for a named binding (`Pat::Ident`) or `None`
/// for a wildcard (`Pat::Wildcard`). The length of the returned vec equals the
/// number of consecutive ARM64 registers consumed by the nested tuple pattern.
///
/// Nested `Pat::Tuple` elements are flattened recursively.
///
/// FLS §5.10.3, §9.2: Nested tuple patterns in parameter position flatten to
/// consecutive registers matching the ARM64 calling convention. For example,
/// `(a, (b, c)): (i32, (i32, i32))` passes three values in x0, x1, x2,
/// which bind to `a`, `b`, `c` respectively.
///
/// FLS §6.1.2:37–45: All spill stores are runtime instructions.
fn collect_tuple_param_leaves(pats: &[Pat]) -> Result<Vec<Option<&crate::ast::Span>>, LowerError> {
    let mut leaves = Vec::new();
    for pat in pats {
        match pat {
            Pat::Ident(span) => leaves.push(Some(span)),
            Pat::Wildcard => leaves.push(None),
            Pat::Tuple(inner) => {
                leaves.extend(collect_tuple_param_leaves(inner)?);
            }
            _ => {
                return Err(LowerError::Unsupported(
                    "only identifier, wildcard, and nested tuple patterns are \
                     supported inside tuple parameter patterns"
                        .into(),
                ));
            }
        }
    }
    Ok(leaves)
}

#[allow(clippy::too_many_arguments)]
fn lower_fn(
    fn_def: &crate::ast::FnDef,
    source: &str,
    struct_defs: &HashMap<String, Vec<String>>,
    tuple_struct_defs: &HashMap<String, usize>,
    tuple_struct_float_field_types: &HashMap<String, Vec<Option<IrTy>>>,
    enum_defs: &EnumDefs,
    enum_variant_float_field_types: &HashMap<String, HashMap<String, Vec<Option<IrTy>>>>,
    method_self_kinds: &HashMap<String, SelfKind>,
    mut_self_scalar_return_fns: &std::collections::HashSet<String>,
    struct_return_fns: &HashMap<String, String>,
    struct_return_free_fns: &HashMap<String, String>,
    enum_return_fns: &HashMap<String, String>,
    struct_return_methods: &HashMap<String, String>,
    tuple_return_free_fns: &HashMap<String, usize>,
    f64_return_fns: &std::collections::HashSet<String>,
    f32_return_fns: &std::collections::HashSet<String>,
    const_vals: &HashMap<String, i32>,
    const_f64_vals: &HashMap<String, f64>,
    const_f32_vals: &HashMap<String, f32>,
    static_names: &std::collections::HashSet<String>,
    static_f64_names: &std::collections::HashSet<String>,
    static_f32_names: &std::collections::HashSet<String>,
    fn_names: &std::collections::HashSet<String>,
    struct_field_types: &HashMap<String, Vec<Option<String>>>,
    struct_field_offsets: &HashMap<String, Vec<usize>>,
    struct_sizes: &HashMap<String, usize>,
    type_aliases: &HashMap<String, IrTy>,
    struct_float_field_types: &HashMap<String, Vec<Option<IrTy>>>,
    method: Option<MethodCtx<'_>>,
    start_label: u32,
) -> Result<(IrFn, Vec<IrFn>, Vec<crate::ir::ClosureTrampoline>, u32), LowerError> {
    // For associated functions, impl_type = None and self_kind = None.
    let impl_type = method.as_ref().and_then(|m| m.impl_type);
    let override_name = method.as_ref().map(|m| m.mangled_name);
    let self_kind = method.as_ref().and_then(|m| m.self_kind);
    let name = override_name
        .map(|s| s.to_owned())
        .unwrap_or_else(|| fn_def.name.text(source).to_owned());

    // FLS §9: "If no return type is specified, the return type is `()`."
    // For functions returning a struct or enum type, `lower_ty` would fail
    // (struct/enum names are not primitive IR types). Detect them separately.
    //
    // FLS §10.1: Associated functions may return the impl type or any other type.
    // FLS §9, §15: Free functions may return an enum type.
    let (ret_ty, struct_ret_name, enum_ret_name, tuple_ret_n) = match &fn_def.ret_ty {
        None => (IrTy::Unit, None, None, None),
        Some(ty) => {
            match lower_ty(ty, source, type_aliases) {
                Ok(t) => (t, None, None, None),
                Err(_) => {
                    // FLS §6.10, §9: Tuple return type — elements returned in x0..x{N-1}.
                    if let TyKind::Tuple(elems) = &ty.kind {
                        if elems.is_empty() {
                            // Empty tuple () is the unit type — already handled by lower_ty.
                            (IrTy::Unit, None, None, None)
                        } else {
                            // Non-empty tuple: use Unit as placeholder IrTy; the actual
                            // return is handled via RetFields in the body lowering below.
                            (IrTy::Unit, None, None, Some(elems.len() as u8))
                        }
                    } else if let TyKind::Path(segs) = &ty.kind {
                        // Check if the return type is a known struct or enum.
                        if segs.len() == 1 {
                            let ret_name = segs[0].text(source);
                            if struct_defs.contains_key(ret_name) {
                                // Function returning a struct type.
                                // Use Unit as a placeholder IrTy; the actual return
                                // is handled via RetFields in the body lowering below.
                                (IrTy::Unit, Some(ret_name.to_owned()), None, None)
                            } else if enum_defs.contains_key(ret_name) {
                                // FLS §9, §15: Free function returning an enum type.
                                // Use Unit as a placeholder IrTy; the actual return
                                // is handled via RetFields after `lower_enum_expr_into`.
                                (IrTy::Unit, None, Some(ret_name.to_owned()), None)
                            } else {
                                return Err(LowerError::Unsupported(format!(
                                    "return type `{ret_name}` (not a known struct, enum, or primitive)"
                                )));
                            }
                        } else {
                            return Err(LowerError::Unsupported("multi-segment return type".into()));
                        }
                    } else {
                        return Err(LowerError::Unsupported("complex return type".into()));
                    }
                }
            }
        }
    };

    let body = match &fn_def.body {
        None => {
            return Err(LowerError::Unsupported(
                "extern / bodyless functions".into(),
            ));
        }
        Some(block) => block,
    };

    let mut ctx = LowerCtx::new(source, &name, ret_ty, struct_defs, tuple_struct_defs, tuple_struct_float_field_types, enum_defs, enum_variant_float_field_types, method_self_kinds, mut_self_scalar_return_fns, struct_return_fns, struct_return_free_fns, enum_return_fns, struct_return_methods, tuple_return_free_fns, f64_return_fns, f32_return_fns, const_vals, const_f64_vals, const_f32_vals, static_names, static_f64_names, static_f32_names, fn_names, struct_field_types, struct_field_offsets, struct_sizes, type_aliases, struct_float_field_types, start_label);

    // FLS §9: Spill incoming parameters from ARM64 registers x0..x{n-1}
    // to stack slots. Each parameter slot is allocated in parameter order
    // so that subsequent path expressions emit `Load { slot }`.
    //
    // ARM64 ABI: the first 8 integer/pointer parameters arrive in x0–x7.
    // Spilling them to the stack normalises parameter access to the same
    // `Load` instruction used for let-binding locals, keeping codegen simple.
    //
    // Cache-line note: each spill is one `str` instruction (4 bytes); two
    // spills per 8-byte stack pair keep the spill sequence cache-aligned.
    // FLS §9: Spill parameters from ARM64 registers to stack slots.
    // `reg_idx` tracks the current register number, which may advance by more
    // than 1 per parameter when an enum type occupies multiple registers
    // (discriminant + field registers).
    let mut reg_idx: usize = 0;
    // `freg_idx` tracks the float register index (d0–d7 / s0–s7).
    // ARM64 ABI: float args occupy a separate register bank from integer args.
    // FLS §4.2: f64/f32 parameters are passed in d0–d7 / s0–s7.
    let mut freg_idx: usize = 0;

    // FLS §10.1: If this is a method with a self parameter, spill the self
    // value from leading registers.
    //
    // For struct self: each field arrives in a separate register (x0..x{N-1}).
    // For enum self: the discriminant arrives in x0, then field registers follow
    // (x1..x{max_fields}), matching the enum-parameter calling convention.
    //
    // `&self` and `self` are lowered identically at this milestone: the
    // caller passes each field as an individual integer register argument
    // (a value copy). The method body can read but not mutate the original.
    //
    // FLS §10.1, §11, §15: impl blocks are legal for both struct and enum types.
    //
    // Cache-line note: N field spills = N `str` instructions (4 bytes each).
    if let Some(type_name) = impl_type {
        if let Some(field_names) = struct_defs.get(type_name) {
            // Struct self parameter: one register per field.
            let n_fields = field_names.len();
            if n_fields > 0 {
                // FLS §4.2, §10.1: f64/f32 struct fields arrive in the float
                // register bank (d0-d7 / s0-s7); integer fields arrive in x0-x7.
                // Count them separately to check register window limits.
                let float_field_tys = struct_float_field_types
                    .get(type_name)
                    .cloned()
                    .unwrap_or_default();
                let n_int_fields = float_field_tys
                    .iter()
                    .filter(|t| t.is_none())
                    .count()
                    .max(n_fields.saturating_sub(float_field_tys.len()));
                let n_float_fields = float_field_tys
                    .iter()
                    .filter(|t| t.is_some())
                    .count();
                if reg_idx + n_int_fields > 8 || freg_idx + n_float_fields > 8 {
                    return Err(LowerError::Unsupported(
                        "self fields exceed ARM64 register window".into(),
                    ));
                }
                let base_slot = ctx.alloc_slot()?;
                for _ in 1..n_fields {
                    ctx.alloc_slot()?;
                }
                let mut self_int = 0usize;
                let mut self_float = 0usize;
                for fi in 0..n_fields {
                    let slot = base_slot + fi as u8;
                    match float_field_tys.get(fi).copied().flatten() {
                        Some(IrTy::F64) => {
                            // FLS §4.2: f64 self field arrives in d{freg_idx+self_float}.
                            ctx.instrs.push(Instr::StoreF64 {
                                src: (freg_idx + self_float) as u8,
                                slot,
                            });
                            ctx.slot_float_ty.insert(slot, IrTy::F64);
                            self_float += 1;
                        }
                        Some(IrTy::F32) => {
                            // FLS §4.2: f32 self field arrives in s{freg_idx+self_float}.
                            ctx.instrs.push(Instr::StoreF32 {
                                src: (freg_idx + self_float) as u8,
                                slot,
                            });
                            ctx.slot_float_ty.insert(slot, IrTy::F32);
                            self_float += 1;
                        }
                        _ => {
                            // Integer/bool/pointer field arrives in x{reg_idx+self_int}.
                            ctx.instrs.push(Instr::Store {
                                src: (reg_idx + self_int) as u8,
                                slot,
                            });
                            self_int += 1;
                        }
                    }
                }
                // Register `self` as a struct variable pointing to base_slot.
                // `self` is a keyword but &'static str coerces to &'src str.
                ctx.locals.insert("self", base_slot);
                ctx.local_struct_types.insert(base_slot, type_name.to_owned());
                reg_idx += self_int;
                freg_idx += self_float;
            } else {
                // Unit struct: no fields to pass. Register `self` with a dummy slot.
                let base_slot = ctx.alloc_slot()?;
                ctx.locals.insert("self", base_slot);
                ctx.local_struct_types.insert(base_slot, type_name.to_owned());
            }
        } else if let Some(variants) = enum_defs.get(type_name) {
            // Enum self parameter: discriminant + up to max_fields registers.
            //
            // FLS §15: Enum values carry a discriminant (tag) and variant-specific
            // fields. The calling convention mirrors enum parameter passing:
            // x{reg_idx} = discriminant, x{reg_idx+1}..x{reg_idx+max_fields} = fields.
            //
            // FLS §6.1.2:37–45: All spills are runtime store instructions.
            // Cache-line note: (1 + max_fields) × 4-byte `str` per enum self spill.
            let max_fields = variants.values().map(|(_, names)| names.len()).max().unwrap_or(0);
            let regs_needed = 1 + max_fields;
            if reg_idx + regs_needed > 8 {
                return Err(LowerError::Unsupported(
                    "enum self exceeds ARM64 register window".into(),
                ));
            }
            let base_slot = ctx.alloc_slot()?;
            for _ in 0..max_fields {
                ctx.alloc_slot()?;
            }
            // Spill discriminant.
            ctx.instrs.push(Instr::Store { src: reg_idx as u8, slot: base_slot });
            // Spill field registers.
            for fi in 0..max_fields {
                ctx.instrs.push(Instr::Store {
                    src: (reg_idx + fi + 1) as u8,
                    slot: base_slot + 1 + fi as u8,
                });
            }
            // Register `self` as an enum variable so match expressions on `self`
            // use the discriminant-based dispatch path.
            ctx.locals.insert("self", base_slot);
            ctx.local_enum_types.insert(base_slot, type_name.to_owned());
            reg_idx += regs_needed;
        } else if let Some(&n_fields) = tuple_struct_defs.get(type_name) {
            // FLS §14.2, §10.1: Tuple struct self parameter.
            //
            // A tuple struct with N fields is passed as N consecutive registers,
            // with integer fields in x{reg_idx}..x{reg_idx+n_int-1} and float
            // fields in d{freg_idx}..d{freg_idx+n_float-1} (ARM64 ABI).
            // Spill to N consecutive stack slots; register in `local_tuple_lens`
            // for `.0`/`.1` field access and in `local_tuple_struct_types` for
            // method call dispatch.
            //
            // FLS §4.2: f64/f32 fields arrive in the float register bank (d0-d7
            // / s0-s7); integer fields arrive in x0-x7.
            // FLS §6.1.2:37–45: All spills are runtime store instructions.
            // Cache-line note: N × 4-byte `str` per self spill.
            if n_fields > 0 {
                let float_field_tys = tuple_struct_float_field_types
                    .get(type_name)
                    .cloned()
                    .unwrap_or_default();
                let n_int_fields = float_field_tys
                    .iter()
                    .filter(|t| t.is_none())
                    .count()
                    .max(n_fields.saturating_sub(float_field_tys.len()));
                let n_float_fields = float_field_tys.iter().filter(|t| t.is_some()).count();
                if reg_idx + n_int_fields > 8 || freg_idx + n_float_fields > 8 {
                    return Err(LowerError::Unsupported(
                        "tuple struct self fields exceed ARM64 register window".into(),
                    ));
                }
                let base_slot = ctx.alloc_slot()?;
                for _ in 1..n_fields {
                    ctx.alloc_slot()?;
                }
                let mut self_int = 0usize;
                let mut self_float = 0usize;
                for fi in 0..n_fields {
                    let slot = base_slot + fi as u8;
                    match float_field_tys.get(fi).copied().flatten() {
                        Some(IrTy::F64) => {
                            // FLS §4.2: f64 self field arrives in d{freg_idx+self_float}.
                            ctx.instrs.push(Instr::StoreF64 {
                                src: (freg_idx + self_float) as u8,
                                slot,
                            });
                            ctx.slot_float_ty.insert(slot, IrTy::F64);
                            self_float += 1;
                        }
                        Some(IrTy::F32) => {
                            // FLS §4.2: f32 self field arrives in s{freg_idx+self_float}.
                            ctx.instrs.push(Instr::StoreF32 {
                                src: (freg_idx + self_float) as u8,
                                slot,
                            });
                            ctx.slot_float_ty.insert(slot, IrTy::F32);
                            self_float += 1;
                        }
                        _ => {
                            // Integer/bool field arrives in x{reg_idx+self_int}.
                            ctx.instrs.push(Instr::Store {
                                src: (reg_idx + self_int) as u8,
                                slot,
                            });
                            self_int += 1;
                        }
                    }
                }
                ctx.locals.insert("self", base_slot);
                ctx.local_tuple_lens.insert(base_slot, n_fields);
                ctx.local_tuple_struct_types.insert(base_slot, type_name.to_owned());
                reg_idx += self_int;
                freg_idx += self_float;
            } else {
                // Zero-field tuple struct: allocate a dummy slot for `self`.
                let base_slot = ctx.alloc_slot()?;
                ctx.locals.insert("self", base_slot);
                ctx.local_tuple_struct_types.insert(base_slot, type_name.to_owned());
            }
        } else {
            return Err(LowerError::Unsupported(format!(
                "impl for unknown type `{type_name}`"
            )));
        }
    }
    for param in fn_def.params.iter() {
        // FLS §5.10.3, §9.2: Tuple pattern parameter, flat or nested.
        //
        // `(a, b): (T1, T2)` — each leaf occupies one ARM64 register.
        // `(a, (b, c)): (T1, (T2, T3))` — three leaves, three registers.
        // The nested structure is purely syntactic: the calling convention is
        // always flat (one register per scalar leaf), matching how the caller
        // passes a tuple value as a sequence of individual register arguments.
        //
        // `collect_tuple_param_leaves` flattens the pattern tree left-to-right,
        // depth-first, yielding one entry per leaf in register order.
        //
        // Cache-line note: N leaves → N × 4-byte `str` spill instructions,
        // same density as flat tuple, struct, and tuple-struct parameters.
        // FLS §6.1.2:37–45: All spills are runtime store instructions.
        if let crate::ast::ParamKind::Tuple(pats) = &param.kind {
            let leaves = collect_tuple_param_leaves(pats)?;
            if reg_idx + leaves.len() > 8 {
                return Err(LowerError::Unsupported(
                    "tuple parameter exceeds ARM64 register window (>8 total registers)".into(),
                ));
            }
            for leaf in &leaves {
                let slot = ctx.alloc_slot()?;
                ctx.instrs.push(Instr::Store { src: reg_idx as u8, slot });
                if let Some(span) = leaf {
                    let name = span.text(source);
                    if name != "_" {
                        ctx.locals.insert(name, slot);
                    }
                }
                reg_idx += 1;
            }
            continue;
        }

        // FLS §5.10.2, §9.2: Struct pattern parameter `Point { x, y }: Point`.
        //
        // The struct value arrives in consecutive registers (one per flat slot),
        // matching the named struct calling convention used for `p: Point`
        // parameters. Spill each register to a slot and bind each named field
        // directly, skipping the intermediate struct-variable binding.
        //
        // Cache-line note: N flat slots → N × 4-byte `str` spill instructions.
        // Same instruction density as the plain `p: Point` path; no extra cost.
        if let crate::ast::ParamKind::Struct { type_span, fields } = &param.kind {
            let type_name = type_span.text(source);
            if let Some(field_names) = struct_defs.get(type_name) {
                let n_slots = struct_sizes.get(type_name).copied().unwrap_or(field_names.len());
                if n_slots > 0 && reg_idx + n_slots > 8 {
                    return Err(LowerError::Unsupported(
                        "struct pattern parameter exceeds ARM64 register window (>8 total registers)".into(),
                    ));
                }
                // Allocate consecutive stack slots for all fields.
                let base_slot = ctx.alloc_slot()?;
                for _ in 1..n_slots {
                    ctx.alloc_slot()?;
                }
                // Spill each incoming register to its slot.
                for fi in 0..n_slots {
                    ctx.instrs.push(Instr::Store {
                        src: (reg_idx + fi) as u8,
                        slot: base_slot + fi as u8,
                    });
                }
                // Bind each named field in the pattern to its slot.
                // FLS §5.10.2: The struct field binding order is determined by
                // the *struct definition*, not the pattern source order.
                // Use struct_field_offsets to get the correct slot even when
                // nested struct fields occupy multiple consecutive slots.
                let offsets = struct_field_offsets.get(type_name);
                for (field_name_span, binding_pat) in fields {
                    let fname = field_name_span.text(source);
                    let Some(fi) = field_names.iter().position(|f| f == fname) else {
                        continue;
                    };
                    let field_offset = offsets.and_then(|o| o.get(fi).copied()).unwrap_or(fi);
                    let slot = base_slot + field_offset as u8;
                    match binding_pat {
                        crate::ast::Pat::Ident(bind_span) => {
                            let bname = bind_span.text(source);
                            if bname != "_" {
                                ctx.locals.insert(bname, slot);
                            }
                        }
                        crate::ast::Pat::Wildcard => {}
                        crate::ast::Pat::StructVariant { path, fields: inner_fields }
                            if path.len() == 1 =>
                        {
                            // FLS §5.10.2: Nested struct pattern in parameter position.
                            // The inner struct's fields are laid out starting at `slot`.
                            let inner_name = path[0].text(source).to_owned();
                            ctx.bind_struct_fields_from_slot(
                                &inner_name,
                                slot,
                                inner_fields,
                            )?;
                        }
                        _ => {
                            return Err(LowerError::Unsupported(
                                "only ident, wildcard, and nested struct sub-patterns are \
                                 supported in struct parameter patterns"
                                    .into(),
                            ));
                        }
                    }
                }
                reg_idx += n_slots;
                continue;
            }
            return Err(LowerError::Unsupported(format!(
                "struct pattern parameter for unknown struct type `{type_name}`"
            )));
        }

        // FLS §5.10.4, §9.2: Tuple struct pattern parameter `Pair(a, b): Pair`.
        //
        // The tuple struct value arrives in consecutive registers (one per field),
        // matching the plain `p: Pair` calling convention. Spill each register to
        // a slot and bind each named field directly.
        //
        // Cache-line note: N fields → N × 4-byte `str` spill instructions,
        // same density as tuple and struct pattern parameters.
        if let crate::ast::ParamKind::TupleStruct { type_span, fields } = &param.kind {
            let type_name = type_span.text(source);
            let n_fields = *tuple_struct_defs.get(type_name).ok_or_else(|| {
                LowerError::Unsupported(format!(
                    "tuple struct pattern parameter for unknown type `{type_name}`"
                ))
            })?;
            // FLS §4.2: float fields arrive in d-registers, integer fields in x-registers.
            let float_field_tys = tuple_struct_float_field_types
                .get(type_name)
                .cloned()
                .unwrap_or_default();
            let n_int = float_field_tys.iter().filter(|t| t.is_none()).count()
                .max(n_fields.saturating_sub(float_field_tys.len()));
            let n_flt = float_field_tys.iter().filter(|t| t.is_some()).count();
            if n_fields > 0 && (reg_idx + n_int > 8 || freg_idx + n_flt > 8) {
                return Err(LowerError::Unsupported(
                    "tuple struct parameter exceeds ARM64 register window (>8 total registers)"
                        .into(),
                ));
            }
            let base_slot = ctx.alloc_slot()?;
            for _ in 1..n_fields {
                ctx.alloc_slot()?;
            }
            let mut p_int = 0usize;
            let mut p_flt = 0usize;
            for fi in 0..n_fields {
                let slot = base_slot + fi as u8;
                match float_field_tys.get(fi).copied().flatten() {
                    Some(IrTy::F64) => {
                        ctx.instrs.push(Instr::StoreF64 { src: (freg_idx + p_flt) as u8, slot });
                        ctx.slot_float_ty.insert(slot, IrTy::F64);
                        p_flt += 1;
                    }
                    Some(IrTy::F32) => {
                        ctx.instrs.push(Instr::StoreF32 { src: (freg_idx + p_flt) as u8, slot });
                        ctx.slot_float_ty.insert(slot, IrTy::F32);
                        p_flt += 1;
                    }
                    _ => {
                        ctx.instrs.push(Instr::Store { src: (reg_idx + p_int) as u8, slot });
                        p_int += 1;
                    }
                }
            }
            // Bind each positional name to its slot (FLS §5.10.4).
            for (fi, name_span) in fields.iter().enumerate() {
                let name = name_span.text(source);
                if name != "_" && fi < n_fields {
                    ctx.locals.insert(name, base_slot + fi as u8);
                }
            }
            // Track type for `.0`/`.1` field access and method dispatch.
            ctx.local_tuple_struct_types.insert(base_slot, type_name.to_owned());
            reg_idx += p_int;
            freg_idx += p_flt;
            continue;
        }

        let param_name = match &param.kind {
            crate::ast::ParamKind::Ident(s) => s.text(source),
            crate::ast::ParamKind::Tuple(_)
            | crate::ast::ParamKind::Struct { .. }
            | crate::ast::ParamKind::TupleStruct { .. } => {
                unreachable!() // handled above
            }
        };

        // FLS §15: Enum type parameters — `fn f(o: Opt)`.
        // FLS §11 / §6.12.2: Struct type parameters — `fn f(s: S)`.
        //
        // A struct value with N fields occupies N consecutive registers: one
        // register per field in declaration order. This matches the method
        // self-parameter calling convention in `lower_fn` (when `impl_type`
        // is set). The N registers are spilled to N consecutive stack slots
        // so that field access uses the same Load/Store paths as let-bindings.
        //
        // An enum value with max N fields occupies N+1 consecutive registers:
        // register reg_idx holds the discriminant; registers reg_idx+1..=reg_idx+N
        // hold the fields. All registers are spilled to consecutive stack slots so
        // that TupleStruct patterns can access fields via `base_slot + 1 + fi`.
        //
        // FLS §6.1.2:37–45: All spills are runtime store instructions.
        // Cache-line note: each spill is one `str` instruction (4 bytes).
        if let TyKind::Path(segs) = &param.ty.kind
            && segs.len() == 1
        {
            let type_name = segs[0].text(source);

            // FLS §11 / §6.12.2: Struct parameter — pass each field as a
            // separate register, matching the method self-parameter convention.
            //
            // For nested structs (e.g., `fn width(r: Rect)` where `Rect` has two
            // `Point` fields), the total slot count from `struct_sizes` is used
            // rather than the number of direct fields. This handles the case where
            // `Rect` has 2 declared fields but 4 total slots (2 per `Point`).
            //
            // FLS §4.11: Struct layout — fields are stored in declaration order.
            // FLS §6.11: Struct expressions — field initializers are evaluated in
            // declaration order and each occupies one or more consecutive slots.
            //
            // Cache-line note: N total slots emit N × 4-byte `str` spill instructions.
            // For a 2-field flat struct: 8 bytes (2 instructions, same as before).
            // For a 2-field nested struct like Rect: 16 bytes (4 instructions).
            if let Some(field_names) = struct_defs.get(type_name) {
                // Use total slot count (accounts for nested struct fields).
                let n_slots = struct_sizes.get(type_name).copied().unwrap_or(field_names.len());
                let regs_needed = n_slots.max(1); // unit structs use 0 slots but need 1 slot allocated
                if n_slots > 0 && reg_idx + n_slots > 8 {
                    return Err(LowerError::Unsupported(
                        "struct parameter exceeds ARM64 register window (>8 total registers)".into(),
                    ));
                }
                let base_slot = ctx.alloc_slot()?;
                for _ in 1..n_slots {
                    ctx.alloc_slot()?;
                }
                ctx.locals.insert(param_name, base_slot);
                ctx.local_struct_types.insert(base_slot, type_name.to_owned());
                // Spill each slot register to its stack slot.
                for fi in 0..n_slots {
                    ctx.instrs.push(Instr::Store {
                        src: (reg_idx + fi) as u8,
                        slot: base_slot + fi as u8,
                    });
                }
                reg_idx += regs_needed;
                continue;
            }

            if let Some(variants) = enum_defs.get(type_name) {
                let max_fields = variants.values().map(|(_, names)| names.len()).max().unwrap_or(0);
                let regs_needed = 1 + max_fields;
                if reg_idx + regs_needed > 8 {
                    return Err(LowerError::Unsupported(
                        "enum parameter exceeds ARM64 register window (>8 total registers)".into(),
                    ));
                }
                let base_slot = ctx.alloc_slot()?;
                for _ in 0..max_fields {
                    ctx.alloc_slot()?;
                }
                ctx.locals.insert(param_name, base_slot);
                ctx.local_enum_types.insert(base_slot, type_name.to_owned());
                // Spill discriminant register.
                ctx.instrs.push(Instr::Store { src: reg_idx as u8, slot: base_slot });
                // Spill field registers.
                for fi in 0..max_fields {
                    ctx.instrs.push(Instr::Store {
                        src: (reg_idx + fi + 1) as u8,
                        slot: base_slot + 1 + fi as u8,
                    });
                }
                reg_idx += regs_needed;
                continue;
            }

            // FLS §14.2, §10.1: Tuple struct parameter — `fn f(w: Wrap)`.
            //
            // A tuple struct with N fields is passed with integer fields in
            // x{reg_idx}..x{reg_idx+n_int-1} and float fields in
            // d{freg_idx}..d{freg_idx+n_flt-1} (ARM64 ABI, FLS §4.2).
            // Spill to N consecutive stack slots; register in `local_tuple_lens`
            // for `.0`/`.1` field access and `local_tuple_struct_types` for
            // method call dispatch (so `w.val()` resolves to `Wrap::val`).
            //
            // FLS §6.1.2:37–45: All spills are runtime store instructions.
            // Cache-line note: N × 4-byte `str` per parameter spill.
            if let Some(&n_fields) = tuple_struct_defs.get(type_name) {
                if n_fields > 0 {
                    let float_field_tys = tuple_struct_float_field_types
                        .get(type_name)
                        .cloned()
                        .unwrap_or_default();
                    let n_int = float_field_tys.iter().filter(|t| t.is_none()).count()
                        .max(n_fields.saturating_sub(float_field_tys.len()));
                    let n_flt = float_field_tys.iter().filter(|t| t.is_some()).count();
                    if reg_idx + n_int > 8 || freg_idx + n_flt > 8 {
                        return Err(LowerError::Unsupported(
                            "tuple struct parameter exceeds ARM64 register window (>8 total registers)".into(),
                        ));
                    }
                    let base_slot = ctx.alloc_slot()?;
                    for _ in 1..n_fields {
                        ctx.alloc_slot()?;
                    }
                    ctx.locals.insert(param_name, base_slot);
                    ctx.local_tuple_lens.insert(base_slot, n_fields);
                    ctx.local_tuple_struct_types.insert(base_slot, type_name.to_owned());
                    let mut p_int = 0usize;
                    let mut p_flt = 0usize;
                    for fi in 0..n_fields {
                        let slot = base_slot + fi as u8;
                        match float_field_tys.get(fi).copied().flatten() {
                            Some(IrTy::F64) => {
                                ctx.instrs.push(Instr::StoreF64 { src: (freg_idx + p_flt) as u8, slot });
                                ctx.slot_float_ty.insert(slot, IrTy::F64);
                                p_flt += 1;
                            }
                            Some(IrTy::F32) => {
                                ctx.instrs.push(Instr::StoreF32 { src: (freg_idx + p_flt) as u8, slot });
                                ctx.slot_float_ty.insert(slot, IrTy::F32);
                                p_flt += 1;
                            }
                            _ => {
                                ctx.instrs.push(Instr::Store { src: (reg_idx + p_int) as u8, slot });
                                p_int += 1;
                            }
                        }
                    }
                    reg_idx += p_int;
                    freg_idx += p_flt;
                } else {
                    // Zero-field tuple struct: allocate a dummy slot.
                    let base_slot = ctx.alloc_slot()?;
                    ctx.locals.insert(param_name, base_slot);
                    ctx.local_tuple_struct_types.insert(base_slot, type_name.to_owned());
                }
                continue;
            }
        }

        // FLS §4.5, §9.2: Array type parameter — `fn f(arr: [T; N])`.
        //
        // An array of N elements is passed as N consecutive integer registers
        // (x{reg_idx}..x{reg_idx+N-1}), one per element in index order.
        // Each register is spilled to a consecutive stack slot so that
        // `LoadIndexed` can address elements via `sp + (base_slot + i) * 8`.
        // Registering in `local_array_lens` lets `for x in arr` iterate over
        // the parameter exactly like a locally-bound array variable.
        //
        // FLS §6.1.2:37–45: All spill stores are runtime instructions.
        // Cache-line note: N × 4-byte `str` spill instructions; for N=4
        // all four spills fit in a single 16-byte instruction-cache slot.
        if let TyKind::Array { len, .. } = &param.ty.kind {
            let n = *len;
            if n > 0 && reg_idx + n > 8 {
                return Err(LowerError::Unsupported(
                    "array parameter exceeds ARM64 register window (>8 total registers)".into(),
                ));
            }
            let base_slot = ctx.alloc_slot()?;
            for _ in 1..n {
                ctx.alloc_slot()?;
            }
            ctx.locals.insert(param_name, base_slot);
            ctx.local_array_lens.insert(base_slot, n);
            for i in 0..n {
                ctx.instrs.push(Instr::Store {
                    src: (reg_idx + i) as u8,
                    slot: base_slot + i as u8,
                });
            }
            reg_idx += n;
            continue;
        }

        let param_ty = lower_ty(&param.ty, source, type_aliases)?;

        // FLS §4.2: f64 parameters are passed in d0–d7 (ARM64 float register bank).
        // Spill d{freg_idx} to the stack slot and register in float_locals so that
        // path expressions emit LoadF64Slot rather than the integer Load.
        if param_ty == IrTy::F64 {
            if freg_idx >= 8 {
                return Err(LowerError::Unsupported(
                    "functions with more than 8 float parameters (exceeds ARM64 float register window)".into(),
                ));
            }
            let slot = ctx.alloc_slot()?;
            ctx.instrs.push(Instr::StoreF64 { src: freg_idx as u8, slot });
            ctx.float_locals.insert(param_name, slot);
            freg_idx += 1;
            continue;
        }

        // FLS §4.2: f32 parameters are passed in s0–s7.
        // Spill s{freg_idx} to the stack slot and register in float32_locals.
        if param_ty == IrTy::F32 {
            if freg_idx >= 8 {
                return Err(LowerError::Unsupported(
                    "functions with more than 8 float parameters (exceeds ARM64 float register window)".into(),
                ));
            }
            let slot = ctx.alloc_slot()?;
            ctx.instrs.push(Instr::StoreF32 { src: freg_idx as u8, slot });
            ctx.float32_locals.insert(param_name, slot);
            freg_idx += 1;
            continue;
        }

        // FLS §9: i32 and bool parameters — one register each.
        if reg_idx >= 8 {
            return Err(LowerError::Unsupported(
                "functions with more than 8 parameters (exceeds ARM64 register window)".into(),
            ));
        }
        // FLS §4.3: bool is passed as a 32-bit integer register on ARM64.
        // FLS §4.1: i32 parameters occupy one 64-bit register (x0–x7).
        // FLS §4.1: All primitive integer types and bool are supported as
        // parameters. Each uses one 64-bit ARM64 register (x0–x7).
        if !matches!(param_ty, IrTy::I32 | IrTy::Bool | IrTy::U32 | IrTy::FnPtr) {
            return Err(LowerError::Unsupported(
                "parameter type other than i32/bool/u32/i64/u64/usize/isize/i8/i16/u8/u16/fn ptr/f64/f32".into(),
            ));
        }
        let slot = ctx.alloc_slot()?;
        ctx.locals.insert(param_name, slot);
        // Spill parameter register reg_idx (arm64 x{reg_idx}) to its stack slot.
        ctx.instrs.push(Instr::Store { src: reg_idx as u8, slot });
        // FLS §4.9: Track function pointer parameters so Call lowering can
        // emit CallIndirect rather than a direct `bl {name}`.
        if param_ty == IrTy::FnPtr {
            ctx.local_fn_ptr_slots.insert(slot);
        }
        reg_idx += 1;
    }

    // FLS §10.1: For `&mut self` methods with unit return type, emit `RetFields`
    // instead of `Ret(Unit)`. This writes modified self fields back to the caller
    // via x0..x{N-1} on return. The caller uses `CallMut` to store them back.
    //
    // Limitation: early `return` expressions inside the body still emit `Ret(Unit)`,
    // bypassing the write-back. Only methods that terminate via their tail expression
    // are fully correct at this milestone.
    //
    // FLS §10.1 AMBIGUOUS: The spec does not define the calling convention for
    // `&mut self` in terms of register passing. Galvanic uses a value-copy convention
    // (fields passed as registers, written back on return) for simplicity.
    if self_kind == Some(SelfKind::RefMut) {
        // Determine the number of self fields. The impl_type is guaranteed Some
        // when self_kind is Some (enforced by the parser / call site).
        let type_name = impl_type.expect("impl_type must be set when self_kind is RefMut");
        if enum_defs.contains_key(type_name) {
            return Err(LowerError::Unsupported(
                "&mut self methods on enum types not yet supported".into(),
            ));
        }
        let n_fields = if let Some(f) = struct_defs.get(type_name) {
            f.len() as u8
        } else if let Some(&n) = tuple_struct_defs.get(type_name) {
            // FLS §14.2, §10.1: &mut self on tuple struct — N consecutive slots.
            n as u8
        } else {
            0
        };
        // Lower the body, capturing the tail value.
        // Fields in the method's local slots will have been updated by body code.
        let tail_val = ctx.lower_block_to_value(body, &ret_ty)?;
        if ret_ty == IrTy::Unit {
            // Unit return: emit RetFields to write back modified fields.
            // Emit RetFields: loads each field from slot 0..n_fields-1 into x0..x{N-1}
            // then performs the normal epilogue.
            ctx.instrs.push(Instr::RetFields { base_slot: 0, n_fields });
        } else {
            // Scalar return: emit RetFieldsAndValue — fields in x0..x{N-1}, value in x{N}.
            //
            // FLS §10.1: &mut self methods may return any type. Galvanic extends the
            // write-back convention: fields in x0..x{N-1}, scalar return in x{N}.
            //
            // FLS §10.1 AMBIGUOUS: The spec does not define the calling convention for
            // &mut self with non-unit return type. This extension is consistent with the
            // existing register-packing convention.
            let val_reg = ctx.val_to_reg(tail_val)?;
            ctx.instrs.push(Instr::RetFieldsAndValue { base_slot: 0, n_fields, val_reg });
        }
    } else if let Some(ref struct_name) = struct_ret_name {
        // FLS §9, §10.1: Function returning a named struct type.
        //
        // The tail expression may be a struct literal, if-else, block, or variable
        // path — any form handled by lower_struct_expr_into. All statements are
        // lowered first, then the tail is lowered into N consecutive return slots,
        // then RetFields emits the fields in x0..x{N-1}.
        //
        // ARM64 ABI: multiple return values packed into x0..x{N-1} (small structs).
        // The call site uses CallMut-style write-back to store them into the
        // destination variable's slots.
        //
        // FLS §10.1 AMBIGUOUS: The spec does not define the calling convention for
        // returning struct types from associated functions. Galvanic uses the same
        // register-packing convention as &mut self (fields in x0..x{N-1}).
        //
        // Cache-line note: lower_struct_expr_into emits N store instructions per
        // struct-literal arm + the RetFields N-ldr sequence = 2N instructions total.
        let n_fields = struct_defs.get(struct_name.as_str())
            .ok_or_else(|| LowerError::Unsupported(format!("unknown struct `{struct_name}`")))?
            .len();

        // Lower all statements.
        for stmt in &body.stmts {
            ctx.lower_stmt(stmt)?;
        }

        // The tail expression produces a struct value.
        //
        // FLS §6.11: The tail may be a struct literal, an if-else expression,
        // a block, or a variable path — any expression yielding the declared
        // struct type.
        // FLS §6.17: If-else is the canonical way to conditionally return a struct.
        let tail = body.tail.as_deref().ok_or_else(|| {
            LowerError::Unsupported(format!(
                "function returning `{struct_name}` must end with a struct expression"
            ))
        })?;

        // Allocate consecutive slots for the return struct fields.
        // FLS §6.11: Struct fields are stored in declaration order.
        // Cache-line note: N consecutive 8-byte slots = N×8 bytes on the stack.
        let base_slot = ctx.alloc_slot()?;
        for _ in 1..n_fields {
            ctx.alloc_slot()?;
        }

        // Delegate to lower_struct_expr_into, which handles struct literals,
        // if/else, blocks, and variable paths (FLS §6.11, §6.17, §6.4, §6.3).
        ctx.lower_struct_expr_into(tail, base_slot, n_fields, struct_name)?;

        // Emit RetFields: loads fields from base_slot..base_slot+n_fields-1
        // into x0..x{N-1} before the epilogue.
        ctx.instrs.push(Instr::RetFields { base_slot, n_fields: n_fields as u8 });
    } else if let Some(ref enum_name) = enum_ret_name {
        // FLS §9, §15: Free function returning an enum type.
        //
        // The callee stores the enum (discriminant + fields) into consecutive
        // stack slots allocated here, then returns them in x0..x{1+max_fields-1}
        // via RetFields. The caller uses CallMut-style write-back to receive them.
        //
        // ARM64 ABI: up to 8 integer return registers (x0..x7). For the common
        // case of 1-field enums (e.g. `Option<i32>-like`), two registers suffice.
        //
        // FLS §15 AMBIGUOUS: The spec does not define a calling convention for
        // enum-returning functions. Galvanic extends the register-packing
        // convention already used for struct returns and enum parameter passing:
        // discriminant in x0, field[0] in x1, field[1] in x2, etc.
        //
        // Cache-line note: (1 + max_fields) × 4-byte ldr in RetFields sequence.
        let max_fields = enum_defs
            .get(enum_name.as_str())
            .map(|v| v.values().map(|(_, names)| names.len()).max().unwrap_or(0))
            .unwrap_or(0);
        let n_ret = 1 + max_fields as u8;

        // Lower all statements.
        for stmt in &body.stmts {
            ctx.lower_stmt(stmt)?;
        }

        // Lower the tail expression into return slots.
        let tail = body.tail.as_deref().ok_or_else(|| {
            LowerError::Unsupported(format!(
                "function returning `{enum_name}` must have a tail expression"
            ))
        })?;

        // Allocate consecutive slots for the enum return value:
        // slot ret_base = discriminant, slot ret_base+1..ret_base+max_fields = fields.
        let ret_base = ctx.alloc_slot()?;
        for _ in 0..max_fields {
            ctx.alloc_slot()?;
        }

        ctx.lower_enum_expr_into(tail, ret_base, max_fields)?;

        // RetFields: load discriminant + fields into x0..x{n_ret-1} before epilogue.
        ctx.instrs.push(Instr::RetFields { base_slot: ret_base, n_fields: n_ret });
    } else if let Some(n_elems) = tuple_ret_n {
        // FLS §6.10, §9: Function returning a tuple type.
        //
        // The callee stores each element in consecutive stack slots and returns
        // them in x0..x{N-1} via RetFields. The caller (in a `let` tuple pattern
        // binding) uses CallMut to write x0..x{N-1} into the destination slots.
        //
        // FLS §6.10 AMBIGUOUS: The spec does not define a tuple return calling
        // convention. Galvanic extends the struct-return convention: element[i]
        // returns in x{i}, matching the register-packing convention for struct
        // and enum returns.
        //
        // ARM64 ABI: up to 8 result registers (x0..x7). At this milestone only
        // tuples with ≤8 scalar elements are supported.
        //
        // Cache-line note: N element stores + RetFields N loads = 2N instructions.
        // For a 2-element tuple this is 8 bytes — fits in two ARM64 instruction slots.

        // Lower all statements.
        for stmt in &body.stmts {
            ctx.lower_stmt(stmt)?;
        }

        // The tail expression produces a tuple value via lower_tuple_expr_into.
        // FLS §6.10: A tuple expression `(e0, e1, ...)` evaluates each element
        // left-to-right and produces a tuple value.
        // FLS §6.17: If/else expressions may also produce tuple values when both
        // branches yield the same tuple type.
        let tail = body.tail.as_deref().ok_or_else(|| {
            LowerError::Unsupported(
                "function returning a tuple must end with a tuple expression".into(),
            )
        })?;

        // Allocate N consecutive stack slots for the tuple elements.
        // FLS §6.10: Tuple elements are in declaration order.
        // Cache-line note: N consecutive 8-byte slots = N×8 bytes on the stack.
        // For a 2-element tuple: 16 bytes, same as a 2-field struct.
        let base_slot = ctx.alloc_slot()?;
        for _ in 1..n_elems {
            ctx.alloc_slot()?;
        }

        // Delegate to lower_tuple_expr_into, which handles tuple literals,
        // if/else, and block expressions (FLS §6.10, §6.17, §6.4).
        ctx.lower_tuple_expr_into(tail, base_slot, n_elems)?;

        // RetFields: load elements from base_slot..base_slot+N-1 into x0..x{N-1}.
        ctx.instrs.push(Instr::RetFields { base_slot, n_fields: n_elems });
    } else {
        ctx.lower_block(body, &ret_ty)?;
    }

    let body_instrs = ctx.instrs;
    let stack_slots = ctx.next_slot;
    let saves_lr = ctx.has_calls;
    let next_label = ctx.next_label;
    let pending_closures = ctx.pending_closures;
    let pending_trampolines = ctx.pending_trampolines;
    let float_consts = ctx.float_consts;
    let float32_consts = ctx.float32_consts;
    Ok((IrFn { name, ret_ty, body: body_instrs, stack_slots, saves_lr, float_consts, float32_consts }, pending_closures, pending_trampolines, next_label))
}

// ── Type lowering ────────────────────────────────────────────────────────────

/// Lower a type expression to an `IrTy`.
///
/// FLS §4: Types. Supports `i32`, `bool`, and `()`.
///
/// FLS §4.3: The boolean type `bool` has two values, `true` (1) and `false`
/// (0). On ARM64, booleans are passed and returned in 32-bit integer registers
/// (the same layout as `i32`), so `bool` maps to `IrTy::I32` in the IR.
/// This is consistent with `LitBool` materialisation (FLS §6.1.3), which
/// already represents `true`/`false` as immediates 1/0.
/// Parse a char literal span text (including surrounding single quotes) to its
/// Unicode scalar value (code point) as a `u32`.
///
/// FLS §2.4.5: A character literal is a `char`-typed expression whose value
/// is a Unicode scalar value represented within single-quote delimiters.
///
/// Supported forms:
/// - Simple ASCII: `'A'` → 65
/// - Common escape sequences: `'\n'` → 10, `'\t'` → 9, `'\r'` → 13,
///   `'\\'` → 92, `'\''` → 39, `'\"'` → 34, `'\0'` → 0
/// - Hex escapes: `'\x7F'` → 127
/// - Unicode escapes: `'\u{1F600}'` → 128512
///
/// Parse the text of a float literal token into an `f64` value.
///
/// FLS §2.4.4.2: Float literal syntax:
///   - Optional sign (handled by unary `-` at the expression level)
///   - Decimal digits, optional decimal point, optional exponent
///   - Optional suffix `_f32` or `_f64`
///   - Underscores allowed as digit separators (e.g. `1_000.0`)
///
/// FLS §2.4.4.2: The suffix determines the type (`f32` or `f64`); galvanic
/// only supports `f64` at this milestone. The value is the same regardless
/// of suffix — suffix affects type, not magnitude.
///
/// FLS §2.4.4.2 AMBIGUOUS: The spec does not specify the handling of
/// NaN/infinity literals (Rust has none) or hexadecimal float literals
/// (not supported by the lexer at this milestone).
fn parse_float_value(text: &str) -> Result<f64, LowerError> {
    // Strip suffix: _f32 or _f64 must be checked longest-first.
    let nosuffix = text
        .strip_suffix("_f64")
        .or_else(|| text.strip_suffix("_f32"))
        .unwrap_or(text);
    // Strip underscores used as digit separators.
    let cleaned: String = nosuffix.chars().filter(|&c| c != '_').collect();
    cleaned.parse::<f64>().map_err(|_| {
        LowerError::Unsupported(format!("cannot parse float literal: `{text}`"))
    })
}

/// Parse the text of a float literal token into an `f32` value.
///
/// FLS §2.4.4.2: Same syntax as f64 but with `_f32` suffix (or forced by
/// context). Underscores are stripped from the numeric part.
fn parse_float32_value(text: &str) -> Result<f32, LowerError> {
    let nosuffix = text
        .strip_suffix("_f64")
        .or_else(|| text.strip_suffix("_f32"))
        .unwrap_or(text);
    let cleaned: String = nosuffix.chars().filter(|&c| c != '_').collect();
    cleaned.parse::<f32>().map_err(|_| {
        LowerError::Unsupported(format!("cannot parse f32 literal: `{text}`"))
    })
}

/// FLS §2.4.5: "A character literal is a character within single-quotes."
/// FLS §4.2 AMBIGUOUS: The spec refers to `char` as "the Unicode scalar value
/// type" but does not give a precise section number in the FLS TOC shown here.
/// Galvanic maps char literals to their Unicode code point as a `u32`.
fn parse_char_value(text: &str) -> Result<u32, LowerError> {
    // FLS §2.4.1: Byte literals have the form `b'...'`.
    // FLS §2.4.5: Char literals have the form `'...'`.
    // Strip the optional `b` prefix and surrounding single quotes.
    let inner = text
        .strip_prefix('b')
        .unwrap_or(text)
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .ok_or_else(|| {
            LowerError::Unsupported(format!("malformed char/byte literal: {text}"))
        })?;

    if !inner.starts_with('\\') {
        // Plain (non-escaped) character — the source text contains the raw Unicode char.
        let ch = inner.chars().next().ok_or_else(|| {
            LowerError::Unsupported(format!("empty char literal: {text}"))
        })?;
        return Ok(ch as u32);
    }

    // Escape sequence: inner starts with `\`.
    // `escaped` is the text after the leading backslash.
    let escaped = &inner[1..];
    match escaped.chars().next() {
        Some('n')  => Ok(10),   // FLS §2.4.5: `\n` → LINE FEED (U+000A)
        Some('r')  => Ok(13),   // FLS §2.4.5: `\r` → CARRIAGE RETURN (U+000D)
        Some('t')  => Ok(9),    // FLS §2.4.5: `\t` → CHARACTER TABULATION (U+0009)
        Some('\\') => Ok(92),   // FLS §2.4.5: `\\` → REVERSE SOLIDUS (U+005C)
        Some('\'') => Ok(39),   // FLS §2.4.5: `\'` → APOSTROPHE (U+0027)
        Some('"')  => Ok(34),   // FLS §2.4.5: `\"` → QUOTATION MARK (U+0022)
        Some('0')  => Ok(0),    // FLS §2.4.5: `\0` → NULL (U+0000)
        Some('x')  => {
            // FLS §2.4.5: `\xNN` — ASCII code point in [0x00, 0x7F].
            // The two hex digits always follow immediately after `x`.
            let hex = escaped.get(1..3).ok_or_else(|| {
                LowerError::Unsupported(format!("malformed \\x escape in char literal: {text}"))
            })?;
            u32::from_str_radix(hex, 16).map_err(|_| {
                LowerError::Unsupported(format!("malformed \\x hex value in char literal: {text}"))
            })
        }
        Some('u')  => {
            // FLS §2.4.5: `\u{N..}` — Unicode code point, 1–6 hex digits.
            let braced = escaped.get(1..).ok_or_else(|| {
                LowerError::Unsupported(format!("malformed \\u escape in char literal: {text}"))
            })?;
            let hex = braced
                .strip_prefix('{')
                .and_then(|s| s.strip_suffix('}'))
                .ok_or_else(|| {
                    LowerError::Unsupported(format!(
                        "malformed \\u{{}} escape in char literal: {text}"
                    ))
                })?;
            u32::from_str_radix(hex, 16).map_err(|_| {
                LowerError::Unsupported(format!(
                    "malformed \\u{{}} code point in char literal: {text}"
                ))
            })
        }
        other => Err(LowerError::Unsupported(format!(
            "unknown escape sequence \\{other:?} in char literal: {text}"
        ))),
    }
}

/// Compute the UTF-8 byte length of a string literal from its source text.
///
/// FLS §2.4.6: String literals are enclosed in double quotes.  Raw string
/// literals are enclosed in `r"..."` or `r#"..."#` (with matching hashes).
/// Escape sequences count as the number of bytes they produce, not the number
/// of source characters they occupy.
///
/// Supported escape sequences (FLS §2.4.6.1):
///   `\n` → 1 byte (0x0A)   `\r` → 1 byte (0x0D)   `\t` → 1 byte (0x09)
///   `\\` → 1 byte (0x5C)   `\"` → 1 byte (0x22)   `\0` → 1 byte (0x00)
///   `\xNN` → 1 byte        `\u{NNNNNN}` → 1–4 UTF-8 bytes
///   `\<newline>` → 0 bytes (line-continuation, trims following whitespace)
///
/// Raw string literals contain no escape sequences; each source char is
/// counted by its UTF-8 byte length.
fn parse_str_byte_len(text: &str) -> Result<usize, LowerError> {
    // FLS §2.4.2: Byte string literals begin with `b"` or `br"`.
    // FLS §2.4.2.2: Raw byte string literals begin with `br"` or `br#"`.
    // Strip the leading `b` so the rest of the function handles `"..."` / `r"..."`.
    let text = text.strip_prefix('b').unwrap_or(text);

    // FLS §2.4.6.2: Raw string literals begin with `r"` or `r#"`.
    // Key property: raw strings contain NO escape sequences — backslash is
    // a literal character.  `r"hello\n"` is 7 bytes, not 6.
    // Strip raw-string prefix if present: `r"..."` or `r##"..."##`.
    // Count the number of `#` characters (0–255).
    if let Some(rest) = text.strip_prefix('r') {
        let hashes = rest.bytes().take_while(|&b| b == b'#').count();
        let inner_start = 1 + hashes; // skip opening '"'
        let inner_end = rest.len() - 1 - hashes; // strip closing '"' + hashes
        let inner = rest.get(inner_start..inner_end).ok_or_else(|| {
            LowerError::Unsupported(format!("malformed raw string literal: {text}"))
        })?;
        // FLS §2.4.6.2: Raw string — no escape processing.  Count UTF-8 bytes directly.
        return Ok(inner.len());
    }

    // Regular string literal: `"..."`.
    let inner = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or_else(|| {
            LowerError::Unsupported(format!("malformed string literal: {text}"))
        })?;

    let mut bytes = 0usize;
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            bytes += ch.len_utf8();
            continue;
        }
        match chars.next() {
            Some('n')  => bytes += 1,  // \n → 0x0A
            Some('r')  => bytes += 1,  // \r → 0x0D
            Some('t')  => bytes += 1,  // \t → 0x09
            Some('\\') => bytes += 1,  // \\ → 0x5C
            Some('"')  => bytes += 1,  // \" → 0x22
            Some('0')  => bytes += 1,  // \0 → 0x00
            Some('x')  => {
                // \xNN — always 1 byte (ASCII, validated elsewhere)
                chars.next(); // skip first hex digit
                chars.next(); // skip second hex digit
                bytes += 1;
            }
            Some('u') => {
                // \u{NNNNNN} — skip until closing '}'
                let mut code_point = 0u32;
                // consume '{'
                chars.next();
                for ch2 in chars.by_ref() {
                    if ch2 == '}' { break; }
                    code_point = code_point * 16
                        + ch2.to_digit(16).unwrap_or(0);
                }
                // Count UTF-8 bytes for the encoded code point.
                if let Some(c) = char::from_u32(code_point) {
                    bytes += c.len_utf8();
                } else {
                    bytes += 3; // replacement character (3 bytes) as fallback
                }
            }
            Some('\n') => {
                // Line-continuation: consume leading whitespace on next line.
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
                // Contributes 0 bytes to the string.
            }
            Some(other) => {
                return Err(LowerError::Unsupported(format!(
                    "unknown escape \\{other} in string literal: {text}"
                )));
            }
            None => {
                return Err(LowerError::Unsupported(format!(
                    "unterminated escape in string literal: {text}"
                )));
            }
        }
    }
    Ok(bytes)
}

fn lower_ty(
    ty: &crate::ast::Ty,
    source: &str,
    type_aliases: &HashMap<String, IrTy>,
) -> Result<IrTy, LowerError> {
    match &ty.kind {
        TyKind::Unit => Ok(IrTy::Unit),
        TyKind::Path(segments) if segments.len() == 1 => {
            match segments[0].text(source) {
                // FLS §4.1: Signed integer types. i8/i16/i64/isize all use
                // signed 64-bit registers on ARM64. Width truncation for
                // narrower types is deferred (FLS §4.1 AMBIGUOUS on ABI layout).
                "i32" | "i8" | "i16" | "i64" | "isize" => Ok(IrTy::I32),
                // FLS §4.3: bool is a distinct type in the IR so that `!` can
                // emit logical NOT (eor, XOR with 1) rather than bitwise NOT (mvn).
                // On ARM64, bool and i32 share the same register layout (0/1 as i64),
                // but the semantics of `!` differ.
                "bool" => Ok(IrTy::Bool),
                // FLS §4.1: Unsigned integer types. u8/u16/u32/u64/usize all
                // use 64-bit registers on ARM64. Unsigned division uses `udiv`
                // and unsigned right shift uses `lsr` (see IrBinOp::UDiv/UShr).
                // Width truncation for narrower types is deferred.
                "u8" | "u16" | "u32" | "u64" | "usize" => Ok(IrTy::U32),
                // FLS §2.4.5: The `char` type is a Unicode scalar value — a u32
                // in the range [0, 0x10FFFF]. On ARM64, char is stored in a
                // 64-bit general-purpose register identical to `u32`.
                // Comparison of chars uses unsigned ordering (code point ordering).
                //
                // FLS §2.4.5 AMBIGUOUS: The FLS TOC does not list a dedicated
                // section for the char type (unlike §4.1 for integers or §4.3
                // for bool). The char type is described in §2.4.5 (character
                // literals) rather than in its own type section.
                "char" => Ok(IrTy::U32),
                // FLS §4.14 / §2.4.6: The unsized type `str` (as the inner type of `&str`).
                // Galvanic materialises `&str` values as their byte length (an i32 immediate)
                // at this milestone.  The pointer half is deferred.
                "str" => Ok(IrTy::I32),
                // FLS §4.2: The 64-bit floating-point type.
                // ARM64: float values live in the `d{N}` register bank.
                // Stack slots are 8 bytes — same layout as integer/pointer types.
                "f64" => Ok(IrTy::F64),
                // FLS §4.2: The 32-bit floating-point type.
                // ARM64: values live in the `s{N}` register bank.
                // Stack slots are 8 bytes — same size, lower 4 bytes used.
                "f32" => Ok(IrTy::F32),
                // FLS §4.10: Type aliases. If the name is a registered alias,
                // return the alias's resolved IrTy directly.
                name => {
                    if let Some(&irt) = type_aliases.get(name) {
                        return Ok(irt);
                    }
                    Err(LowerError::Unsupported(format!("type `{name}`")))
                }
            }
        }
        // FLS §4.8: Reference types `&T` and `&mut T`. A reference is a pointer —
        // an 8-byte address on ARM64. Galvanic represents reference-typed parameters
        // and locals using `IrTy::I32` (a 64-bit register), matching the inner type's
        // IR representation. The pointer IS the value in the register; `*x` dereferences
        // it via `LoadPtr { dst, src }`.
        //
        // FLS §4.8: "A reference type is a kind of pointer type."
        // Cache-line note: references occupy one 8-byte register slot — same as i32/i64.
        TyKind::Ref { inner, .. } => lower_ty(inner, source, type_aliases),
        // FLS §4.4: Tuple types — used only in parameter position for tuple patterns.
        // A tuple type as a scalar return/local is not yet supported.
        TyKind::Tuple(_) => Err(LowerError::Unsupported(
            "tuple type in scalar context (use tuple pattern parameter instead)".into(),
        )),
        // FLS §4.9: Function pointer types `fn(T1, ...) -> R`.
        // A function pointer is a 64-bit address — one register, like a scalar.
        TyKind::FnPtr { .. } => Ok(IrTy::FnPtr),
        // FLS §4.5: Array types `[T; N]`.
        // An array is not a scalar — it occupies N consecutive 8-byte slots.
        // In a scalar context (e.g. function parameter `a: [T; N]`) we return
        // the element type as a best-effort hint; the let-binding path handles
        // the full array allocation via `local_array_lens` and does not consult
        // `lower_ty` for the array case (it returns early on the array literal
        // initializer). Aggregate array parameters are deferred.
        TyKind::Array { elem, .. } => lower_ty(elem, source, type_aliases),
        _ => Err(LowerError::Unsupported("complex type".into())),
    }
}

// ── Loop context ─────────────────────────────────────────────────────────────

/// Context for a loop being lowered.
///
/// Pushed onto `LowerCtx::loop_stack` when entering a loop expression and
/// popped when the loop body has been fully lowered.
///
/// `break` consults the top entry for `exit_label` to branch past the loop.
/// `continue` consults `header_label` to jump back to the loop top.
///
/// FLS §6.15.2: Infinite loop expressions.
/// FLS §6.15.3: While loop expressions.
/// FLS §6.15.6: Break expressions.
/// FLS §6.15.7: Continue expressions.
struct LoopCtx {
    /// The optional block label for this loop (`'name` from `'name: loop …`).
    /// Used by `break 'name` and `continue 'name` to target a specific loop
    /// rather than the innermost one. FLS §6.15.6, §6.15.7.
    label: Option<String>,
    /// Label at the top of the loop. Target for `continue` and the back-edge.
    header_label: u32,
    /// Label immediately after the loop. Target for `break`.
    exit_label: u32,
    /// Stack slot for the loop's result value, allocated when the loop body
    /// contains a `break <value>` expression. Only `loop` expressions support
    /// break-with-value (FLS §6.15.6). `None` for `while` and `for` loops.
    break_slot: Option<u8>,
    /// The expected result type of the loop expression. Needed so that
    /// `break <expr>` can lower the value with the correct type context.
    /// Matches the `ret_ty` passed to `lower_expr` for the `loop` node.
    break_ret_ty: IrTy,
}

// ── Lowering context ─────────────────────────────────────────────────────────

/// Mutable state threaded through the lowering of a single function body.
///
/// Tracks the instruction buffer, virtual register counter, stack slot
/// counter, label counter, and the local variable map. All instructions for
/// one function are accumulated here and transferred to `IrFn::body` at the end.
///
/// FLS §8.1: Each `let` binding allocates a new stack slot and registers
/// the variable name in `locals`. Path expressions consult `locals` to
/// find the slot to load from.
struct LowerCtx<'src> {
    source: &'src str,
    instrs: Vec<Instr>,
    next_reg: u8,
    /// Next stack slot index. Slot `s` maps to byte offset `s * 8` on the
    /// stack frame. The frame size is rounded up to 16 bytes in codegen.
    next_slot: u8,
    /// Next label ID for branch targets.
    ///
    /// FLS §6.17: if expressions require unique labels for else and end
    /// targets. Labels are monotonically increasing per function.
    next_label: u32,
    /// Maps local variable names to their stack slot indices.
    ///
    /// FLS §8.1: Let statements introduce bindings into the current scope.
    /// FLS §6.3: Path expressions are resolved here before emitting Load.
    ///
    /// Limitation: this flat map does not model nested scopes. Variables
    /// introduced inside an if branch remain visible after it. Proper lexical
    /// scoping is deferred to a future milestone.
    locals: HashMap<&'src str, u8>,
    /// Whether this function emits any `Call` instructions.
    ///
    /// Set to `true` when `Instr::Call` is pushed. Used to set `IrFn::saves_lr`
    /// so codegen knows to save/restore x30 around calls.
    ///
    /// FLS §6.12.1: Call expressions make a function non-leaf; the link
    /// register must be preserved so the function can return correctly.
    has_calls: bool,

    /// Maps float (`f64`) local variable names to their stack slot indices.
    ///
    /// FLS §4.2: Float locals are stored in 8-byte stack slots, identical in
    /// layout to integer locals. They are loaded/stored with float-register
    /// instructions (`ldr d{N}` / `str d{N}`) rather than integer ones.
    ///
    /// Kept separate from `locals` so that path-expression lowering can choose
    /// the right load instruction (`LoadF64Slot` vs `Load`) without tracking
    /// per-slot type information across the entire `locals` map.
    float_locals: HashMap<&'src str, u8>,

    /// Accumulated float constants for the current function.
    ///
    /// FLS §2.4.4.2: Float literals. Each entry is the raw IEEE 754 bit
    /// pattern of a `f64` constant referenced by index in the function body
    /// via `Instr::LoadF64Const`. Moved into `IrFn::float_consts` when the
    /// function finishes lowering.
    float_consts: Vec<u64>,

    /// Maps float (`f32`) local variable names to their stack slot indices.
    ///
    /// FLS §4.2: f32 locals use 8-byte stack slots (same as all other types)
    /// but are loaded/stored with single-precision float-register instructions
    /// (`ldr s{N}` / `str s{N}`). Kept separate from `float_locals` (f64) so
    /// path-expression lowering can choose the right instruction.
    float32_locals: HashMap<&'src str, u8>,

    /// Accumulated f32 constants for the current function.
    ///
    /// FLS §2.4.4.2: Float literal with `_f32` suffix. Each entry is the raw
    /// IEEE 754 bit pattern of an `f32` constant referenced by index in the
    /// function body via `Instr::LoadF32Const`. Moved into `IrFn::float32_consts`
    /// when the function finishes lowering.
    float32_consts: Vec<u32>,
    /// Stack of enclosing loop contexts.
    ///
    /// Each entry corresponds to one loop currently being lowered. The top
    /// (last) entry is the innermost loop — the target of an unqualified
    /// `break` or `continue`.
    ///
    /// FLS §6.15.6: "A break expression without a label exits the innermost
    /// enclosing loop expression."
    /// FLS §6.15.7: "A continue expression without a label continues the
    /// innermost enclosing loop expression."
    loop_stack: Vec<LoopCtx>,

    /// The return type of the current function.
    ///
    /// Stored so that `return` expressions (FLS §6.19) can lower the returned
    /// value using the correct type, regardless of the expression context type
    /// (`ret_ty`) passed to `lower_expr` at the point of the `return`.
    ///
    /// For example, `return 42` appearing inside a unit-typed `if` body still
    /// needs to lower `42` as `IrTy::I32` if the enclosing function returns i32.
    fn_ret_ty: IrTy,

    /// Struct type definitions: maps struct name → field names in declaration order.
    ///
    /// FLS §14: Struct definitions. Used to look up field indices during
    /// struct literal construction and field access lowering.
    ///
    /// Cache-line note: field index determines the stack slot offset from the
    /// struct's base slot. Field `i` is at `base_slot + i`, each slot 8 bytes.
    struct_defs: &'src HashMap<String, Vec<String>>,

    /// Tuple struct definitions: maps struct name → field count.
    ///
    /// FLS §14.2: Tuple struct items. When `let p = Point(a, b)` is lowered
    /// and `Point` is a known tuple struct, N consecutive stack slots are
    /// allocated and the base slot is registered in `local_tuple_lens`.
    /// Subsequent `.0`, `.1` field accesses use the existing tuple field
    /// access path (FLS §6.10).
    ///
    /// Cache-line note: same layout as anonymous tuples — N consecutive
    /// 8-byte slots, with `.i` at `base_slot + i`.
    tuple_struct_defs: &'src HashMap<String, usize>,

    /// Per-field float type for tuple structs.
    ///
    /// FLS §4.2, §14.2: `None` = integer/bool field; `Some(IrTy::F64)` = f64;
    /// `Some(IrTy::F32)` = f32. Used to select StoreF64/StoreF32 during
    /// construction, StoreF64/StoreF32 during parameter spilling, and
    /// LoadF64Slot/LoadF32Slot when loading fields to pass as arguments.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    tuple_struct_float_field_types: &'src HashMap<String, Vec<Option<IrTy>>>,

    /// Enum type definitions: maps enum name → (variant name → discriminant).
    ///
    /// FLS §15: Enumerations. Unit variants are assigned integer discriminants
    /// (0, 1, 2, ...) in declaration order. Used to resolve path expressions
    /// (`Color::Red`) and path patterns (`Color::Red` in match arms) to their
    /// integer discriminant values.
    ///
    /// Cache-line note: enum values are represented as i32, occupying one
    /// 8-byte stack slot — identical to any other i32 local.
    enum_defs: &'src EnumDefs,

    /// Per-field float type for enum tuple variants.
    ///
    /// FLS §4.2, §15: Maps enum_name → variant_name → [field_float_types].
    /// `None` = integer/bool field; `Some(IrTy::F64)` = f64; `Some(IrTy::F32)` = f32.
    /// Used during variant construction (StoreF64/StoreF32 vs Store) and
    /// TupleStruct pattern binding (LoadF64Slot/LoadF32Slot vs Load).
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    enum_variant_float_field_types: &'src HashMap<String, HashMap<String, Vec<Option<IrTy>>>>,

    /// Method self-kind registry: maps mangled method name → SelfKind.
    ///
    /// FLS §10.1: `&mut self` methods use a different calling convention
    /// (write-back via `CallMut`) than `&self` or `self` methods (`Call`).
    /// At the call site, this registry is consulted to select the right
    /// instruction.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    method_self_kinds: &'src HashMap<String, SelfKind>,

    /// `&mut self` scalar-return registry: mangled names of `&mut self` methods
    /// that return a scalar (non-unit, non-struct, non-enum) type.
    ///
    /// FLS §10.1: `&mut self` methods may return any type. When the return type
    /// is scalar, the call site emits `CallMutReturn` instead of `CallMut` to
    /// capture the scalar return value from x{N} (after the field write-backs).
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    mut_self_scalar_return_fns: &'src std::collections::HashSet<String>,

    /// Struct-returning associated function registry: mangled name → struct type name.
    ///
    /// FLS §10.1: Associated functions that return a struct type use a
    /// write-back calling convention (fields returned in x0..x{N-1}).
    /// At the call site, this registry identifies such functions so the
    /// caller can emit `CallMut`-style write-back into the destination slots.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_return_fns: &'src HashMap<String, String>,

    /// Struct-returning free function registry: fn name → struct type name.
    ///
    /// FLS §9: Free functions that return a named struct type use the same
    /// write-back calling convention as struct-returning associated functions:
    /// fields returned in x0..x{N-1} via RetFields; the call site writes them
    /// to the destination variable's consecutive stack slots via CallMut.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_return_free_fns: &'src HashMap<String, String>,

    /// Struct-returning `&self` instance method registry: mangled name → struct type name.
    ///
    /// FLS §10.1: `&self` instance methods that return a named struct type use
    /// the same write-back calling convention as struct-returning associated
    /// functions. At the call site (in a `let` binding), the caller emits
    /// `CallMut` to write x0..x{N-1} into the destination variable's slots.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_return_methods: &'src HashMap<String, String>,

    /// Enum-returning free function registry: fn name → enum type name.
    ///
    /// FLS §9, §15: Free functions that return an enum type use the same
    /// write-back calling convention: discriminant + fields returned in
    /// x0..x{1+max_fields-1} via RetFields; the call site writes them to
    /// the destination enum variable's stack slots via CallMut.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    enum_return_fns: &'src HashMap<String, String>,

    /// Tuple-returning free function registry: fn name → number of elements.
    ///
    /// FLS §6.10, §9: Free functions that return a tuple type use the
    /// write-back calling convention: elements returned in x0..x{N-1} via
    /// RetFields; the call site writes them to consecutive stack slots via
    /// CallMut and binds each to the tuple pattern variables.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    tuple_return_free_fns: &'src HashMap<String, usize>,

    /// f64-returning free function registry.
    ///
    /// FLS §4.2: Functions that return f64 place the result in d0.
    /// The call site must capture from d0 (not x0) into a float register.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    f64_return_fns: &'src std::collections::HashSet<String>,

    /// f32-returning free function registry.
    ///
    /// FLS §4.2: Functions that return f32 place the result in s0.
    /// The call site must capture from s0 (not x0) into a float register.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    f32_return_fns: &'src std::collections::HashSet<String>,

    /// Maps a struct variable's base stack slot to its struct type name.
    ///
    /// FLS §6.13: Field access expressions need the struct type to compute
    /// the slot offset. When `let p = Point { x, y }` is lowered, `p`'s
    /// base slot is registered here with type `"Point"`.
    local_struct_types: HashMap<u8, String>,

    /// Maps an enum variable's base stack slot to its enum type name.
    ///
    /// FLS §15: Enum values with tuple variants occupy multiple consecutive
    /// slots: slot 0 = discriminant, slots 1..N = positional fields.
    /// When `let x = Opt::Some(v)` is lowered, `x`'s base slot is
    /// registered here with type `"Opt"`. Used by TupleStruct pattern
    /// lowering to locate field slots from the base slot.
    local_enum_types: HashMap<u8, String>,

    /// Maps an array variable's base stack slot to its element count.
    ///
    /// FLS §6.8: Array variables occupy N consecutive 8-byte stack slots,
    /// where N is the element count. When `let a = [e0, e1, e2]` is lowered,
    /// `a`'s base slot is registered here with count 3. Index expressions
    /// look up this map to emit `LoadIndexed` with the correct base slot.
    ///
    /// Cache-line note: populated once per array let binding; read once per
    /// index expression. Not on a hot path.
    local_array_lens: HashMap<u8, usize>,

    /// Maps a 2D array variable's base stack slot to its inner (row) element count.
    ///
    /// FLS §6.8: A 2D array `[[T; M]; N]` occupies N×M consecutive 8-byte stack
    /// slots. `local_array_lens[slot] = N` (outer count); `local_array_inner_lens[slot] = M`
    /// (inner count). Index expression `arr[i][j]` computes linear index `i*M + j`.
    ///
    /// Cache-line note: populated once per 2D array let binding. Not on a hot path.
    local_array_inner_lens: HashMap<u8, usize>,

    /// Set of base slots for arrays whose elements are `f64`.
    ///
    /// FLS §4.5, §4.2: `[f64; N]` arrays store IEEE 754 doubles in each slot.
    /// Index expressions (`arr[i]`) and for-loop element loads emit `LoadIndexedF64`
    /// instead of `LoadIndexed` when the base slot is in this set.
    ///
    /// Cache-line note: populated once per f64 array let binding. Not on a hot path.
    local_f64_array_slots: std::collections::HashSet<u8>,

    /// Set of base slots for arrays whose elements are `f32`.
    ///
    /// FLS §4.5, §4.2: `[f32; N]` arrays store IEEE 754 singles in each slot.
    /// Index expressions and for-loop element loads emit `LoadIndexedF32` when
    /// the base slot is in this set.
    ///
    /// Cache-line note: populated once per f32 array let binding. Not on a hot path.
    local_f32_array_slots: std::collections::HashSet<u8>,

    /// Maps a tuple variable's base stack slot to its field count.
    ///
    /// FLS §6.10: Tuple values occupy N consecutive 8-byte stack slots where
    /// N is the number of elements. When `let t = (a, b)` is lowered, `t`'s
    /// base slot is registered here with count 2. Field access `.0`, `.1`
    /// is lowered to `base_slot + index`.
    ///
    /// Cache-line note: same layout as struct fields or array elements —
    /// N consecutive slots per tuple.
    local_tuple_lens: HashMap<u8, usize>,

    /// Maps a tuple struct variable's base stack slot to its type name.
    ///
    /// FLS §14.2: Tuple struct types. When `let p = Point(a, b)` or `self`
    /// in an `impl Point` method is lowered, the base slot is registered here
    /// with type `"Point"`. This lets method call dispatch compute the mangled
    /// name `Point__method_name` for tuple struct receivers.
    ///
    /// Cache-line note: populated once per let binding or method self-spill;
    /// read once per method call on a tuple struct receiver. Not on a hot path.
    local_tuple_struct_types: HashMap<u8, String>,

    /// Stack slots that hold function pointer values.
    ///
    /// FLS §4.9: Function pointer types. When a parameter or local variable has
    /// type `fn(T) -> R`, its stack slot is recorded here. The Call lowering
    /// path checks this set to decide between direct (`bl {name}`) and indirect
    /// (`blr x9`) call emission.
    ///
    /// Cache-line note: a function pointer occupies one stack slot — same as i32.
    local_fn_ptr_slots: std::collections::HashSet<u8>,

    /// Stack slots that hold a `&str` value.
    ///
    /// FLS §2.4.6: String literals have type `&str`. At this milestone galvanic
    /// materialises only the byte length (a compile-time constant), so the slot
    /// stores an `i32` equal to the UTF-8 byte count of the literal.  The string
    /// pointer (for indexing, slicing, or display) is deferred to a future milestone.
    ///
    /// When `let s = "hello"` is lowered, slot N is stored in this set.  A
    /// subsequent `.len()` call on `s` loads slot N rather than dispatching to
    /// a struct/enum method table.
    ///
    /// Cache-line note: same slot footprint as a scalar `i32` — no extra cost.
    local_str_slots: std::collections::HashSet<u8>,

    /// Captured outer-scope slots for each closure fn-pointer slot.
    ///
    /// FLS §6.22: Capturing closures capture free variables from the enclosing
    /// scope by value. Galvanic compiles each captured variable as a hidden
    /// leading parameter of the closure's hidden function. At every call site
    /// the captured values are loaded from their outer-scope slots and prepended
    /// to the explicit argument list before `CallIndirect` is emitted.
    ///
    /// Maps `fn_ptr_slot → vec![outer_slot_0, outer_slot_1, ...]` in the order
    /// the captures appear in the closure body (first-seen, deduplicated).
    ///
    /// Cache-line note: read/write during closure lowering only; not on the hot
    /// arithmetic path. Map entry is 24 bytes per captured variable.
    local_capture_args: HashMap<u8, Vec<u8>>,

    /// Side-channel from `lower_expr(Closure)` to the surrounding `lower_stmt(Let)`.
    ///
    /// FLS §6.22, §8.1: When a capturing closure is lowered, this field records
    /// the outer-scope slots it captures (in parameter order). The let-binding
    /// handler drains this after storing the closure address to register the
    /// captures for the new fn-pointer slot.
    ///
    /// `None` when the most recently lowered closure had no captures (or when
    /// no closure has been lowered yet).
    ///
    /// Cache-line note: `Option<Vec<u8>>` = 24 bytes (None = 0 heap allocation).
    last_closure_captures: Option<Vec<u8>>,

    /// Side-channel: the name of the most recently lowered capturing closure.
    ///
    /// FLS §6.22, §4.13: Set alongside `last_closure_captures` when a closure
    /// with captures is lowered. The call-arg handler uses this to generate the
    /// trampoline name (`{closure_name}_trampoline`) and tail-call target.
    ///
    /// Cleared when consumed (by either the let-binding handler or the call-arg
    /// trampoline generator). `None` if no capturing closure was recently lowered.
    last_closure_name: Option<String>,

    /// Side-channel: the number of explicit parameters of the last capturing closure.
    ///
    /// FLS §6.22: The trampoline needs to shift `n_explicit` argument registers
    /// up by `n_caps` positions before inserting the captured values at the front.
    ///
    /// Cleared alongside `last_closure_name`.
    last_closure_n_explicit: Option<usize>,

    /// Trampolines generated by this function when capturing closures are passed
    /// as `impl Fn` arguments.
    ///
    /// FLS §6.22, §4.13: Collected here during lowering and propagated up through
    /// `lower_fn`'s return value to the module-level `Module::trampolines` list.
    ///
    /// Cache-line note: typically empty or very small (0–2 entries per function).
    pending_trampolines: Vec<crate::ir::ClosureTrampoline>,

    /// Names of all free functions visible in the current scope.
    ///
    /// FLS §4.9: Used to detect when a single-segment path expression refers to a
    /// function item (and should emit `LoadFnAddr`) rather than a local variable.
    /// Populated from the top-level `lower()` function before lowering begins.
    /// Inner function names (FLS §9, §3: items inside block bodies) are inserted
    /// dynamically as they are encountered during block lowering.
    ///
    /// Owned rather than borrowed so that inner function names can be added
    /// during lowering without requiring a mutable reference to the module-level set.
    ///
    /// Cache-line note: modified once per inner-function declaration; read once
    /// per path expression that might be a function. Not on a hot arithmetic path.
    fn_names: std::collections::HashSet<String>,

    /// Compile-time constant values: maps const name → i32.
    ///
    /// FLS §7.1: Constant items. Every use of a constant is replaced with its
    /// value. When a path expression `FOO` resolves to a known const name,
    /// `LoadImm(value)` is emitted instead of `Load { slot }`.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    const_vals: &'src HashMap<String, i32>,

    /// Compile-time f64 constant values: maps const name → f64.
    ///
    /// FLS §7.1, §4.2: Float const items. When a path expression `PI` resolves
    /// to a known f64 const name, `LoadF64Const` is emitted using the stored
    /// bit pattern.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    const_f64_vals: &'src HashMap<String, f64>,

    /// Compile-time f32 constant values: maps const name → f32.
    ///
    /// FLS §7.1, §4.2: Float const items. When a path expression `HALF` resolves
    /// to a known f32 const name, `LoadF32Const` is emitted using the stored
    /// bit pattern.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    const_f32_vals: &'src HashMap<String, f32>,

    /// Static variable names: the set of names declared as integer `static` items.
    ///
    /// FLS §7.2: Static items. When a path expression `FOO` resolves to a known
    /// static name, `LoadStatic { dst, name }` is emitted instead of `Load { slot }`.
    /// This causes the codegen to emit ADRP + ADD + LDR at the use site.
    ///
    /// FLS §7.2:15: All references to a static go through its memory address —
    /// unlike const substitution, static reads are actual memory loads.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    static_names: &'src std::collections::HashSet<String>,

    /// Names of f64 `static` items. Used to emit `LoadStaticF64` instead of
    /// `LoadStatic` when a path expression resolves to a float static.
    ///
    /// FLS §7.2, §4.2: f64 static items.
    static_f64_names: &'src std::collections::HashSet<String>,

    /// Names of f32 `static` items. Used to emit `LoadStaticF32` instead of
    /// `LoadStatic` when a path expression resolves to an f32 static.
    ///
    /// FLS §7.2, §4.2: f32 static items.
    static_f32_names: &'src std::collections::HashSet<String>,

    /// Per-field struct type names for nested struct support.
    ///
    /// FLS §6.11, §6.13: Struct-type fields require more than one stack slot and
    /// need special handling in both construction (recursive literal storing) and
    /// field access (chained offset computation).
    ///
    /// `None` = scalar field (i32, bool, u32, etc.), `Some(name)` = struct field.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_field_types: &'src HashMap<String, Vec<Option<String>>>,

    /// Per-struct field slot offsets.
    ///
    /// FLS §6.13: Chained field access `s.b.x` requires computing the slot
    /// offset of field `b` within `s`, which equals the sum of sizes of preceding
    /// fields. For scalar fields this is 1; for struct-type fields it is that
    /// struct's total slot count (see `struct_sizes`).
    ///
    /// `struct_field_offsets["Outer"][i]` is the slot offset of field `i`
    /// relative to the base slot of an `Outer` variable.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_field_offsets: &'src HashMap<String, Vec<usize>>,

    /// Total stack slot count for each named struct type.
    ///
    /// FLS §4.11: Galvanic lays out struct fields in declaration order, each
    /// occupying 8 bytes. A struct with N scalar fields has size N. A struct
    /// with a struct-type field of size M has size (sum of field sizes).
    ///
    /// Used to allocate the correct number of consecutive stack slots when a
    /// nested struct variable is bound via `let`.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_sizes: &'src HashMap<String, usize>,

    /// Name of the function currently being lowered.
    ///
    /// Used to generate unique names for closure functions defined inside
    /// this function: `__closure_{fn_name}_{closure_counter}`.
    ///
    /// FLS §6.14: Non-capturing closures compile to hidden named functions.
    fn_name: String,

    /// Counter for closure functions generated inside this function.
    ///
    /// FLS §6.14: Each closure expression produces a unique hidden function.
    /// The name is `__closure_{fn_name}_{closure_counter}`. Incremented for
    /// each closure encountered during lowering.
    ///
    /// Cache-line note: scalar field, negligible overhead.
    closure_counter: u32,

    /// Hidden functions generated by closure expressions in this function.
    ///
    /// FLS §6.14: Non-capturing closures compile to top-level functions.
    /// These are accumulated here and added to the module after `lower_fn`
    /// returns. Nested closures (closures inside closures) are also collected
    /// here by draining the inner `LowerCtx`'s `pending_closures`.
    ///
    /// Cache-line note: Vec header is 24 bytes; elements are heap-allocated.
    pending_closures: Vec<IrFn>,

    /// Resolved type alias map: maps alias name → IrTy.
    ///
    /// FLS §4.10: Type aliases introduce a new name for an existing type.
    /// This map is populated during the first pass of `lower()` and enables
    /// `lower_ty` to resolve aliased type names that are not built-in primitives.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    type_aliases: &'src HashMap<String, IrTy>,

    /// Per-field float type for named structs.
    ///
    /// FLS §4.2, §6.11, §6.13: `None` = not a float field; `Some(IrTy::F64)` = f64;
    /// `Some(IrTy::F32)` = f32. Used to choose the correct store/load instruction
    /// when constructing struct literals with float fields or accessing float fields.
    ///
    /// Cache-line note: read-only during lowering; not on any hot path.
    struct_float_field_types: &'src HashMap<String, Vec<Option<IrTy>>>,

    /// Tracks which stack slots hold f64 or f32 values from struct fields.
    ///
    /// FLS §4.2: When a struct with float fields is stored to the stack, each
    /// float field's slot is registered here with its IrTy. Field-access loads
    /// check this map to emit `LoadF64Slot`/`LoadF32Slot` instead of `Load`.
    ///
    /// Cache-line note: populated at struct literal construction; read at field
    /// access. Not on any critical-path loop.
    slot_float_ty: HashMap<u8, IrTy>,
}

/// Collect all free variables in `expr` that are present in `outer_locals`
/// but are not bound by the closure's own parameter list (`closure_params`).
///
/// FLS §6.22: A closure captures variables from the enclosing scope by value
/// when they appear as free identifiers in the closure body. This function
/// implements capture analysis: it walks the expression AST and for each
/// single-segment path that resolves to an outer local, records the variable
/// name and its stack slot.
///
/// Results are accumulated into `captured` as `(name, outer_slot)` pairs in
/// first-seen order (later duplicate references to the same variable are
/// skipped). The caller uses this ordered list to generate the hidden leading
/// parameters of the closure's compiled function.
///
/// FLS §6.22: "A closure expression captures variables from the surrounding
/// environment." The spec does not mandate a particular capture strategy;
/// galvanic uses capture-by-copy (each captured variable is passed as an
/// extra leading argument on every call, matching the ARM64 integer register
/// ABI). This is equivalent to `move` closure semantics for scalar types.
///
/// Cache-line note: called once per closure during lowering; not on any
/// performance-critical path.
fn find_captures<'src>(
    expr: &Expr,
    outer_locals: &HashMap<&'src str, u8>,
    closure_params: &std::collections::HashSet<&str>,
    source: &'src str,
    captured: &mut Vec<(&'src str, u8)>,
) {
    use crate::ast::{Block, Pat, StmtKind};

    /// Walk a block, tracking inner let-bound names to avoid spurious captures.
    fn find_in_block<'src>(
        block: &Block,
        outer_locals: &HashMap<&'src str, u8>,
        closure_params: &std::collections::HashSet<&str>,
        source: &'src str,
        captured: &mut Vec<(&'src str, u8)>,
    ) {
        // Clone the exclusion set so inner let-bound names don't escape.
        let mut inner_params = closure_params.clone();
        for stmt in &block.stmts {
            match &stmt.kind {
                StmtKind::Let { pat, init, .. } => {
                    // Evaluate the RHS before the binding comes into scope.
                    if let Some(init_expr) = init {
                        find_captures(init_expr, outer_locals, &inner_params, source, captured);
                    }
                    // Bring the bound name into scope (may shadow an outer local).
                    if let Pat::Ident(span) = pat {
                        let n = span.text(source);
                        inner_params.insert(n);
                    }
                }
                StmtKind::Expr(e) => {
                    find_captures(e, outer_locals, &inner_params, source, captured);
                }
                StmtKind::Empty => {}
                // FLS §3, §9: Inner function items do not capture outer locals.
                StmtKind::Item(_) => {}
            }
        }
        if let Some(tail) = &block.tail {
            find_captures(tail, outer_locals, &inner_params, source, captured);
        }
    }

    match &expr.kind {
        // FLS §6.3, §6.22: A single-segment path that names an outer local is
        // a free variable → capture it.
        ExprKind::Path(segs) if segs.len() == 1 => {
            let name = segs[0].text(source);
            if !closure_params.contains(name)
                && let Some(&slot) = outer_locals.get(name)
            {
                // Deduplicate: only record first occurrence.
                if !captured.iter().any(|(n, _)| *n == name) {
                    captured.push((name, slot));
                }
            }
        }

        // FLS §6.4: Block expressions — recurse into stmts and tail.
        ExprKind::Block(block) => {
            find_in_block(block, outer_locals, closure_params, source, captured);
        }

        // Recurse into all compound expression forms.
        ExprKind::Unary { operand, .. } => {
            find_captures(operand, outer_locals, closure_params, source, captured);
        }
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::CompoundAssign { target: lhs, value: rhs, .. } => {
            find_captures(lhs, outer_locals, closure_params, source, captured);
            find_captures(rhs, outer_locals, closure_params, source, captured);
        }
        ExprKind::Cast { expr: inner, .. } => {
            find_captures(inner, outer_locals, closure_params, source, captured);
        }
        ExprKind::Call { callee, args } => {
            find_captures(callee, outer_locals, closure_params, source, captured);
            for a in args {
                find_captures(a, outer_locals, closure_params, source, captured);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            find_captures(receiver, outer_locals, closure_params, source, captured);
            for a in args {
                find_captures(a, outer_locals, closure_params, source, captured);
            }
        }
        ExprKind::FieldAccess { receiver, .. } => {
            find_captures(receiver, outer_locals, closure_params, source, captured);
        }
        ExprKind::Index { base, index } => {
            find_captures(base, outer_locals, closure_params, source, captured);
            find_captures(index, outer_locals, closure_params, source, captured);
        }
        ExprKind::If { cond, then_block, else_expr } => {
            find_captures(cond, outer_locals, closure_params, source, captured);
            find_in_block(then_block, outer_locals, closure_params, source, captured);
            if let Some(e) = else_expr {
                find_captures(e, outer_locals, closure_params, source, captured);
            }
        }
        ExprKind::Return(Some(v)) => {
            find_captures(v, outer_locals, closure_params, source, captured);
        }
        ExprKind::Break { value: Some(v), .. } => {
            find_captures(v, outer_locals, closure_params, source, captured);
        }
        ExprKind::Tuple(elems) | ExprKind::Array(elems) => {
            for e in elems {
                find_captures(e, outer_locals, closure_params, source, captured);
            }
        }
        ExprKind::ArrayRepeat { value, count } => {
            find_captures(value, outer_locals, closure_params, source, captured);
            find_captures(count, outer_locals, closure_params, source, captured);
        }
        ExprKind::While { cond, body, .. } => {
            find_captures(cond, outer_locals, closure_params, source, captured);
            find_in_block(body, outer_locals, closure_params, source, captured);
        }
        ExprKind::Loop { body, .. } => {
            find_in_block(body, outer_locals, closure_params, source, captured);
        }
        ExprKind::Match { scrutinee, arms } => {
            find_captures(scrutinee, outer_locals, closure_params, source, captured);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    find_captures(guard, outer_locals, closure_params, source, captured);
                }
                find_captures(&arm.body, outer_locals, closure_params, source, captured);
            }
        }
        // Literals, unit, path (multi-segment), LitFloat, LitStr, LitChar,
        // LitBool, LitInt, Continue, Return(None), Break(None): no sub-expressions.
        _ => {}
    }
}

impl<'src> LowerCtx<'src> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        source: &'src str,
        fn_name: &str,
        fn_ret_ty: IrTy,
        struct_defs: &'src HashMap<String, Vec<String>>,
        tuple_struct_defs: &'src HashMap<String, usize>,
        tuple_struct_float_field_types: &'src HashMap<String, Vec<Option<IrTy>>>,
        enum_defs: &'src EnumDefs,
        enum_variant_float_field_types: &'src HashMap<String, HashMap<String, Vec<Option<IrTy>>>>,
        method_self_kinds: &'src HashMap<String, SelfKind>,
        mut_self_scalar_return_fns: &'src std::collections::HashSet<String>,
        struct_return_fns: &'src HashMap<String, String>,
        struct_return_free_fns: &'src HashMap<String, String>,
        enum_return_fns: &'src HashMap<String, String>,
        struct_return_methods: &'src HashMap<String, String>,
        tuple_return_free_fns: &'src HashMap<String, usize>,
        f64_return_fns: &'src std::collections::HashSet<String>,
        f32_return_fns: &'src std::collections::HashSet<String>,
        const_vals: &'src HashMap<String, i32>,
        const_f64_vals: &'src HashMap<String, f64>,
        const_f32_vals: &'src HashMap<String, f32>,
        static_names: &'src std::collections::HashSet<String>,
        static_f64_names: &'src std::collections::HashSet<String>,
        static_f32_names: &'src std::collections::HashSet<String>,
        fn_names: &std::collections::HashSet<String>,
        struct_field_types: &'src HashMap<String, Vec<Option<String>>>,
        struct_field_offsets: &'src HashMap<String, Vec<usize>>,
        struct_sizes: &'src HashMap<String, usize>,
        type_aliases: &'src HashMap<String, IrTy>,
        struct_float_field_types: &'src HashMap<String, Vec<Option<IrTy>>>,
        start_label: u32,
    ) -> Self {
        LowerCtx {
            source,
            fn_name: fn_name.to_owned(),
            closure_counter: 0,
            pending_closures: Vec::new(),
            instrs: Vec::new(),
            next_reg: 0,
            next_slot: 0,
            next_label: start_label,
            locals: HashMap::new(),
            has_calls: false,
            loop_stack: Vec::new(),
            fn_ret_ty,
            struct_defs,
            tuple_struct_defs,
            tuple_struct_float_field_types,
            enum_defs,
            enum_variant_float_field_types,
            method_self_kinds,
            mut_self_scalar_return_fns,
            struct_return_fns,
            struct_return_free_fns,
            enum_return_fns,
            struct_return_methods,
            tuple_return_free_fns,
            f64_return_fns,
            f32_return_fns,
            const_vals,
            const_f64_vals,
            const_f32_vals,
            static_names,
            static_f64_names,
            static_f32_names,
            fn_names: fn_names.clone(),
            struct_field_types,
            struct_field_offsets,
            struct_sizes,
            local_struct_types: HashMap::new(),
            local_enum_types: HashMap::new(),
            local_array_lens: HashMap::new(),
            local_array_inner_lens: HashMap::new(),
            local_f64_array_slots: std::collections::HashSet::new(),
            local_f32_array_slots: std::collections::HashSet::new(),
            local_tuple_lens: HashMap::new(),
            local_tuple_struct_types: HashMap::new(),
            local_fn_ptr_slots: std::collections::HashSet::new(),
            local_str_slots: std::collections::HashSet::new(),
            local_capture_args: HashMap::new(),
            last_closure_captures: None,
            last_closure_name: None,
            last_closure_n_explicit: None,
            pending_trampolines: Vec::new(),
            float_locals: HashMap::new(),
            float_consts: Vec::new(),
            float32_locals: HashMap::new(),
            float32_consts: Vec::new(),
            type_aliases,
            struct_float_field_types,
            slot_float_ty: HashMap::new(),
        }
    }

    /// Allocate the next virtual register.
    fn alloc_reg(&mut self) -> Result<u8, LowerError> {
        let r = self.next_reg;
        self.next_reg = self.next_reg.checked_add(1).ok_or_else(|| {
            LowerError::Unsupported("exceeded 256 virtual registers".into())
        })?;
        Ok(r)
    }

    /// Allocate the next stack slot for a local variable.
    ///
    /// FLS §8.1: Each let binding gets one 8-byte slot.
    fn alloc_slot(&mut self) -> Result<u8, LowerError> {
        let s = self.next_slot;
        self.next_slot = self.next_slot.checked_add(1).ok_or_else(|| {
            LowerError::Unsupported("exceeded 256 stack slots".into())
        })?;
        Ok(s)
    }

    /// Return the float IrTy (F64 or F32) for a named struct field, if any.
    ///
    /// FLS §4.2, §6.11, §6.13: Used to choose the correct store/load instruction
    /// for struct fields declared as `f64` or `f32`. Returns `None` for integer-like
    /// fields (i32, bool, etc.) and nested-struct fields.
    fn field_float_ty(&self, struct_name: &str, field_idx: usize) -> Option<IrTy> {
        self.struct_float_field_types
            .get(struct_name)
            .and_then(|fts| fts.get(field_idx))
            .copied()
            .flatten()
    }

    /// Return the float IrTy (F64 or F32) for a positional field of an enum tuple variant.
    ///
    /// FLS §4.2, §15: Used during variant construction and TupleStruct pattern binding
    /// to choose StoreF64/LoadF64Slot vs StoreF32/LoadF32Slot vs Store/Load.
    /// Returns `None` for integer/bool fields.
    fn enum_variant_field_float_ty(
        &self,
        enum_name: &str,
        variant_name: &str,
        field_idx: usize,
    ) -> Option<IrTy> {
        self.enum_variant_float_field_types
            .get(enum_name)
            .and_then(|vm| vm.get(variant_name))
            .and_then(|fts| fts.get(field_idx))
            .copied()
            .flatten()
    }

    /// Return true if `expr` is statically known to produce an `f64` value.
    ///
    /// FLS §4.2: Used at cast sites to select `F64ToI32` (FCVTZS) when the
    /// source is a float, vs. integer identity/narrowing otherwise.
    ///
    /// Conservative check: only recognises float literals and path expressions
    /// to variables registered in `float_locals`. All other expressions are
    /// assumed integer-typed at this milestone.
    fn is_f64_expr(&self, expr: &crate::ast::Expr) -> bool {
        match &expr.kind {
            crate::ast::ExprKind::LitFloat => {
                // FLS §2.4.4.2: A literal with `_f32` suffix is f32, not f64.
                let text = expr.span.text(self.source);
                !text.ends_with("_f32")
            }
            crate::ast::ExprKind::Path(segs) if segs.len() == 1 => {
                let name = segs[0].text(self.source);
                self.float_locals.contains_key(name)
                    || self.const_f64_vals.contains_key(name)
                    || self.static_f64_names.contains(name)
            }
            // FLS §6.5.5: A binary arithmetic expression on f64 operands produces f64.
            // Bitwise ops and shifts are not defined for f64 (FLS §6.5.6–§6.5.8).
            crate::ast::ExprKind::Binary { op: BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div, lhs, rhs } => {
                self.is_f64_expr(lhs) && self.is_f64_expr(rhs)
            }
            // FLS §6.5.9: A cast expression `x as f64` produces an f64 value.
            crate::ast::ExprKind::Cast { ty, .. } => {
                if let crate::ast::TyKind::Path(segs) = &ty.kind {
                    segs.len() == 1 && segs[0].text(self.source) == "f64"
                } else {
                    false
                }
            }
            // FLS §6.5.4: `-expr` has the same type as `expr`.
            crate::ast::ExprKind::Unary { op: crate::ast::UnaryOp::Neg, operand } => {
                self.is_f64_expr(operand)
            }
            // FLS §6.9 + §4.5: `arr[i]` produces f64 if `arr` is a `[f64; N]` local.
            // Required so that `arr[i] as i32` selects `F64ToI32` (fcvtzs).
            crate::ast::ExprKind::Index { base, .. } => {
                if let crate::ast::ExprKind::Path(segs) = &base.kind
                    && segs.len() == 1
                    && let Some(&slot) = self.locals.get(segs[0].text(self.source))
                {
                    return self.local_f64_array_slots.contains(&slot);
                }
                false
            }
            // FLS §6.12.1: A call to a function in `f64_return_fns` produces f64.
            // Required so that `f64_fn(...) as i32` selects `F64ToI32` (fcvtzs)
            // rather than falling through to the integer cast path.
            crate::ast::ExprKind::Call { callee, .. } => {
                if let crate::ast::ExprKind::Path(segs) = &callee.kind {
                    if segs.len() == 1 {
                        return self.f64_return_fns.contains(segs[0].text(self.source));
                    }
                    if segs.len() == 2 {
                        let mangled = format!("{}__{}", segs[0].text(self.source), segs[1].text(self.source));
                        return self.f64_return_fns.contains(&mangled);
                    }
                }
                false
            }
            // FLS §6.13, §4.2: Field access on a struct with an f64 field produces f64.
            // Enables `p.x as i32` and `(p.x + p.y) as i32` to select F64ToI32.
            // FLS §6.10: Also handles tuple field access `t.0` where t has an f64
            // element — checks slot_float_ty for the resolved element slot.
            crate::ast::ExprKind::FieldAccess { receiver, field } => {
                if let crate::ast::ExprKind::Path(segs) = &receiver.kind
                    && segs.len() == 1
                {
                    let var_name = segs[0].text(self.source);
                    if let Some(&slot) = self.locals.get(var_name) {
                        // FLS §6.10: Tuple field — check slot_float_ty on the element slot.
                        if self.local_tuple_lens.contains_key(&slot)
                            && let Ok(idx) = field.text(self.source).parse::<usize>()
                        {
                            let elem_slot = slot + idx as u8;
                            return matches!(self.slot_float_ty.get(&elem_slot), Some(IrTy::F64));
                        }
                        if let Some(struct_name) = self.local_struct_types.get(&slot).cloned() {
                            let field_name = field.text(self.source);
                            if let Some(field_names) = self.struct_defs.get(&struct_name)
                                && let Some(fi) = field_names.iter().position(|n| n == field_name) {
                                    return matches!(self.field_float_ty(&struct_name, fi), Some(IrTy::F64));
                                }
                        }
                    }
                }
                false
            }
            // FLS §6.12.2, §4.2: A method call `var.method(...)` returns f64 if the
            // method is registered in `f64_return_fns` (mangled as `Type__method`).
            // Required so that `p.sum() as i32` selects `F64ToI32` (fcvtzs).
            // FLS §14.2: Tuple struct receivers are registered in `local_tuple_struct_types`,
            // not `local_struct_types` — check both.
            crate::ast::ExprKind::MethodCall { receiver, method, .. } => {
                if let crate::ast::ExprKind::Path(segs) = &receiver.kind
                    && segs.len() == 1
                {
                    let var_name = segs[0].text(self.source);
                    if let Some(&slot) = self.locals.get(var_name) {
                        let struct_name = self.local_struct_types.get(&slot)
                            .or_else(|| self.local_tuple_struct_types.get(&slot))
                            .cloned();
                        if let Some(struct_name) = struct_name {
                            let method_name = method.text(self.source);
                            let mangled = format!("{struct_name}__{method_name}");
                            return self.f64_return_fns.contains(&mangled);
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Return true if `expr` is statically known to produce an `f32` value.
    ///
    /// FLS §4.2: Used at cast sites to select `F32ToI32` when the source is
    /// an f32, vs. f64 or integer otherwise.
    ///
    /// FLS §2.4.4.2: Float literals with `_f32` suffix are f32-typed.
    fn is_f32_expr(&self, expr: &crate::ast::Expr) -> bool {
        match &expr.kind {
            crate::ast::ExprKind::LitFloat => {
                let text = expr.span.text(self.source);
                text.ends_with("_f32")
            }
            crate::ast::ExprKind::Path(segs) if segs.len() == 1 => {
                let name = segs[0].text(self.source);
                self.float32_locals.contains_key(name)
                    || self.const_f32_vals.contains_key(name)
                    || self.static_f32_names.contains(name)
            }
            // FLS §6.5.5: A binary arithmetic expression on f32 operands produces f32.
            crate::ast::ExprKind::Binary { op: BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div, lhs, rhs } => {
                self.is_f32_expr(lhs) && self.is_f32_expr(rhs)
            }
            // FLS §6.5.9: A cast expression `x as f32` produces an f32 value.
            crate::ast::ExprKind::Cast { ty, .. } => {
                if let crate::ast::TyKind::Path(segs) = &ty.kind {
                    segs.len() == 1 && segs[0].text(self.source) == "f32"
                } else {
                    false
                }
            }
            // FLS §6.5.4: `-expr` has the same type as `expr`.
            crate::ast::ExprKind::Unary { op: crate::ast::UnaryOp::Neg, operand } => {
                self.is_f32_expr(operand)
            }
            // FLS §6.9 + §4.5: `arr[i]` produces f32 if `arr` is a `[f32; N]` local.
            // Required so that `arr[i] as i32` selects `F32ToI32` (fcvtzs).
            crate::ast::ExprKind::Index { base, .. } => {
                if let crate::ast::ExprKind::Path(segs) = &base.kind
                    && segs.len() == 1
                    && let Some(&slot) = self.locals.get(segs[0].text(self.source))
                {
                    return self.local_f32_array_slots.contains(&slot);
                }
                false
            }
            // FLS §6.12.1: A call to a function in `f32_return_fns` produces f32.
            // Required so that `f32_fn(...) as i32` selects `F32ToI32` (fcvtzs)
            // rather than falling through to the integer cast path.
            crate::ast::ExprKind::Call { callee, .. } => {
                if let crate::ast::ExprKind::Path(segs) = &callee.kind {
                    if segs.len() == 1 {
                        return self.f32_return_fns.contains(segs[0].text(self.source));
                    }
                    if segs.len() == 2 {
                        let mangled = format!("{}__{}", segs[0].text(self.source), segs[1].text(self.source));
                        return self.f32_return_fns.contains(&mangled);
                    }
                }
                false
            }
            // FLS §6.13, §4.2: Field access on a struct with an f32 field produces f32.
            // Enables `p.x as i32` and `(p.x + p.y) as i32` to select F32ToI32.
            // FLS §6.10: Also handles tuple field access `t.0` where t has an f32
            // element — checks slot_float_ty for the resolved element slot.
            crate::ast::ExprKind::FieldAccess { receiver, field } => {
                if let crate::ast::ExprKind::Path(segs) = &receiver.kind
                    && segs.len() == 1
                {
                    let var_name = segs[0].text(self.source);
                    if let Some(&slot) = self.locals.get(var_name) {
                        // FLS §6.10: Tuple field — check slot_float_ty on the element slot.
                        if self.local_tuple_lens.contains_key(&slot)
                            && let Ok(idx) = field.text(self.source).parse::<usize>()
                        {
                            let elem_slot = slot + idx as u8;
                            return matches!(self.slot_float_ty.get(&elem_slot), Some(IrTy::F32));
                        }
                        if let Some(struct_name) = self.local_struct_types.get(&slot).cloned() {
                            let field_name = field.text(self.source);
                            if let Some(field_names) = self.struct_defs.get(&struct_name)
                                && let Some(fi) = field_names.iter().position(|n| n == field_name) {
                                    return matches!(self.field_float_ty(&struct_name, fi), Some(IrTy::F32));
                                }
                        }
                    }
                }
                false
            }
            // FLS §6.12.2, §4.2: A method call `var.method(...)` returns f32 if the
            // method is registered in `f32_return_fns` (mangled as `Type__method`).
            // Required so that `p.sum() as i32` selects `F32ToI32` (fcvtzs).
            // FLS §14.2: Tuple struct receivers are registered in `local_tuple_struct_types`,
            // not `local_struct_types` — check both.
            crate::ast::ExprKind::MethodCall { receiver, method, .. } => {
                if let crate::ast::ExprKind::Path(segs) = &receiver.kind
                    && segs.len() == 1
                {
                    let var_name = segs[0].text(self.source);
                    if let Some(&slot) = self.locals.get(var_name) {
                        let struct_name = self.local_struct_types.get(&slot)
                            .or_else(|| self.local_tuple_struct_types.get(&slot))
                            .cloned();
                        if let Some(struct_name) = struct_name {
                            let method_name = method.text(self.source);
                            let mangled = format!("{struct_name}__{method_name}");
                            return self.f32_return_fns.contains(&mangled);
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// FLS §5.10.3 + §8.1: Recursively bind a tuple pattern to a tuple literal init.
    ///
    /// Handles `let (a, (b, c)) = (1, (2, 3));` by recursing into nested
    /// `Pat::Tuple` sub-patterns. Each leaf `Pat::Ident` gets its own stack
    /// slot; `Pat::Wildcard` discards (no slot allocated); nested `Pat::Tuple`
    /// requires the corresponding expression to also be a `ExprKind::Tuple`.
    ///
    /// Elements are evaluated left-to-right (FLS §6.4:14). All stores are
    /// runtime instructions (FLS §6.1.2:37–45).
    ///
    /// Returns the base slot of the first allocated slot in this call (used
    /// by the caller to optionally register the tuple in `local_tuple_lens`).
    fn lower_tuple_pat_from_literal(
        &mut self,
        pats: &[Pat],
        exprs: &[Expr],
    ) -> Result<u8, LowerError> {
        if pats.len() != exprs.len() {
            return Err(LowerError::Unsupported(format!(
                "tuple pattern has {} elements but initializer has {}",
                pats.len(),
                exprs.len()
            )));
        }
        let base_slot = self.next_slot;
        for (sub_pat, elem_expr) in pats.iter().zip(exprs.iter()) {
            match sub_pat {
                Pat::Ident(span) => {
                    let slot = self.alloc_slot()?;
                    // FLS §6.4:14: Evaluate left-to-right before storing.
                    let val = self.lower_expr(elem_expr, &IrTy::I32)?;
                    let src = self.val_to_reg(val)?;
                    self.instrs.push(Instr::Store { src, slot });
                    self.locals.insert(span.text(self.source), slot);
                }
                Pat::Wildcard => {
                    // FLS §6.4:14: Evaluate for side effects; discard result.
                    // No stack slot allocated — wildcard binds nothing.
                    self.lower_expr(elem_expr, &IrTy::I32)?;
                }
                Pat::Tuple(inner_pats) => {
                    // FLS §5.10.3: Nested tuple — the corresponding element
                    // must itself be a tuple literal at this milestone.
                    let ExprKind::Tuple(inner_exprs) = &elem_expr.kind else {
                        return Err(LowerError::Unsupported(
                            "nested tuple pattern requires a tuple literal initializer; \
                             variable-init nested tuples are not yet supported"
                                .into(),
                        ));
                    };
                    self.lower_tuple_pat_from_literal(inner_pats, inner_exprs)?;
                }
                _ => {
                    return Err(LowerError::Unsupported(
                        "only ident, wildcard, and tuple sub-patterns are supported \
                         in tuple let-patterns at this milestone"
                            .into(),
                    ));
                }
            }
        }
        Ok(base_slot)
    }

    /// Allocate the next unique label ID.
    ///
    /// FLS §6.17: Each if expression needs two labels (else and end).
    /// Labels are function-scoped and monotonically increasing.
    fn alloc_label(&mut self) -> u32 {
        let id = self.next_label;
        self.next_label += 1;
        id
    }

    /// Resolve a literal or path pattern to its integer value.
    ///
    /// Used inside OR pattern inner loops where the alternative is a simple
    /// scalar value. `Pat::LitInt`, `Pat::NegLitInt`, `Pat::LitBool`, and
    /// `Pat::Path` (enum unit variant) are all valid. Other pattern kinds
    /// (ranges, wildcards, OR, Ident) are not supported here.
    ///
    /// FLS §5.2: Literal patterns. FLS §5.5 + §15: Path patterns.
    fn pat_scalar_imm(&self, pat: &Pat) -> Result<i32, LowerError> {
        match pat {
            Pat::LitInt(n) => Ok(*n as i32),
            Pat::NegLitInt(n) => Ok(-(*n as i32)),
            Pat::LitBool(b) => Ok(*b as i32),
            Pat::Path(segs) if segs.len() == 2 => {
                let enum_name = segs[0].text(self.source);
                let variant_name = segs[1].text(self.source);
                self.enum_defs
                    .get(enum_name)
                    .and_then(|v| v.get(variant_name))
                    .map(|(disc, _)| *disc)
                    .ok_or_else(|| LowerError::Unsupported(format!(
                        "unknown enum variant `{enum_name}::{variant_name}` in pattern"
                    )))
            }
            _ => Err(LowerError::Unsupported(
                "unsupported pattern kind inside OR pattern".into(),
            )),
        }
    }

    /// Ensure `val` is in a virtual register. If it's already a register,
    /// return it. If it's a constant, emit a `LoadImm`.
    fn val_to_reg(&mut self, val: IrValue) -> Result<u8, LowerError> {
        match val {
            IrValue::Reg(r) => Ok(r),
            IrValue::I32(n) => {
                let r = self.alloc_reg()?;
                self.instrs.push(Instr::LoadImm(r, n));
                Ok(r)
            }
            IrValue::Unit => Err(LowerError::Unsupported(
                "unit value used as arithmetic operand".into(),
            )),
            // FLS §4.2: Float register values cannot be used in integer
            // arithmetic contexts directly; F64ToI32 is required first.
            IrValue::FReg(_) => Err(LowerError::Unsupported(
                "float register used in integer context; cast to i32 first (FLS §6.5.9)".into(),
            )),
            // FLS §4.2: f32 register values also require an explicit cast.
            IrValue::F32Reg(_) => Err(LowerError::Unsupported(
                "f32 register used in integer context; cast to i32 first (FLS §6.5.9)".into(),
            )),
        }
    }

    // ── Nested struct helpers ─────────────────────────────────────────────────

    /// Store a nested struct literal into consecutive stack slots starting at `base_slot`.
    ///
    /// Called when a struct literal has a field whose type is itself a named struct.
    /// For example, in `let r = Rect { a: Point { x: 1, y: 2 }, b: ... }`, when
    /// storing the `a` field we recurse with `struct_name = "Point"`.
    ///
    /// FLS §6.11: Struct expressions. Each field initializer is evaluated and
    /// stored into the corresponding slot of the nested struct's layout.
    ///
    /// FLS §6.1.2:37–45: All stores are runtime instructions — no const folding.
    ///
    /// Cache-line note: N scalar fields in the nested struct emit N `str` instructions
    /// (4 bytes each); the slots are consecutive so consecutive stores touch the same
    /// 64-byte cache lines as non-nested structs.
    fn store_nested_struct_lit(
        &mut self,
        expr: &Expr,
        base_slot: u8,
        struct_name: &str,
    ) -> Result<(), LowerError> {
        // The initializer must be a struct literal for the same type.
        let ExprKind::StructLit { fields: lit_fields, .. } = &expr.kind else {
            return Err(LowerError::Unsupported(format!(
                "expected struct literal `{struct_name} {{ .. }}` for nested struct field"
            )));
        };

        let field_names = self
            .struct_defs
            .get(struct_name)
            .ok_or_else(|| {
                LowerError::Unsupported(format!("unknown struct type `{struct_name}`"))
            })?
            .clone();

        let field_offsets = self
            .struct_field_offsets
            .get(struct_name)
            .cloned()
            .unwrap_or_default();
        let field_types = self
            .struct_field_types
            .get(struct_name)
            .cloned()
            .unwrap_or_default();

        for (field_idx, field_name) in field_names.iter().enumerate() {
            let field_offset = field_offsets.get(field_idx).copied().unwrap_or(field_idx);
            let dst_slot = base_slot + field_offset as u8;

            let field_init = lit_fields
                .iter()
                .find(|(f, _)| f.text(self.source) == field_name.as_str())
                .ok_or_else(|| {
                    LowerError::Unsupported(format!(
                        "missing field `{field_name}` in nested `{struct_name}` literal"
                    ))
                })?;

            let nested_ty = field_types.get(field_idx).cloned().flatten();
            if let Some(nested_type_name) = nested_ty {
                // Doubly-nested struct: recurse.
                self.store_nested_struct_lit(&field_init.1, dst_slot, &nested_type_name)?;
            } else {
                let val = self.lower_expr(&field_init.1, &IrTy::I32)?;
                let src = self.val_to_reg(val)?;
                self.instrs.push(Instr::Store { src, slot: dst_slot });
            }
        }
        Ok(())
    }

    /// Expand a (possibly nested) tuple literal into leaf argument registers.
    ///
    /// For `(1, (2, 3))` passed to `fn f((a, (b, c)): (i32, (i32, i32)))`,
    /// the calling convention passes each scalar leaf as a separate register:
    /// x0 = 1, x1 = 2, x2 = 3. This helper recurses into nested tuple elements
    /// so that the correct number and order of registers are pushed to `arg_regs`.
    ///
    /// FLS §6.10, §5.10.3: Count total scalar leaves in a tuple expression.
    ///
    /// For flat `(a, b, c)` → 3. For nested `(a, (b, c))` → 3.
    /// Used to determine how many stack slots to allocate when storing
    /// a tuple literal in a `let` binding.
    fn count_tuple_leaves(elements: &[Expr]) -> usize {
        let mut count = 0;
        for elem in elements {
            if let ExprKind::Tuple(inner) = &elem.kind {
                count += Self::count_tuple_leaves(inner);
            } else {
                count += 1;
            }
        }
        count
    }

    /// FLS §6.10, §5.10.3: Store all scalar leaves of a nested tuple literal
    /// into consecutive stack slots starting at `base_slot + *offset`.
    ///
    /// Recurses into nested `ExprKind::Tuple` elements. Each scalar leaf is
    /// evaluated at runtime and stored in the next available slot.
    ///
    /// FLS §6.4:14: Elements evaluated and stored left-to-right.
    /// FLS §6.1.2:37–45: All stores are runtime instructions.
    ///
    /// Cache-line note: N leaves → N × 4-byte `str` instructions.
    fn store_tuple_leaves(
        &mut self,
        elements: &[Expr],
        base_slot: u8,
        offset: &mut u8,
    ) -> Result<(), LowerError> {
        for elem in elements {
            if let ExprKind::Tuple(inner) = &elem.kind {
                self.store_tuple_leaves(inner, base_slot, offset)?;
            } else {
                let slot = base_slot + *offset;
                // FLS §4.2: lower with I32 hint; float literals and float
                // variable paths return FReg/F32Reg regardless of hint, so
                // detect the register kind and use the appropriate store.
                // FLS §6.1.2:37–45: All stores are runtime instructions.
                let val = self.lower_expr(elem, &IrTy::I32)?;
                match val {
                    IrValue::FReg(src) => {
                        // FLS §4.2: f64 element — record slot type and emit
                        // StoreF64 so subsequent LoadF64Slot emits correctly.
                        self.slot_float_ty.insert(slot, IrTy::F64);
                        self.instrs.push(Instr::StoreF64 { src, slot });
                    }
                    IrValue::F32Reg(src) => {
                        // FLS §4.2: f32 element — same pattern for s-registers.
                        self.slot_float_ty.insert(slot, IrTy::F32);
                        self.instrs.push(Instr::StoreF32 { src, slot });
                    }
                    other => {
                        let src = self.val_to_reg(other)?;
                        self.instrs.push(Instr::Store { src, slot });
                    }
                }
                *offset += 1;
            }
        }
        Ok(())
    }

    /// FLS §5.10.3, §6.10, §9.2: Nested tuple patterns in parameter position
    /// flatten to consecutive registers. The tuple literal argument must match
    /// the structure of the parameter pattern.
    /// FLS §6.1.2:37–45: All evaluations emit runtime instructions.
    ///
    /// Cache-line note: N scalar leaves → N × 4-byte instructions.
    fn push_tuple_lit_arg_regs(
        &mut self,
        elements: &[Expr],
        arg_regs: &mut Vec<u8>,
    ) -> Result<(), LowerError> {
        for elem in elements {
            if let ExprKind::Tuple(inner) = &elem.kind {
                // Nested tuple: recurse into its elements.
                self.push_tuple_lit_arg_regs(inner, arg_regs)?;
            } else {
                let val = self.lower_expr(elem, &IrTy::I32)?;
                let reg = self.val_to_reg(val)?;
                arg_regs.push(reg);
            }
        }
        Ok(())
    }

    /// Expand a struct literal expression into leaf-field argument registers.
    ///
    /// For `Outer { x: 3, inner: Inner { a: 4 } }` passed to `fn f(o: Outer)`,
    /// the calling convention passes each leaf slot as a separate register:
    /// x0 = 3 (outer.x), x1 = 4 (inner.a). This helper recurses into nested
    /// struct-type fields so that the correct number and order of registers are
    /// pushed to `arg_regs`.
    ///
    /// FLS §6.11 + §11: Struct literal arguments are expanded field-by-field in
    /// declaration order. Nested struct fields are recursively expanded.
    /// FLS §6.1.2:37–45: All register loads are runtime instructions.
    ///
    /// Cache-line note: N leaf registers = N × 4-byte `mov`/load instructions,
    /// matching the N-slot parameter spill in `lower_fn`.
    fn push_struct_lit_arg_regs(
        &mut self,
        expr: &Expr,
        struct_name: &str,
        arg_regs: &mut Vec<u8>,
    ) -> Result<(), LowerError> {
        let ExprKind::StructLit { fields: lit_fields, .. } = &expr.kind else {
            return Err(LowerError::Unsupported(format!(
                "expected struct literal of type `{struct_name}` as function argument"
            )));
        };

        let field_names = self
            .struct_defs
            .get(struct_name)
            .ok_or_else(|| {
                LowerError::Unsupported(format!(
                    "unknown struct type `{struct_name}` in function argument"
                ))
            })?
            .clone();

        let field_types = self
            .struct_field_types
            .get(struct_name)
            .cloned()
            .unwrap_or_default();

        for (fi, field_name) in field_names.iter().enumerate() {
            let (_, field_val_expr) = lit_fields
                .iter()
                .find(|(f, _)| f.text(self.source) == field_name.as_str())
                .ok_or_else(|| {
                    LowerError::Unsupported(format!(
                        "missing field `{field_name}` in `{struct_name}` literal argument"
                    ))
                })?;

            let nested_ty = field_types.get(fi).cloned().flatten();
            if let Some(inner_type) = nested_ty {
                // This field is a nested struct — recurse to expand its leaf fields.
                // FLS §6.11: Field initializers of struct type must themselves be
                // struct literals (or struct variables handled by the var-path path).
                self.push_struct_lit_arg_regs(field_val_expr, &inner_type, arg_regs)?;
            } else {
                // Scalar field — lower to a register and push.
                let val = self.lower_expr(field_val_expr, &IrTy::I32)?;
                let reg = self.val_to_reg(val)?;
                arg_regs.push(reg);
            }
        }
        Ok(())
    }

    /// Bind struct field patterns recursively from a known base slot.
    ///
    /// After a struct value is on the stack (slots `base_slot .. base_slot + size`),
    /// this method walks `pat_fields` and installs each `Pat::Ident` binding into
    /// `self.locals`. For nested struct sub-patterns (`inner: Inner { a, b }`),
    /// it recurses with the inner struct's base slot.
    ///
    /// FLS §5.10.2: Struct patterns. FLS §8.1: Let statements.
    ///
    /// Cache-line note: only `self.locals` is mutated — zero instructions emitted.
    fn bind_struct_fields_from_slot(
        &mut self,
        struct_name: &str,
        base_slot: u8,
        pat_fields: &[(crate::ast::Span, Pat)],
    ) -> Result<(), LowerError> {
        let field_names = self
            .struct_defs
            .get(struct_name)
            .cloned()
            .ok_or_else(|| {
                LowerError::Unsupported(format!("unknown struct type `{struct_name}`"))
            })?;
        let offsets = self
            .struct_field_offsets
            .get(struct_name)
            .cloned()
            .ok_or_else(|| {
                LowerError::Unsupported(format!(
                    "unknown field offsets for struct `{struct_name}`"
                ))
            })?;
        for (field_name_span, sub_pat) in pat_fields {
            let fname = field_name_span.text(self.source);
            let fi = field_names
                .iter()
                .position(|f| f == fname)
                .ok_or_else(|| {
                    LowerError::Unsupported(format!(
                        "no field `{fname}` on struct `{struct_name}`"
                    ))
                })?;
            let slot = base_slot + offsets[fi] as u8;
            match sub_pat {
                Pat::Ident(bind_span) => {
                    let bind_name = bind_span.text(self.source);
                    self.locals.insert(bind_name, slot);
                }
                Pat::Wildcard => {}
                Pat::StructVariant { path, fields: inner_fields } if path.len() == 1 => {
                    // FLS §5.10.2: Struct patterns may nest arbitrarily deep.
                    // The field's slot is the base of the inner struct's layout.
                    let inner_name = path[0].text(self.source).to_owned();
                    self.bind_struct_fields_from_slot(&inner_name, slot, inner_fields)?;
                }
                _ => {
                    return Err(LowerError::Unsupported(
                        "only ident, wildcard, and nested struct sub-patterns are \
                         supported in struct let-patterns"
                            .into(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Resolve a place expression (variable or chained field access) to its
    /// stack slot index and optional struct type name.
    ///
    /// Returns `(slot, Some(type_name))` for a struct-typed place (e.g., `r.b`
    /// where `b` is of type `Point`) or `(slot, None)` for a scalar place.
    ///
    /// This is the core of chained field access (`r.b.x`): the outer call
    /// resolves `r.b` to `(slot_of_b, Some("Point"))`, then resolves `.x`
    /// within `Point` to yield the final scalar slot.
    ///
    /// FLS §6.13: Field access expressions. The spec allows any number of
    /// nested field accesses; we support arbitrary depth by recursion.
    ///
    /// FLS §6.1.4: Place expressions. A field access on a place expression is
    /// itself a place expression — it can be the left operand of assignment.
    ///
    /// Cache-line note: each level of recursion costs one map lookup, not a
    /// memory access. The actual load is emitted by the caller.
    fn resolve_place(
        &self,
        expr: &Expr,
    ) -> Result<(u8, Option<String>), LowerError> {
        match &expr.kind {
            ExprKind::Path(segs) if segs.len() == 1 => {
                let var_name = segs[0].text(self.source);
                let slot = *self.locals.get(var_name).ok_or_else(|| {
                    LowerError::Unsupported(format!(
                        "undefined variable `{var_name}` in field access"
                    ))
                })?;
                // Check if this variable is a struct type — if so, further field
                // access is possible. Non-struct locals return None (scalar type).
                let ty = self.local_struct_types.get(&slot).cloned();
                Ok((slot, ty))
            }
            ExprKind::FieldAccess { receiver, field } => {
                // Recursively resolve the receiver to a (slot, optional_type) pair.
                let (recv_slot, recv_ty) = self.resolve_place(receiver)?;

                let field_name = field.text(self.source);

                // FLS §6.10: Tuple field access `.0`, `.1` — integer index.
                // Check tuple lens BEFORE the struct-type check, because tuples
                // have no entry in `local_struct_types` (recv_ty is None for them).
                if self.local_tuple_lens.contains_key(&recv_slot) {
                    let idx: usize = field_name.parse().map_err(|_| {
                        LowerError::Unsupported(format!(
                            "invalid tuple field index `{field_name}`"
                        ))
                    })?;
                    return Ok((recv_slot + idx as u8, None));
                }

                // FLS §6.13: Named struct field — look up field index and offset.
                let type_name = recv_ty.ok_or_else(|| {
                    LowerError::Unsupported(format!(
                        "field access on scalar value (field `{field_name}`)"
                    ))
                })?;

                let field_names = self.struct_defs.get(&type_name).ok_or_else(|| {
                    LowerError::Unsupported(format!("unknown struct type `{type_name}`"))
                })?;
                let field_idx = field_names
                    .iter()
                    .position(|n| n == field_name)
                    .ok_or_else(|| {
                        LowerError::Unsupported(format!(
                            "no field `{field_name}` in struct `{type_name}`"
                        ))
                    })?;

                let offset = self
                    .struct_field_offsets
                    .get(&type_name)
                    .and_then(|o| o.get(field_idx))
                    .copied()
                    .unwrap_or(field_idx);

                // Return the type of this field (None if scalar, Some if nested struct).
                let field_ty = self
                    .struct_field_types
                    .get(&type_name)
                    .and_then(|t| t.get(field_idx))
                    .cloned()
                    .flatten();

                Ok((recv_slot + offset as u8, field_ty))
            }
            _ => Err(LowerError::Unsupported(
                "unsupported place expression (only variables and field accesses supported)".into(),
            )),
        }
    }

    // ── Block lowering ────────────────────────────────────────────────────────

    /// Lower a block to a value without emitting `Ret`.
    ///
    /// Processes all statements in order, then lowers the tail expression
    /// and returns its value. Used by `lower_block` (function body) and by
    /// `lower_expr` for block expressions and if/else branches.
    ///
    /// FLS §6.4: Block expressions.
    /// FLS §6.1.2:37–45: Non-const function bodies must emit runtime code.
    fn lower_block_to_value(&mut self, block: &Block, ret_ty: &IrTy) -> Result<IrValue, LowerError> {
        // FLS §3, §9: Pre-pass — register inner function names so they are
        // visible as direct-call targets throughout the block, even for calls
        // that appear before the function's definition statement.
        //
        // This enables forward calls and mutual recursion between inner functions
        // in the same block (subject to the no-struct-return limitation at this
        // milestone). The names are inserted into `self.fn_names` (now owned)
        // so all subsequent expression lowering in this block sees them.
        for stmt in &block.stmts {
            if let StmtKind::Item(item) = &stmt.kind
                && let ItemKind::Fn(fn_def) = &item.kind
            {
                let name = fn_def.name.text(self.source).to_owned();
                self.fn_names.insert(name);
            }
        }

        for stmt in &block.stmts {
            self.lower_stmt(stmt)?;
        }
        match &block.tail {
            None => Ok(IrValue::Unit),
            Some(tail) => self.lower_expr(tail, ret_ty),
        }
    }

    /// Lower a function body block, appending a final `Ret` instruction.
    ///
    /// FLS §6.4: Block expressions.
    /// FLS §6.19: Return expressions — the tail is the block's return value.
    /// FLS §6.1.2:37–45: Non-const function bodies must emit runtime code.
    fn lower_block(&mut self, block: &Block, ret_ty: &IrTy) -> Result<(), LowerError> {
        let ret_val = self.lower_block_to_value(block, ret_ty)?;
        self.instrs.push(Instr::Ret(ret_val));
        Ok(())
    }

    /// Lower an expression that returns an enum value into pre-allocated stack slots.
    ///
    /// Stores discriminant at `base_slot`, positional fields at `base_slot+1..`.
    /// Used for functions returning enum types (FLS §9, §15).
    ///
    /// Handles:
    /// - Tuple variant constructor `Enum::Variant(field, ...)` — stores discriminant + fields
    /// - Unit variant path `Enum::Variant` — stores discriminant only
    /// - Enum variable path `x` (where x is a local enum variable) — copies all slots
    /// - If-else expression — handles each branch recursively
    /// - Block expression — lowers stmts then handles tail
    ///
    /// FLS §6.1.2:37–45: All stores are runtime instructions.
    /// FLS §15 AMBIGUOUS: The spec does not define a calling convention for
    /// enum-returning functions. Galvanic uses discriminant in x0, fields in x1..xN.
    fn lower_enum_expr_into(
        &mut self,
        expr: &Expr,
        base_slot: u8,
        max_fields: usize,
    ) -> Result<(), LowerError> {
        match &expr.kind {
            // FLS §6.12.1 + §15: Tuple variant constructor `Enum::Variant(f0, f1, ...)`.
            ExprKind::Call { callee, args }
                if matches!(&callee.kind, ExprKind::Path(segs) if segs.len() == 2) =>
            {
                let segs = if let ExprKind::Path(segs) = &callee.kind {
                    segs
                } else {
                    unreachable!()
                };
                let enum_name = segs[0].text(self.source);
                let variant_name = segs[1].text(self.source);
                if let Some((discriminant, _)) = self
                    .enum_defs
                    .get(enum_name)
                    .and_then(|v| v.get(variant_name))
                    .cloned()
                {
                    let disc_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                    self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                    // FLS §4.2, §15: Float fields use StoreF64/StoreF32.
                    for (i, arg) in args.iter().enumerate() {
                        let slot = base_slot + 1 + i as u8;
                        match self.enum_variant_field_float_ty(enum_name, variant_name, i) {
                            Some(IrTy::F64) => {
                                let val = self.lower_expr(arg, &IrTy::F64)?;
                                let src = match val {
                                    IrValue::FReg(r) => r,
                                    _ => return Err(LowerError::Unsupported(
                                        "f64 enum return tuple field did not produce float".into(),
                                    )),
                                };
                                self.instrs.push(Instr::StoreF64 { src, slot });
                                self.slot_float_ty.insert(slot, IrTy::F64);
                            }
                            Some(IrTy::F32) => {
                                let val = self.lower_expr(arg, &IrTy::F32)?;
                                let src = match val {
                                    IrValue::F32Reg(r) => r,
                                    _ => return Err(LowerError::Unsupported(
                                        "f32 enum return tuple field did not produce float".into(),
                                    )),
                                };
                                self.instrs.push(Instr::StoreF32 { src, slot });
                                self.slot_float_ty.insert(slot, IrTy::F32);
                            }
                            _ => {
                                let val = self.lower_expr(arg, &IrTy::I32)?;
                                let src = self.val_to_reg(val)?;
                                self.instrs.push(Instr::Store { src, slot });
                            }
                        }
                    }
                    return Ok(());
                }
                Err(LowerError::Unsupported(
                    "enum return: unknown variant constructor".into(),
                ))
            }

            // FLS §15.3 + §6.11: Named-field enum variant `Enum::Variant { field: expr, ... }`.
            ExprKind::EnumVariantLit { path, fields: lit_fields } if path.len() == 2 => {
                let enum_name = path[0].text(self.source);
                let variant_name = path[1].text(self.source);
                if let Some((discriminant, field_names)) = self
                    .enum_defs
                    .get(enum_name)
                    .and_then(|v| v.get(variant_name))
                    .cloned()
                {
                    let disc_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                    self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                    // Store fields in declaration order.
                    // FLS §4.2, §15: Float fields use StoreF64/StoreF32.
                    for (field_idx, field_name) in field_names.iter().enumerate() {
                        let slot = base_slot + 1 + field_idx as u8;
                        let field_init = lit_fields
                            .iter()
                            .find(|(f, _)| f.text(self.source) == field_name.as_str())
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "missing field `{field_name}` in `{enum_name}::{variant_name}` literal"
                                ))
                            })?;
                        match self.enum_variant_field_float_ty(enum_name, variant_name, field_idx) {
                            Some(IrTy::F64) => {
                                let val = self.lower_expr(&field_init.1, &IrTy::F64)?;
                                let src = match val {
                                    IrValue::FReg(r) => r,
                                    _ => return Err(LowerError::Unsupported(
                                        "f64 enum return named field did not produce float".into(),
                                    )),
                                };
                                self.instrs.push(Instr::StoreF64 { src, slot });
                                self.slot_float_ty.insert(slot, IrTy::F64);
                            }
                            Some(IrTy::F32) => {
                                let val = self.lower_expr(&field_init.1, &IrTy::F32)?;
                                let src = match val {
                                    IrValue::F32Reg(r) => r,
                                    _ => return Err(LowerError::Unsupported(
                                        "f32 enum return named field did not produce float".into(),
                                    )),
                                };
                                self.instrs.push(Instr::StoreF32 { src, slot });
                                self.slot_float_ty.insert(slot, IrTy::F32);
                            }
                            _ => {
                                let val = self.lower_expr(&field_init.1, &IrTy::I32)?;
                                let src = self.val_to_reg(val)?;
                                self.instrs.push(Instr::Store { src, slot });
                            }
                        }
                    }
                    return Ok(());
                }
                Err(LowerError::Unsupported(
                    "enum return: unknown named-field variant".into(),
                ))
            }

            // FLS §6.3 + §15: Unit variant path `Enum::Variant`.
            ExprKind::Path(segs) if segs.len() == 2 => {
                let enum_name = segs[0].text(self.source);
                let variant_name = segs[1].text(self.source);
                if let Some((discriminant, field_names)) = self
                    .enum_defs
                    .get(enum_name)
                    .and_then(|v| v.get(variant_name))
                    .cloned()
                    .filter(|(_, names)| names.is_empty())
                {
                    let _ = field_names;
                    let disc_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                    self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                    return Ok(());
                }
                Err(LowerError::Unsupported(
                    "enum return: non-unit variant used as path (needs call syntax)".into(),
                ))
            }

            // FLS §6.3 + §15: Enum variable path `x` (copy all slots).
            ExprKind::Path(segs) if segs.len() == 1 => {
                let var_name = segs[0].text(self.source);
                let src_base = *self.locals.get(var_name).ok_or_else(|| {
                    LowerError::Unsupported(format!("enum return: undefined variable `{var_name}`"))
                })?;
                if !self.local_enum_types.contains_key(&src_base) {
                    return Err(LowerError::Unsupported(format!(
                        "enum return: variable `{var_name}` is not an enum"
                    )));
                }
                // Copy discriminant.
                let disc_reg = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: disc_reg, slot: src_base });
                self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                // Copy fields.
                for fi in 0..max_fields {
                    let fr = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: fr, slot: src_base + 1 + fi as u8 });
                    self.instrs.push(Instr::Store { src: fr, slot: base_slot + 1 + fi as u8 });
                }
                Ok(())
            }

            // FLS §6.17: If-else expression — handle each branch recursively.
            ExprKind::If { cond, then_block, else_expr } => {
                let else_label = self.alloc_label();
                let end_label = self.alloc_label();

                let cond_val = self.lower_expr(cond, &IrTy::Bool)?;
                let cond_reg = self.val_to_reg(cond_val)?;
                self.instrs.push(Instr::CondBranch { reg: cond_reg, label: else_label });

                // Then branch.
                for stmt in &then_block.stmts {
                    self.lower_stmt(stmt)?;
                }
                if let Some(tail) = then_block.tail.as_deref() {
                    self.lower_enum_expr_into(tail, base_slot, max_fields)?;
                }
                self.instrs.push(Instr::Branch(end_label));

                // Else branch.
                self.instrs.push(Instr::Label(else_label));
                match else_expr {
                    Some(e) => self.lower_enum_expr_into(e, base_slot, max_fields)?,
                    None => {
                        return Err(LowerError::Unsupported(
                            "enum-returning if expression must have an else branch".into(),
                        ))
                    }
                }

                self.instrs.push(Instr::Label(end_label));
                Ok(())
            }

            // FLS §6.4: Block expression — lower stmts then handle tail.
            ExprKind::Block(block) => {
                for stmt in &block.stmts {
                    self.lower_stmt(stmt)?;
                }
                match block.tail.as_deref() {
                    Some(tail) => self.lower_enum_expr_into(tail, base_slot, max_fields),
                    None => Err(LowerError::Unsupported(
                        "enum-returning block must have a tail expression".into(),
                    )),
                }
            }

            // FLS §6.18, §15: Match expression producing an enum value.
            //
            // Each arm body is lowered via `lower_enum_expr_into`, storing the
            // discriminant + fields into base_slot..base_slot+max_fields. The
            // pattern check follows the same strategy as scalar match: emit a
            // comparison and `cbz` to the next arm label on mismatch.
            //
            // FLS §6.18: "A match expression is used to branch over the possible
            // values of the scrutinee operand." Arms are tried in source order.
            // FLS §15: Enum values occupy 1+max_fields consecutive slots:
            // slot 0 = discriminant, slots 1..max_fields = fields.
            // FLS §6.1.2:37–45: All comparisons and stores are runtime instructions.
            //
            // Cache-line note: each literal-pattern arm emits ~5 instructions
            // (ldr + mov + cmp + cbz + disc+field stores). Two short arms fit
            // in a 64-byte cache line.
            ExprKind::Match { scrutinee, arms } => {
                if arms.is_empty() {
                    return Err(LowerError::Unsupported(
                        "enum-returning match expression with no arms".into(),
                    ));
                }

                // FLS §6.18: The scrutinee is evaluated once before any arm is
                // tried. We spill it to a stack slot so each arm's pattern check
                // can reload it without re-evaluating the scrutinee.
                let scrut_val = self.lower_expr(scrutinee, &IrTy::I32)?;
                let scrut_reg = self.val_to_reg(scrut_val)?;
                let scrut_slot = self.alloc_slot()?;
                self.instrs.push(Instr::Store { src: scrut_reg, slot: scrut_slot });

                // Split into checked arms (all but last) and the default arm.
                // FLS §6.18: Arms are tested in source order; the last arm is
                // emitted without a pattern check (unconditional fall-through).
                let (checked_arms, default_arm) = arms.split_at(arms.len() - 1);
                let exit_label = self.alloc_label();

                for arm in checked_arms {
                    let next_label = self.alloc_label();

                    match &arm.pat {
                        // FLS §5.1: Wildcard — always matches. Check guard only.
                        Pat::Wildcard => {
                            if let Some(guard) = &arm.guard {
                                let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                let gr = self.val_to_reg(gv)?;
                                self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                            }
                            self.lower_enum_expr_into(&arm.body, base_slot, max_fields)?;
                            self.instrs.push(Instr::Branch(exit_label));
                            self.instrs.push(Instr::Label(next_label));
                            continue;
                        }

                        // FLS §5.1.4: Identifier pattern — always matches, binds name.
                        // FLS §6.18: Binding is visible inside the arm body and guard.
                        Pat::Ident(span) => {
                            let name = span.text(self.source);
                            let bind_slot = self.alloc_slot()?;
                            let bind_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                            self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                            self.locals.insert(name, bind_slot);
                            if let Some(guard) = &arm.guard {
                                let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                let gr = self.val_to_reg(gv)?;
                                self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                            }
                            self.lower_enum_expr_into(&arm.body, base_slot, max_fields)?;
                            self.locals.remove(name);
                            self.instrs.push(Instr::Branch(exit_label));
                            self.instrs.push(Instr::Label(next_label));
                            continue;
                        }

                        // FLS §5.2: Literal pattern — emit equality check on scrutinee.
                        Pat::LitInt(n) => {
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, *n as i32));
                            let eq_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp { op: IrBinOp::Eq, dst: eq_reg, lhs: s_reg, rhs: p_reg });
                            self.instrs.push(Instr::CondBranch { reg: eq_reg, label: next_label });
                        }

                        Pat::NegLitInt(n) => {
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, -(*n as i32)));
                            let eq_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp { op: IrBinOp::Eq, dst: eq_reg, lhs: s_reg, rhs: p_reg });
                            self.instrs.push(Instr::CondBranch { reg: eq_reg, label: next_label });
                        }

                        Pat::LitBool(b) => {
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, *b as i32));
                            let eq_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp { op: IrBinOp::Eq, dst: eq_reg, lhs: s_reg, rhs: p_reg });
                            self.instrs.push(Instr::CondBranch { reg: eq_reg, label: next_label });
                        }

                        _ => {
                            return Err(LowerError::Unsupported(
                                "match arm pattern type not yet supported in enum-returning match".into(),
                            ));
                        }
                    }

                    // Pattern matched: check guard (if any), then lower arm body.
                    if let Some(guard) = &arm.guard {
                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                        let gr = self.val_to_reg(gv)?;
                        self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                    }
                    self.lower_enum_expr_into(&arm.body, base_slot, max_fields)?;
                    self.instrs.push(Instr::Branch(exit_label));
                    self.instrs.push(Instr::Label(next_label));
                }

                // Default (last) arm — no pattern check.
                // FLS §6.18: The last arm is typically a wildcard or identifier
                // pattern; it is executed unconditionally if all prior arms failed.
                let default = &default_arm[0];
                match &default.pat {
                    Pat::Ident(span) => {
                        let name = span.text(self.source);
                        let bind_slot = self.alloc_slot()?;
                        let bind_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                        self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                        self.locals.insert(name, bind_slot);
                        self.lower_enum_expr_into(&default.body, base_slot, max_fields)?;
                        self.locals.remove(name);
                    }
                    _ => {
                        // Wildcard or literal (exhaustive last arm).
                        self.lower_enum_expr_into(&default.body, base_slot, max_fields)?;
                    }
                }

                self.instrs.push(Instr::Label(exit_label));
                Ok(())
            }

            _ => Err(LowerError::Unsupported(
                "enum return: unsupported expression form".into(),
            )),
        }
    }

    /// Lower an expression that must produce a tuple value into consecutive
    /// stack slots `base_slot..base_slot+n_elems`.
    ///
    /// FLS §6.10: Tuple expressions evaluate each element left-to-right.
    /// FLS §6.17: If/else expressions where both branches produce tuples.
    /// FLS §6.4: Block expressions whose tail produces a tuple.
    ///
    /// This follows the same "expr-into-slots" pattern as `lower_enum_expr_into`
    /// but for tuple types. The slots are pre-allocated by the caller; this
    /// function only emits Store instructions to fill them.
    fn lower_tuple_expr_into(
        &mut self,
        expr: &Expr,
        base_slot: u8,
        n_elems: u8,
    ) -> Result<(), LowerError> {
        match &expr.kind {
            // FLS §6.10: Tuple literal `(e0, e1, ...)`.
            ExprKind::Tuple(elems) => {
                if elems.len() != n_elems as usize {
                    return Err(LowerError::Unsupported(format!(
                        "declared {} return elements but tuple literal has {}",
                        n_elems,
                        elems.len()
                    )));
                }
                // FLS §6:3: Tuple elements are evaluated left-to-right.
                // FLS §6.1.2:37–45: All stores are runtime instructions.
                for (i, elem) in elems.iter().enumerate() {
                    let val = self.lower_expr(elem, &IrTy::I32)?;
                    let src = self.val_to_reg(val)?;
                    self.instrs.push(Instr::Store { src, slot: base_slot + i as u8 });
                }
                Ok(())
            }

            // FLS §6.17: If-else expression — each branch stores its tuple
            // elements into the shared return slots.
            //
            // FLS §6.17: Both branches must produce a value of the same type.
            // Galvanic enforces this structurally: both branches must lower
            // successfully via lower_tuple_expr_into with the same n_elems.
            ExprKind::If { cond, then_block, else_expr } => {
                let else_label = self.alloc_label();
                let end_label = self.alloc_label();

                let cond_val = self.lower_expr(cond, &IrTy::Bool)?;
                let cond_reg = self.val_to_reg(cond_val)?;
                self.instrs.push(Instr::CondBranch { reg: cond_reg, label: else_label });

                // Then branch: lower stmts, then store tuple result.
                for stmt in &then_block.stmts {
                    self.lower_stmt(stmt)?;
                }
                if let Some(tail) = then_block.tail.as_deref() {
                    self.lower_tuple_expr_into(tail, base_slot, n_elems)?;
                } else {
                    return Err(LowerError::Unsupported(
                        "tuple-returning if expression: then branch must have a tail".into(),
                    ));
                }
                self.instrs.push(Instr::Branch(end_label));

                // Else branch: store tuple result.
                self.instrs.push(Instr::Label(else_label));
                match else_expr {
                    Some(e) => self.lower_tuple_expr_into(e, base_slot, n_elems)?,
                    None => {
                        return Err(LowerError::Unsupported(
                            "tuple-returning if expression must have an else branch".into(),
                        ))
                    }
                }

                self.instrs.push(Instr::Label(end_label));
                Ok(())
            }

            // FLS §6.4: Block expression — lower stmts then handle tail.
            ExprKind::Block(block) => {
                for stmt in &block.stmts {
                    self.lower_stmt(stmt)?;
                }
                match block.tail.as_deref() {
                    Some(tail) => self.lower_tuple_expr_into(tail, base_slot, n_elems),
                    None => Err(LowerError::Unsupported(
                        "tuple-returning block must have a tail expression".into(),
                    )),
                }
            }

            // FLS §6.18, §6.10: Match expression producing a tuple value.
            //
            // Each arm body is lowered via `lower_tuple_expr_into`, storing N
            // elements into base_slot..base_slot+n_elems-1. The pattern check
            // follows the same strategy as scalar match: emit a comparison and
            // `cbz` to the next arm label on mismatch.
            //
            // FLS §6.18: "A match expression is used to branch over the possible
            // values of the scrutinee operand." Arms are tried in source order.
            // FLS §6.10: All tuple elements are in declaration order and stored
            // to consecutive stack slots.
            // FLS §6.1.2:37–45: All comparisons and stores are runtime instructions.
            //
            // Cache-line note: each literal-pattern arm emits ~5 instructions
            // (ldr + mov + cmp + cbz + stores = 20+ bytes). Two short arms fit
            // in a 64-byte cache line.
            ExprKind::Match { scrutinee, arms } => {
                if arms.is_empty() {
                    return Err(LowerError::Unsupported(
                        "match expression with no arms".into(),
                    ));
                }

                // FLS §6.18: The scrutinee is evaluated once before any arm is
                // tried. We spill it to a stack slot so each arm's pattern check
                // can reload it without re-evaluating the scrutinee.
                let scrut_val = self.lower_expr(scrutinee, &IrTy::I32)?;
                let scrut_reg = self.val_to_reg(scrut_val)?;
                let scrut_slot = self.alloc_slot()?;
                self.instrs.push(Instr::Store { src: scrut_reg, slot: scrut_slot });

                // Split into checked arms (all but last) and the default arm.
                // FLS §6.18: Arms are tested in source order; the last arm is
                // emitted without a pattern check (unconditional fall-through).
                let (checked_arms, default_arm) = arms.split_at(arms.len() - 1);
                let exit_label = self.alloc_label();

                for arm in checked_arms {
                    let next_label = self.alloc_label();

                    match &arm.pat {
                        // FLS §5.1: Wildcard — always matches. Check guard only.
                        Pat::Wildcard => {
                            if let Some(guard) = &arm.guard {
                                let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                let gr = self.val_to_reg(gv)?;
                                self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                            }
                            self.lower_tuple_expr_into(&arm.body, base_slot, n_elems)?;
                            self.instrs.push(Instr::Branch(exit_label));
                            self.instrs.push(Instr::Label(next_label));
                            continue;
                        }

                        // FLS §5.1.4: Identifier pattern — always matches, binds name.
                        // FLS §6.18: Binding is visible inside the arm body and guard.
                        Pat::Ident(span) => {
                            let name = span.text(self.source);
                            let bind_slot = self.alloc_slot()?;
                            let bind_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                            self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                            self.locals.insert(name, bind_slot);
                            if let Some(guard) = &arm.guard {
                                let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                let gr = self.val_to_reg(gv)?;
                                self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                            }
                            self.lower_tuple_expr_into(&arm.body, base_slot, n_elems)?;
                            self.locals.remove(name);
                            self.instrs.push(Instr::Branch(exit_label));
                            self.instrs.push(Instr::Label(next_label));
                            continue;
                        }

                        // FLS §5.4: Literal/path pattern — emit equality check.
                        // LitInt, NegLitInt, LitBool, or two-segment path (enum variant).
                        Pat::LitInt(n) => {
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, *n as i32));
                            let eq_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp { op: IrBinOp::Eq, dst: eq_reg, lhs: s_reg, rhs: p_reg });
                            self.instrs.push(Instr::CondBranch { reg: eq_reg, label: next_label });
                        }

                        Pat::NegLitInt(n) => {
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, -(*n as i32)));
                            let eq_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp { op: IrBinOp::Eq, dst: eq_reg, lhs: s_reg, rhs: p_reg });
                            self.instrs.push(Instr::CondBranch { reg: eq_reg, label: next_label });
                        }

                        Pat::LitBool(b) => {
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, *b as i32));
                            let eq_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp { op: IrBinOp::Eq, dst: eq_reg, lhs: s_reg, rhs: p_reg });
                            self.instrs.push(Instr::CondBranch { reg: eq_reg, label: next_label });
                        }

                        _ => {
                            return Err(LowerError::Unsupported(
                                "match arm pattern type not yet supported in tuple-returning match".into(),
                            ));
                        }
                    }

                    // Pattern matched: check guard (if any), then lower the arm body.
                    if let Some(guard) = &arm.guard {
                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                        let gr = self.val_to_reg(gv)?;
                        self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                    }
                    self.lower_tuple_expr_into(&arm.body, base_slot, n_elems)?;
                    self.instrs.push(Instr::Branch(exit_label));
                    self.instrs.push(Instr::Label(next_label));
                }

                // Default (last) arm — no pattern check.
                // FLS §6.18: The last arm is typically a wildcard or identifier
                // pattern; it is executed unconditionally if all prior arms failed.
                let default = &default_arm[0];
                match &default.pat {
                    Pat::Ident(span) => {
                        let name = span.text(self.source);
                        let bind_slot = self.alloc_slot()?;
                        let bind_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                        self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                        self.locals.insert(name, bind_slot);
                        self.lower_tuple_expr_into(&default.body, base_slot, n_elems)?;
                        self.locals.remove(name);
                    }
                    _ => {
                        // Wildcard or literal (exhaustive last arm).
                        self.lower_tuple_expr_into(&default.body, base_slot, n_elems)?;
                    }
                }

                self.instrs.push(Instr::Label(exit_label));
                Ok(())
            }

            _ => Err(LowerError::Unsupported(
                "tuple return: unsupported expression form".into(),
            )),
        }
    }

    /// Lower an expression that produces a named struct value into pre-allocated
    /// stack slots.
    ///
    /// Stores the N fields of `struct_name` into `base_slot..base_slot+n_fields-1`.
    /// Used for functions returning named struct types (FLS §9, §6.11, §6.17).
    ///
    /// Handles:
    /// - Struct literal `S { field: expr, ... }` — stores fields in declaration order
    /// - If-else expression — each branch stores into the same slots
    /// - Block expression — lowers statements then handles tail
    /// - Variable path `x` (where x is a local of struct type) — copies all N slots
    ///
    /// FLS §6.11: Struct expression field initializers are evaluated in source order,
    /// stored in declaration order for layout stability.
    /// FLS §6.17: If-else expression — both branches must yield the same struct type.
    /// FLS §6.4: Block expressions — statements then tail.
    /// FLS §6.1.2:37–45: All stores are runtime instructions.
    ///
    /// Cache-line note: a 2-field struct literal emits 2 store instructions = 8 bytes,
    /// fitting alongside the RetFields sequence in a single 64-byte cache line.
    fn lower_struct_expr_into(
        &mut self,
        expr: &Expr,
        base_slot: u8,
        n_fields: usize,
        struct_name: &str,
    ) -> Result<(), LowerError> {
        match &expr.kind {
            // FLS §6.11: Struct literal `S { field: expr, ... }`.
            //
            // Fields may appear in any order in the source; they are stored in
            // declaration order. If struct update syntax (`..base`) is used,
            // unspecified fields are copied from the base variable.
            ExprKind::StructLit { name: sn, fields: lit_fields, base: update_base, .. } => {
                let actual_name = sn.text(self.source);
                let field_names = self.struct_defs
                    .get(actual_name)
                    .ok_or_else(|| {
                        LowerError::Unsupported(format!("unknown struct `{actual_name}`"))
                    })?
                    .clone();
                for (field_idx, field_name) in field_names.iter().enumerate() {
                    let slot = base_slot + field_idx as u8;
                    if let Some(field_init) = lit_fields
                        .iter()
                        .find(|(f, _)| f.text(self.source) == field_name.as_str())
                    {
                        // Explicitly provided field initializer.
                        let val = self.lower_expr(&field_init.1, &IrTy::I32)?;
                        let src = self.val_to_reg(val)?;
                        self.instrs.push(Instr::Store { src, slot });
                    } else if let Some(base_expr) = update_base.as_deref() {
                        // FLS §6.11: Struct update syntax — copy this field from base.
                        let base_var = match &base_expr.kind {
                            ExprKind::Path(segs) if segs.len() == 1 => segs[0].text(self.source),
                            _ => {
                                return Err(LowerError::Unsupported(
                                    "struct update base must be a simple variable path".into(),
                                ))
                            }
                        };
                        let base_base = *self.locals.get(base_var).ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "struct update: undefined base variable `{base_var}`"
                            ))
                        })?;
                        let offset = self
                            .struct_field_offsets
                            .get(actual_name)
                            .and_then(|o| o.get(field_idx))
                            .copied()
                            .unwrap_or(field_idx);
                        let tmp = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: tmp, slot: base_base + offset as u8 });
                        self.instrs.push(Instr::Store { src: tmp, slot });
                    } else {
                        return Err(LowerError::Unsupported(format!(
                            "missing field `{field_name}` in `{actual_name}` literal"
                        )));
                    }
                }
                Ok(())
            }

            // FLS §6.17: If-else expression — each branch stores the struct fields
            // into the shared return slots.
            //
            // FLS §6.17: Both branches must produce a value of the same struct type.
            // Galvanic enforces this structurally: both branches must lower
            // successfully via lower_struct_expr_into with the same struct_name.
            //
            // FLS §6.1.2:37–45: The condition check and all stores are runtime.
            //
            // Cache-line note: the condition check emits 2 instructions (ldr + cbz).
            // Each struct-literal branch emits N stores (N×4 bytes).
            ExprKind::If { cond, then_block, else_expr } => {
                let else_label = self.alloc_label();
                let end_label = self.alloc_label();

                let cond_val = self.lower_expr(cond, &IrTy::Bool)?;
                let cond_reg = self.val_to_reg(cond_val)?;
                self.instrs.push(Instr::CondBranch { reg: cond_reg, label: else_label });

                // Then branch: lower statements then store struct result.
                for stmt in &then_block.stmts {
                    self.lower_stmt(stmt)?;
                }
                if let Some(tail) = then_block.tail.as_deref() {
                    self.lower_struct_expr_into(tail, base_slot, n_fields, struct_name)?;
                } else {
                    return Err(LowerError::Unsupported(
                        "struct-returning if expression: then branch must have a tail".into(),
                    ));
                }
                self.instrs.push(Instr::Branch(end_label));

                // Else branch: store struct result.
                self.instrs.push(Instr::Label(else_label));
                match else_expr {
                    Some(e) => self.lower_struct_expr_into(e, base_slot, n_fields, struct_name)?,
                    None => {
                        return Err(LowerError::Unsupported(
                            "struct-returning if expression must have an else branch".into(),
                        ))
                    }
                }

                self.instrs.push(Instr::Label(end_label));
                Ok(())
            }

            // FLS §6.4: Block expression — lower statements then handle tail.
            ExprKind::Block(block) => {
                for stmt in &block.stmts {
                    self.lower_stmt(stmt)?;
                }
                match block.tail.as_deref() {
                    Some(tail) => {
                        self.lower_struct_expr_into(tail, base_slot, n_fields, struct_name)
                    }
                    None => Err(LowerError::Unsupported(
                        "struct-returning block must have a tail expression".into(),
                    )),
                }
            }

            // FLS §6.3, §7.1: Variable path — copy all struct slots from the source.
            //
            // Reading a local variable in a value context copies its contents
            // (value semantics, FLS §7.1). For a struct with N fields we copy
            // each of the N consecutive stack slots.
            //
            // Cache-line note: N copies = N ldr + N str = 2N instructions.
            ExprKind::Path(segs) if segs.len() == 1 => {
                let var_name = segs[0].text(self.source);
                let src_base = *self.locals.get(var_name).ok_or_else(|| {
                    LowerError::Unsupported(format!(
                        "struct return: undefined variable `{var_name}`"
                    ))
                })?;
                for i in 0..n_fields as u8 {
                    let tmp = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: tmp, slot: src_base + i });
                    self.instrs.push(Instr::Store { src: tmp, slot: base_slot + i });
                }
                Ok(())
            }

            // FLS §6.18, §6.11: Match expression producing a struct value.
            //
            // Each arm body is lowered via `lower_struct_expr_into`, storing N
            // fields into base_slot..base_slot+n_fields-1. The pattern check
            // follows the same strategy as scalar match: emit a comparison and
            // `cbz` to the next arm label on mismatch.
            //
            // FLS §6.18: "A match expression is used to branch over the possible
            // values of the scrutinee operand." Arms are tried in source order.
            // FLS §6.11: All struct fields are stored in declaration order to
            // consecutive stack slots.
            // FLS §6.1.2:37–45: All comparisons and stores are runtime instructions.
            //
            // Cache-line note: each literal-pattern arm emits ~5 instructions
            // (ldr + mov + cmp + cbz + N stores). Compact arms fit in a 64-byte
            // cache line.
            ExprKind::Match { scrutinee, arms } => {
                if arms.is_empty() {
                    return Err(LowerError::Unsupported(
                        "struct-returning match expression with no arms".into(),
                    ));
                }

                // FLS §6.18: The scrutinee is evaluated once before any arm is
                // tried. We spill it to a stack slot so each arm's pattern check
                // can reload it without re-evaluating the scrutinee.
                let scrut_val = self.lower_expr(scrutinee, &IrTy::I32)?;
                let scrut_reg = self.val_to_reg(scrut_val)?;
                let scrut_slot = self.alloc_slot()?;
                self.instrs.push(Instr::Store { src: scrut_reg, slot: scrut_slot });

                // Split into checked arms (all but last) and the default arm.
                // FLS §6.18: Arms are tested in source order; the last arm is
                // emitted without a pattern check (unconditional fall-through).
                let (checked_arms, default_arm) = arms.split_at(arms.len() - 1);
                let exit_label = self.alloc_label();

                for arm in checked_arms {
                    let next_label = self.alloc_label();

                    match &arm.pat {
                        // FLS §5.1: Wildcard — always matches. Check guard only.
                        Pat::Wildcard => {
                            if let Some(guard) = &arm.guard {
                                let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                let gr = self.val_to_reg(gv)?;
                                self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                            }
                            self.lower_struct_expr_into(&arm.body, base_slot, n_fields, struct_name)?;
                            self.instrs.push(Instr::Branch(exit_label));
                            self.instrs.push(Instr::Label(next_label));
                            continue;
                        }

                        // FLS §5.1.4: Identifier pattern — always matches, binds name.
                        // FLS §6.18: Binding is visible inside the arm body and guard.
                        Pat::Ident(span) => {
                            let name = span.text(self.source);
                            let bind_slot = self.alloc_slot()?;
                            let bind_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                            self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                            self.locals.insert(name, bind_slot);
                            if let Some(guard) = &arm.guard {
                                let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                let gr = self.val_to_reg(gv)?;
                                self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                            }
                            self.lower_struct_expr_into(&arm.body, base_slot, n_fields, struct_name)?;
                            self.locals.remove(name);
                            self.instrs.push(Instr::Branch(exit_label));
                            self.instrs.push(Instr::Label(next_label));
                            continue;
                        }

                        // FLS §5.4: Literal/path pattern — emit equality check.
                        Pat::LitInt(n) => {
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, *n as i32));
                            let eq_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp { op: IrBinOp::Eq, dst: eq_reg, lhs: s_reg, rhs: p_reg });
                            self.instrs.push(Instr::CondBranch { reg: eq_reg, label: next_label });
                        }

                        Pat::NegLitInt(n) => {
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, -(*n as i32)));
                            let eq_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp { op: IrBinOp::Eq, dst: eq_reg, lhs: s_reg, rhs: p_reg });
                            self.instrs.push(Instr::CondBranch { reg: eq_reg, label: next_label });
                        }

                        Pat::LitBool(b) => {
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, *b as i32));
                            let eq_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp { op: IrBinOp::Eq, dst: eq_reg, lhs: s_reg, rhs: p_reg });
                            self.instrs.push(Instr::CondBranch { reg: eq_reg, label: next_label });
                        }

                        _ => {
                            return Err(LowerError::Unsupported(
                                "match arm pattern type not yet supported in struct-returning match".into(),
                            ));
                        }
                    }

                    // Pattern matched: check guard (if any), then lower the arm body.
                    if let Some(guard) = &arm.guard {
                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                        let gr = self.val_to_reg(gv)?;
                        self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                    }
                    self.lower_struct_expr_into(&arm.body, base_slot, n_fields, struct_name)?;
                    self.instrs.push(Instr::Branch(exit_label));
                    self.instrs.push(Instr::Label(next_label));
                }

                // Default (last) arm — no pattern check.
                // FLS §6.18: The last arm is typically a wildcard or identifier
                // pattern; it is executed unconditionally if all prior arms failed.
                let default = &default_arm[0];
                match &default.pat {
                    Pat::Ident(span) => {
                        let name = span.text(self.source);
                        let bind_slot = self.alloc_slot()?;
                        let bind_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                        self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                        self.locals.insert(name, bind_slot);
                        self.lower_struct_expr_into(&default.body, base_slot, n_fields, struct_name)?;
                        self.locals.remove(name);
                    }
                    _ => {
                        // Wildcard or literal (exhaustive last arm).
                        self.lower_struct_expr_into(&default.body, base_slot, n_fields, struct_name)?;
                    }
                }

                self.instrs.push(Instr::Label(exit_label));
                Ok(())
            }

            _ => Err(LowerError::Unsupported(format!(
                "struct return (`{struct_name}`): unsupported expression form",
            ))),
        }
    }

    // ── Statement lowering ────────────────────────────────────────────────────

    /// Lower one statement to runtime IR instructions.
    ///
    /// FLS §8: Statements.
    fn lower_stmt(&mut self, stmt: &crate::ast::Stmt) -> Result<(), LowerError> {
        match &stmt.kind {
            // FLS §8.1: Let statement — allocate a stack slot and optionally
            // store the initializer value. The variable name is registered in
            // `locals` so that later path expressions can emit a Load.
            //
            // FLS §8.1: "A LetStatement may optionally have an Initializer."
            // When no initializer is present the slot is allocated but left
            // uninitialized (no Store is emitted). A later plain-assignment
            // expression statement (`x = expr;`) will store to this slot via
            // the `BinOp::Assign` path in `lower_expr`.
            //
            // FLS §8.1 NOTE: The spec requires definite initialization before
            // use (via flow analysis). Galvanic does not yet enforce this —
            // reading an uninitialized slot produces architecturally undefined
            // behavior. Programs that always initialize before use are correct.
            //
            // FLS §6.1.2:37–45: Any store instruction is a runtime instruction;
            // the initializer (when present) is evaluated at runtime.
            StmtKind::Let { pat, ty, init } => {
                // FLS §5.10.3: Tuple pattern in let position — `let (a, b) = t;`.
                //
                // Handled before the scalar special-case chain since it
                // doesn't produce a single `var_name`. Supports two init forms:
                //
                // 1. Tuple literal RHS: `let (a, b) = (e0, e1);` — evaluate
                //    each element, store to fresh slots, bind names.
                // 2. Variable RHS:      `let (a, b) = pair;` — `pair` must be
                //    a known tuple variable; bind names to its existing slots.
                //
                // Sub-patterns: only `Pat::Ident` (binding) and `Pat::Wildcard`
                // (discard) are supported at this milestone (FLS §5.10.3).
                //
                // FLS §6.1.2:37–45: All stores are runtime instructions.
                // Cache-line note: literal init costs N stores (4N bytes);
                // variable rebind costs 0 instructions (slot alias only).
                if let Pat::Tuple(pats) = pat {
                    // Case 1: init is a tuple literal `(e0, e1, ...)`.
                    //
                    // Supports nested `Pat::Tuple` sub-patterns recursively via
                    // `lower_tuple_pat_from_literal` (FLS §5.10.3). Each leaf
                    // `Pat::Ident` gets its own stack slot; wildcards are evaluated
                    // for side effects but bind nothing.
                    //
                    // FLS §6.4:14: Elements evaluated left-to-right.
                    // FLS §6.1.2:37–45: All stores are runtime instructions.
                    // Cache-line note: N leaf slots → N stores (4N bytes).
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::Tuple(elems) = &init_expr.kind
                    {
                        let base_slot =
                            self.lower_tuple_pat_from_literal(pats, elems)?;
                        // Register top-level tuple length for potential re-access
                        // via `.0`, `.1` field expressions (FLS §6.10).
                        if !pats.is_empty() {
                            self.local_tuple_lens.insert(base_slot, pats.len());
                        }
                        return Ok(());
                    }

                    // Case 2: init is a simple variable path whose value is
                    // a known tuple (registered in `local_tuple_lens`).
                    //
                    // This is a zero-instruction rebind: we simply register each
                    // sub-pattern name against the source variable's slots.
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::Path(segs) = &init_expr.kind
                        && segs.len() == 1
                    {
                        let src_name = segs[0].text(self.source);
                        let src_slot = *self.locals.get(src_name).ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "undefined variable `{src_name}` in tuple destructure"
                            ))
                        })?;
                        let src_len = *self.local_tuple_lens.get(&src_slot).ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "variable `{src_name}` is not a tuple"
                            ))
                        })?;
                        if src_len != pats.len() {
                            return Err(LowerError::Unsupported(format!(
                                "tuple pattern has {} elements but `{src_name}` has {src_len}",
                                pats.len()
                            )));
                        }
                        for (i, sub_pat) in pats.iter().enumerate() {
                            match sub_pat {
                                Pat::Ident(span) => {
                                    let name = span.text(self.source);
                                    self.locals.insert(name, src_slot + i as u8);
                                }
                                Pat::Wildcard => {}
                                _ => {
                                    return Err(LowerError::Unsupported(
                                        "nested tuple pattern not yet supported".into(),
                                    ));
                                }
                            }
                        }
                        return Ok(());
                    }

                    // Case 3: init is a call to a tuple-returning free function.
                    //
                    // `let (a, b) = pair(x)` where `pair` is in
                    // `tuple_return_free_fns`. The callee returns element values in
                    // x0..x{N-1} via RetFields. We allocate N consecutive slots and
                    // emit CallMut to write x0..x{N-1} into those slots after the bl,
                    // then bind each pattern element to its slot.
                    //
                    // FLS §6.10, §9: Tuple-returning function calling convention.
                    // FLS §5.10.3: Tuple pattern destructuring.
                    // FLS §6.1.2:37–45: All spills are runtime store instructions.
                    // Cache-line note: N arg moves + bl + N stores = (2N+1) instructions.
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::Call { callee, args } = &init_expr.kind
                        && let ExprKind::Path(segs) = &callee.kind
                        && segs.len() == 1
                    {
                        let fn_name = segs[0].text(self.source).to_owned();
                        if let Some(&n_elems) = self.tuple_return_free_fns.get(&fn_name) {
                            if n_elems != pats.len() {
                                return Err(LowerError::Unsupported(format!(
                                    "tuple pattern has {} elements but `{fn_name}` returns {n_elems}",
                                    pats.len()
                                )));
                            }
                            // Evaluate arguments and collect their virtual registers.
                            let mut arg_regs: Vec<u8> = Vec::new();
                            for arg_expr in args.iter() {
                                let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                                let reg = self.val_to_reg(val)?;
                                arg_regs.push(reg);
                            }
                            // Allocate N consecutive slots for the returned tuple.
                            let base_slot = self.alloc_slot()?;
                            for _ in 1..n_elems {
                                self.alloc_slot()?;
                            }
                            self.has_calls = true;
                            // CallMut: call fn, then store x0..x{N-1} to
                            // base_slot..base_slot+N-1.
                            self.instrs.push(Instr::CallMut {
                                name: fn_name,
                                args: arg_regs,
                                write_back_slot: base_slot,
                                n_fields: n_elems as u8,
                            });
                            // Register the base slot as a tuple for field access (.0, .1).
                            self.local_tuple_lens.insert(base_slot, n_elems);
                            // Bind each pattern element to its slot.
                            for (i, sub_pat) in pats.iter().enumerate() {
                                match sub_pat {
                                    Pat::Ident(span) => {
                                        let name = span.text(self.source);
                                        self.locals.insert(name, base_slot + i as u8);
                                    }
                                    Pat::Wildcard => {}
                                    _ => {
                                        return Err(LowerError::Unsupported(
                                            "only ident and wildcard patterns supported in \
                                             tuple-returning call destructure"
                                                .into(),
                                        ));
                                    }
                                }
                            }
                            return Ok(());
                        }
                    }

                    return Err(LowerError::Unsupported(
                        "tuple destructuring requires a tuple literal, simple variable, \
                         or tuple-returning function call as initializer"
                            .into(),
                    ));
                }

                // FLS §5.1.8 + §8.1: Slice/array pattern in let binding.
                //
                // `let [a, b, c] = arr;` destructures a fixed-size array by index.
                //
                // Supported initializer forms:
                //   1. Variable path — `let [a, b, c] = arr;` — load each element
                //      by index (LoadIndexed) and store to fresh slots; zero extra
                //      stack overhead if the pattern arity matches the array length.
                //   2. Array literal — `let [a, b, c] = [10, 20, 30];` — evaluate
                //      each element expression and store to fresh slots.
                //
                // FLS §5.1.8: Sub-patterns are matched left-to-right against array
                //   elements at indices 0, 1, …, N-1.
                // FLS §6.1.2:37–45: All loads and stores are runtime instructions.
                // Cache-line note: N-element destructure emits up to 2N instructions
                //   (N LoadIndexed + N Store = 8N bytes) — 8 elements per 64-byte line.
                if let Pat::Slice(pats) = pat {
                    // Case 1: init is a local array variable path.
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::Path(segs) = &init_expr.kind
                        && segs.len() == 1
                    {
                        let src_name = segs[0].text(self.source);
                        if let Some(src_slot) = self.locals.get(src_name).copied()
                            && let Some(&arr_len) = self.local_array_lens.get(&src_slot)
                        {
                            if arr_len != pats.len() {
                                return Err(LowerError::Unsupported(format!(
                                    "slice pattern has {} elements but `{src_name}` has {arr_len}",
                                    pats.len()
                                )));
                            }
                            // Emit LoadIndexed + Store for each Ident sub-pattern.
                            // FLS §6.9: Array indexing; FLS §5.1.8: element binding.
                            for (i, sub_pat) in pats.iter().enumerate() {
                                match sub_pat {
                                    Pat::Ident(span) => {
                                        let name = span.text(self.source);
                                        if name != "_" {
                                            let dst = self.alloc_reg()?;
                                            let idx_reg = self.alloc_reg()?;
                                            self.instrs.push(Instr::LoadImm(idx_reg, i as i32));
                                            self.instrs.push(Instr::LoadIndexed {
                                                dst,
                                                base_slot: src_slot,
                                                index_reg: idx_reg,
                                            });
                                            let elem_slot = self.alloc_slot()?;
                                            self.instrs.push(Instr::Store { src: dst, slot: elem_slot });
                                            self.locals.insert(name, elem_slot);
                                        }
                                    }
                                    Pat::Wildcard => {}
                                    _ => {
                                        return Err(LowerError::Unsupported(
                                            "only ident and wildcard sub-patterns supported \
                                             in slice pattern at this milestone"
                                                .into(),
                                        ));
                                    }
                                }
                            }
                            return Ok(());
                        }
                    }

                    // Case 2: init is an array literal `[e0, e1, …]`.
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::Array(elems) = &init_expr.kind
                    {
                        if elems.len() != pats.len() {
                            return Err(LowerError::Unsupported(format!(
                                "slice pattern has {} elements but array literal has {}",
                                pats.len(),
                                elems.len()
                            )));
                        }
                        for (sub_pat, elem_expr) in pats.iter().zip(elems.iter()) {
                            match sub_pat {
                                Pat::Ident(span) => {
                                    let name = span.text(self.source);
                                    // Evaluate element and store to fresh slot.
                                    // FLS §6.4:14: Elements evaluated left-to-right.
                                    // FLS §6.1.2:37–45: Store is a runtime instruction.
                                    let val = self.lower_expr(elem_expr, &IrTy::I32)?;
                                    let reg = self.val_to_reg(val)?;
                                    let slot = self.alloc_slot()?;
                                    self.instrs.push(Instr::Store { src: reg, slot });
                                    if name != "_" {
                                        self.locals.insert(name, slot);
                                    }
                                }
                                Pat::Wildcard => {
                                    // Evaluate for side effects but discard.
                                    // FLS §6.4:14: Left-to-right evaluation order preserved.
                                    let _ = self.lower_expr(elem_expr, &IrTy::I32)?;
                                }
                                _ => {
                                    return Err(LowerError::Unsupported(
                                        "only ident and wildcard sub-patterns supported \
                                         in slice pattern at this milestone"
                                            .into(),
                                    ));
                                }
                            }
                        }
                        return Ok(());
                    }

                    return Err(LowerError::Unsupported(
                        "slice/array pattern requires a local array variable or array \
                         literal as initializer"
                            .into(),
                    ));
                }

                // FLS §5.10.2 + §8.1: Struct pattern in let binding.
                //
                // `let StructName { field1, field2, .. } = expr;` binds each named
                // field pattern to the corresponding stack slot of the struct value.
                //
                // Supported initializer forms:
                //   1. Variable path — `let Point { x, y } = p;` — slot aliasing,
                //      zero runtime instructions.
                //   2. Struct literal — `let Point { x, y } = Point { x: 3, y: 4 };` —
                //      evaluate each field and store to fresh slots, then alias names.
                //
                // FLS §6.1.2:37–45: All stores are runtime instructions; no
                // compile-time evaluation of non-const code.
                // Cache-line note: N-field struct occupies N consecutive 8-byte
                // slots. Slot aliasing costs 0 instructions; literal init costs
                // N store instructions.
                if let Pat::StructVariant { path: pat_path, fields: pat_fields } = pat
                    && pat_path.len() == 1
                {
                    let struct_name = pat_path[0].text(self.source);

                    // Case 1: Init is a simple variable path.
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::Path(segs) = &init_expr.kind
                        && segs.len() == 1
                    {
                        let src_name = segs[0].text(self.source);
                        let src_slot =
                            *self.locals.get(src_name).ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "undefined variable `{src_name}` in struct destructure"
                                ))
                            })?;
                        let src_type = self
                            .local_struct_types
                            .get(&src_slot)
                            .cloned()
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "variable `{src_name}` is not a struct (cannot destructure)"
                                ))
                            })?;
                        if src_type != struct_name {
                            return Err(LowerError::Unsupported(format!(
                                "struct pattern `{struct_name}` does not match \
                                 variable type `{src_type}`"
                            )));
                        }
                        // Alias each named field pattern to its source slot.
                        // FLS §5.10.2: supports nested struct sub-patterns via
                        // `bind_struct_fields_from_slot`.
                        self.bind_struct_fields_from_slot(
                            struct_name,
                            src_slot,
                            pat_fields,
                        )?;
                        return Ok(());
                    }

                    // Case 2: Init is an inline struct literal.
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::StructLit {
                            name: lit_name_span,
                            fields: lit_fields,
                            base: lit_base,
                        } = &init_expr.kind
                    {
                        let lit_struct_name = lit_name_span.text(self.source);
                        if lit_struct_name != struct_name {
                            return Err(LowerError::Unsupported(format!(
                                "struct pattern `{struct_name}` does not match \
                                 literal type `{lit_struct_name}`"
                            )));
                        }
                        if lit_base.is_some() {
                            return Err(LowerError::Unsupported(
                                "struct update syntax not supported in struct \
                                 let-pattern"
                                    .into(),
                            ));
                        }
                        let field_names = self
                            .struct_defs
                            .get(struct_name)
                            .cloned()
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown struct type `{struct_name}`"
                                ))
                            })?;
                        let offsets = self
                            .struct_field_offsets
                            .get(struct_name)
                            .cloned()
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown field offsets for struct `{struct_name}`"
                                ))
                            })?;
                        let n_slots = self
                            .struct_sizes
                            .get(struct_name)
                            .copied()
                            .unwrap_or(field_names.len());
                        let field_types = self
                            .struct_field_types
                            .get(struct_name)
                            .cloned()
                            .unwrap_or_default();

                        // Allocate consecutive slots for the temporary struct.
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..n_slots {
                            self.alloc_slot()?;
                        }
                        self.local_struct_types
                            .insert(base_slot, struct_name.to_owned());

                        // Evaluate and store each field in declaration order
                        // (FLS §6.4:14 — left-to-right evaluation order).
                        for (fi, fname) in field_names.iter().enumerate() {
                            let slot = base_slot + offsets[fi] as u8;
                            let field_init = lit_fields
                                .iter()
                                .find(|(fs, _)| fs.text(self.source) == fname.as_str())
                                .ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "missing field `{fname}` in struct literal"
                                    ))
                                })?;
                            let inner_type =
                                field_types.get(fi).and_then(|t| t.clone());
                            if let Some(ref itype) = inner_type
                                && self.struct_defs.contains_key(itype.as_str())
                            {
                                self.store_nested_struct_lit(
                                    &field_init.1,
                                    slot,
                                    itype,
                                )?;
                            } else {
                                // FLS §4.2: Use float store for f64/f32 fields.
                                let fty = self.field_float_ty(struct_name, fi);
                                match fty {
                                    Some(IrTy::F64) => {
                                        let val = self.lower_expr(&field_init.1, &IrTy::F64)?;
                                        let freg = match val {
                                            IrValue::FReg(r) => r,
                                            _ => return Err(LowerError::Unsupported(
                                                "f64 struct field: initializer did not produce a float register".into(),
                                            )),
                                        };
                                        self.instrs.push(Instr::StoreF64 { src: freg, slot });
                                        self.slot_float_ty.insert(slot, IrTy::F64);
                                    }
                                    Some(IrTy::F32) => {
                                        let val = self.lower_expr(&field_init.1, &IrTy::F32)?;
                                        let freg = match val {
                                            IrValue::F32Reg(r) => r,
                                            _ => return Err(LowerError::Unsupported(
                                                "f32 struct field: initializer did not produce an f32 register".into(),
                                            )),
                                        };
                                        self.instrs.push(Instr::StoreF32 { src: freg, slot });
                                        self.slot_float_ty.insert(slot, IrTy::F32);
                                    }
                                    _ => {
                                        let val = self.lower_expr(&field_init.1, &IrTy::I32)?;
                                        let src = self.val_to_reg(val)?;
                                        self.instrs.push(Instr::Store { src, slot });
                                    }
                                }
                            }
                        }

                        // Bind each field pattern to its allocated slot.
                        // FLS §5.10.2: supports nested struct sub-patterns via
                        // `bind_struct_fields_from_slot`.
                        self.bind_struct_fields_from_slot(
                            struct_name,
                            base_slot,
                            pat_fields,
                        )?;
                        return Ok(());
                    }

                    return Err(LowerError::Unsupported(
                        "struct let-pattern requires a struct variable or struct \
                         literal initializer"
                            .into(),
                    ));
                }

                // FLS §5.10.4 + §8.1: Tuple struct pattern in let position.
                //
                // `let Point(x, y) = expr;` — single-segment path followed by
                // positional field patterns. Only `Pat::Ident` (binding) and
                // `Pat::Wildcard` (discard) sub-patterns are supported.
                //
                // Supported initializer forms:
                //   1. Variable path — `let Point(x, y) = p;` — slot aliasing,
                //      zero runtime instructions.
                //   2. Tuple struct constructor call — `let Point(x, y) = Point(3, 4);` —
                //      evaluate each arg and store to fresh slots, then alias names.
                //
                // FLS §6.1.2:37–45: All stores are runtime instructions; no
                // compile-time evaluation of non-const code.
                // Cache-line note: constructor call costs N stores (4N bytes);
                // variable rebind costs 0 instructions (slot alias only).
                if let Pat::TupleStruct { path: pat_path, fields: pat_fields } = pat
                    && pat_path.len() == 1
                {
                    let struct_name = pat_path[0].text(self.source);
                    let n_fields = *self.tuple_struct_defs.get(struct_name).ok_or_else(|| {
                        LowerError::Unsupported(format!(
                            "unknown tuple struct type `{struct_name}` in let pattern"
                        ))
                    })?;
                    if pat_fields.len() != n_fields {
                        return Err(LowerError::Unsupported(format!(
                            "tuple struct pattern has {} fields but `{struct_name}` has {n_fields}",
                            pat_fields.len()
                        )));
                    }

                    // Case 1: Init is a simple variable path — alias slots.
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::Path(segs) = &init_expr.kind
                        && segs.len() == 1
                    {
                        let src_name = segs[0].text(self.source);
                        let src_slot =
                            *self.locals.get(src_name).ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "undefined variable `{src_name}` in tuple struct \
                                     destructure"
                                ))
                            })?;
                        for (i, sub_pat) in pat_fields.iter().enumerate() {
                            match sub_pat {
                                Pat::Ident(bind_span) => {
                                    let bind_name = bind_span.text(self.source);
                                    self.locals.insert(bind_name, src_slot + i as u8);
                                }
                                Pat::Wildcard => {}
                                _ => {
                                    return Err(LowerError::Unsupported(
                                        "only ident/wildcard sub-patterns in tuple \
                                         struct let-pattern"
                                            .into(),
                                    ));
                                }
                            }
                        }
                        return Ok(());
                    }

                    // Case 2: Init is a constructor call `Point(e0, e1, ...)` —
                    // evaluate each argument and store to fresh consecutive slots.
                    if let Some(init_expr) = init.as_ref()
                        && let ExprKind::Call { callee, args } = &init_expr.kind
                        && let ExprKind::Path(call_segs) = &callee.kind
                        && call_segs.len() == 1
                        && call_segs[0].text(self.source) == struct_name
                    {
                        if args.len() != n_fields {
                            return Err(LowerError::Unsupported(format!(
                                "constructor `{struct_name}` called with {} args but \
                                 expects {n_fields}",
                                args.len()
                            )));
                        }
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..n_fields {
                            self.alloc_slot()?;
                        }
                        self.local_tuple_lens.insert(base_slot, n_fields);
                        self.local_tuple_struct_types
                            .insert(base_slot, struct_name.to_owned());
                        // Evaluate and store each argument left-to-right (FLS §6.4:14).
                        for (i, arg_expr) in args.iter().enumerate() {
                            let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                            let src = self.val_to_reg(val)?;
                            self.instrs.push(Instr::Store {
                                src,
                                slot: base_slot + i as u8,
                            });
                        }
                        // Bind each sub-pattern to its slot.
                        for (i, sub_pat) in pat_fields.iter().enumerate() {
                            match sub_pat {
                                Pat::Ident(bind_span) => {
                                    let bind_name = bind_span.text(self.source);
                                    self.locals.insert(bind_name, base_slot + i as u8);
                                }
                                Pat::Wildcard => {}
                                _ => {
                                    return Err(LowerError::Unsupported(
                                        "only ident/wildcard sub-patterns in tuple \
                                         struct let-pattern"
                                            .into(),
                                    ));
                                }
                            }
                        }
                        return Ok(());
                    }

                    return Err(LowerError::Unsupported(
                        "tuple struct let-pattern requires a tuple struct variable \
                         or constructor call initializer"
                            .into(),
                    ));
                }

                // All other patterns: extract `var_name` for the scalar path.
                //
                // FLS §5.11: Wildcard pattern `_` — evaluate init for side
                // effects but bind no name.
                let var_name = match pat {
                    Pat::Ident(span) => span.text(self.source),
                    Pat::Wildcard => {
                        if let Some(init_expr) = init.as_ref() {
                            self.lower_expr(init_expr, &IrTy::I32)?;
                        }
                        return Ok(());
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "complex let pattern not yet supported".into(),
                        ));
                    }
                };

                // FLS §10.1: Struct-returning associated function call as let-binding init.
                //
                // `let p = Point::new(x, y)` where `Point__new` is in `struct_return_fns`.
                // The callee returns field values in x0..x{N-1} via RetFields.
                // We allocate N consecutive slots for the variable and emit CallMut
                // to write x0..x{N-1} into those slots after the bl.
                //
                // This is the same write-back mechanism used by &mut self methods,
                // reused here for constructor-style associated functions.
                //
                // FLS §6.12.1: Call expressions. FLS §10.1: Associated functions.
                // Cache-line note: arg moves + bl + N stores per construction.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Call { callee, args } = &init_expr.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 2
                {
                    let type_name = segs[0].text(self.source);
                    let fn_name_seg = segs[1].text(self.source);
                    let mangled = format!("{type_name}__{fn_name_seg}");
                    if let Some(struct_name) = self.struct_return_fns.get(&mangled).cloned() {
                        let field_names = self.struct_defs
                            .get(&struct_name)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!("unknown struct `{struct_name}`"))
                            })?
                            .clone();
                        let n_fields = field_names.len();

                        // Allocate N consecutive slots for the new struct variable.
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..n_fields {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_struct_types.insert(base_slot, struct_name.clone());

                        // Evaluate arguments and collect their virtual registers.
                        let mut arg_regs: Vec<u8> = Vec::new();
                        for arg_expr in args.iter() {
                            let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }

                        self.has_calls = true;
                        // Emit CallMut: args → bl → store x0..x{N-1} to base_slot..
                        // The callee's RetFields has already put field values in x0..x{N-1}.
                        self.instrs.push(Instr::CallMut {
                            name: mangled,
                            args: arg_regs,
                            write_back_slot: base_slot,
                            n_fields: n_fields as u8,
                        });
                        return Ok(());
                    }
                }

                // FLS §9: Struct-returning free function call as let-binding init.
                //
                // `let p = make_point(a, b)` where `make_point` is in
                // `struct_return_free_fns`. The callee returns field values in
                // x0..x{N-1} via RetFields. We allocate N consecutive slots for
                // the variable and emit CallMut to write x0..x{N-1} into those
                // slots after the bl.
                //
                // This uses the same write-back mechanism as struct-returning
                // associated functions (`struct_return_fns`), extended here to
                // free functions.
                //
                // FLS §6.12.1: Call expressions. FLS §9: Functions.
                // Cache-line note: arg moves + bl + N stores per construction.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Call { callee, args } = &init_expr.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 1
                {
                    let fn_name = segs[0].text(self.source);
                    if let Some(struct_name) = self.struct_return_free_fns.get(fn_name).cloned() {
                        let field_names = self.struct_defs
                            .get(&struct_name)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown struct `{struct_name}` from free fn `{fn_name}`"
                                ))
                            })?
                            .clone();
                        let n_fields = field_names.len();

                        // Allocate N consecutive slots for the new struct variable.
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..n_fields {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_struct_types.insert(base_slot, struct_name.clone());

                        // Evaluate arguments and collect their virtual registers.
                        let mut arg_regs: Vec<u8> = Vec::new();
                        for arg_expr in args.iter() {
                            let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }

                        self.has_calls = true;
                        // Emit CallMut: args → bl → store x0..x{N-1} to base_slot..
                        // The callee's RetFields has already put field values in
                        // x0..x{N-1}.
                        self.instrs.push(Instr::CallMut {
                            name: fn_name.to_owned(),
                            args: arg_regs,
                            write_back_slot: base_slot,
                            n_fields: n_fields as u8,
                        });
                        return Ok(());
                    }
                }

                // FLS §9, §15: Enum-returning free function call as let-binding init.
                //
                // `let x = wrap(n)` where `wrap` is in `enum_return_fns`.
                // The callee returns discriminant + fields in x0..x{1+max_fields-1}
                // via RetFields. We allocate 1+max_fields consecutive slots for the
                // variable and emit CallMut to write them after the bl.
                //
                // FLS §6.12.1: Call expressions. FLS §15: Enum values occupy
                // 1+max_fields consecutive slots.
                // Cache-line note: arg moves + bl + (1+max_fields) stores.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Call { callee, args } = &init_expr.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 1
                {
                    let fn_name = segs[0].text(self.source);
                    if let Some(enum_name) = self.enum_return_fns.get(fn_name).cloned() {
                        let max_fields = self.enum_defs
                            .get(enum_name.as_str())
                            .map(|v| v.values().map(|(_, names)| names.len()).max().unwrap_or(0))
                            .unwrap_or(0);
                        let n_ret = 1 + max_fields as u8;
                        // Allocate discriminant slot + field slots.
                        let base_slot = self.alloc_slot()?;
                        for _ in 0..max_fields {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_enum_types.insert(base_slot, enum_name.clone());
                        // Evaluate arguments.
                        let mut arg_regs: Vec<u8> = Vec::new();
                        for arg_expr in args.iter() {
                            let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }
                        self.has_calls = true;
                        self.instrs.push(Instr::CallMut {
                            name: fn_name.to_owned(),
                            args: arg_regs,
                            write_back_slot: base_slot,
                            n_fields: n_ret,
                        });
                        return Ok(());
                    }
                }

                // FLS §10.1, §6.12.2: `&self` method call returning a struct.
                //
                // `let q = p.translate(dx, dy)` where `Point__translate` is in
                // `struct_return_methods`. The callee returns the new struct's
                // field values in x0..x{N-1} via `RetFields`. We:
                //   1. Load receiver (p) fields into leading arg registers.
                //   2. Lower explicit arguments.
                //   3. Allocate N consecutive slots for the destination variable (q).
                //   4. Emit `CallMut` to write x0..x{N-1} into q's slots after `bl`.
                //
                // The receiver struct's fields are passed by value (copied into
                // registers); the receiver itself is not modified (read-only self).
                //
                // FLS §10.1 AMBIGUOUS: The spec does not define the calling
                // convention for `&self` methods returning struct types. Galvanic
                // uses the same register-packing convention as struct-returning
                // associated functions: fields returned in x0..x{N-1}.
                //
                // FLS §6.1.2:37–45: All instructions are runtime (no const folding).
                // Cache-line note: N self-field loads + arg moves + bl + N return
                // stores = 2N+1 extra instructions compared to a scalar method call.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::MethodCall { receiver, method, args } = &init_expr.kind
                    && let ExprKind::Path(recv_segs) = &receiver.kind
                    && recv_segs.len() == 1
                {
                    let recv_name = recv_segs[0].text(self.source);
                    let method_name = method.text(self.source);
                    if let Some(recv_base_slot) = self.locals.get(recv_name).copied()
                        && let Some(recv_type) = self.local_struct_types.get(&recv_base_slot).cloned() {
                            let mangled = format!("{recv_type}__{method_name}");
                            if let Some(ret_struct_name) = self.struct_return_methods.get(&mangled).cloned() {
                                let field_names = self.struct_defs
                                    .get(ret_struct_name.as_str())
                                    .ok_or_else(|| {
                                        LowerError::Unsupported(format!(
                                            "unknown return struct `{ret_struct_name}`"
                                        ))
                                    })?
                                    .clone();
                                let n_return_fields = field_names.len();

                                // Allocate slots for destination variable (q).
                                let base_slot = self.alloc_slot()?;
                                for _ in 1..n_return_fields {
                                    self.alloc_slot()?;
                                }
                                self.locals.insert(var_name, base_slot);
                                self.local_struct_types.insert(base_slot, ret_struct_name.clone());

                                // Load receiver fields into leading arg registers.
                                let recv_field_names = self.struct_defs
                                    .get(recv_type.as_str())
                                    .ok_or_else(|| {
                                        LowerError::Unsupported(format!(
                                            "unknown receiver struct `{recv_type}`"
                                        ))
                                    })?
                                    .clone();
                                let n_recv_fields = recv_field_names.len();
                                let mut arg_regs: Vec<u8> = Vec::new();
                                for fi in 0..n_recv_fields {
                                    let slot = recv_base_slot + fi as u8;
                                    let reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: reg, slot });
                                    arg_regs.push(reg);
                                }

                                // Lower explicit arguments.
                                for arg_expr in args.iter() {
                                    let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                                    let reg = self.val_to_reg(val)?;
                                    arg_regs.push(reg);
                                }

                                self.has_calls = true;
                                // Emit CallMut: args → bl → store x0..x{N-1} to base_slot..
                                // The callee's RetFields returns new struct fields in x0..x{N-1}.
                                self.instrs.push(Instr::CallMut {
                                    name: mangled,
                                    args: arg_regs,
                                    write_back_slot: base_slot,
                                    n_fields: n_return_fields as u8,
                                });
                                return Ok(());
                            }
                        }
                }

                // FLS §6.11: Struct literal initializer — allocate one slot per
                // scalar field (or N slots for a struct-type field of total size N)
                // and store each field value.
                //
                // FLS §6.13, §4.11: Nested struct fields occupy multiple consecutive
                // slots. The total slot count for the outer struct is `struct_sizes`
                // entry for the struct type; field slot offsets come from
                // `struct_field_offsets`.
                //
                // Cache-line note: an N-slot struct occupies N consecutive 8-byte
                // slots. The base slot is recorded so that chained field access
                // can compute `base_slot + field_offset`.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::StructLit {
                        name: struct_name_span,
                        fields: init_fields,
                        base: init_base,
                    } = &init_expr.kind
                {
                        let struct_name = struct_name_span.text(self.source);
                        // Clone to avoid borrow conflict with self inside the loop.
                        let field_names: Vec<String> = self
                            .struct_defs
                            .get(struct_name)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown struct type `{struct_name}`"
                                ))
                            })?
                            .clone();

                        // FLS §4.11: Allocate the total number of slots for this struct,
                        // which may be greater than field_names.len() when any field is
                        // itself a struct type (each such field occupies multiple slots).
                        let total_slots = self
                            .struct_sizes
                            .get(struct_name)
                            .copied()
                            .unwrap_or(field_names.len());
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..total_slots {
                            self.alloc_slot()?; // consume additional slots for nested fields
                        }

                        self.locals.insert(var_name, base_slot);
                        self.local_struct_types
                            .insert(base_slot, struct_name.to_owned());

                        // FLS §6.11: Struct update syntax `Struct { field: val, ..other }`.
                        // Resolve the base struct's first slot so we can copy unspecified
                        // fields from it. The base must be a simple variable path.
                        //
                        // FLS §6.11: "A struct expression with a base struct specifies one
                        // or more fields for the new value and copies all remaining fields
                        // from the base struct expression."
                        let base_struct_slot: Option<u8> = if let Some(base_expr) = init_base {
                            if let ExprKind::Path(segs) = &base_expr.kind
                                && segs.len() == 1
                            {
                                let base_var = segs[0].text(self.source);
                                Some(*self.locals.get(base_var).ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "unknown variable `{base_var}` in struct update syntax"
                                    ))
                                })?)
                            } else {
                                return Err(LowerError::Unsupported(
                                    "struct update base must be a simple variable path".into(),
                                ));
                            }
                        } else {
                            None
                        };

                        // Pre-fetch field offset and type tables to avoid repeated map
                        // lookups inside the loop. Both are keyed by struct name.
                        let field_offsets: Vec<usize> = self
                            .struct_field_offsets
                            .get(struct_name)
                            .cloned()
                            .unwrap_or_default();
                        let field_types: Vec<Option<String>> = self
                            .struct_field_types
                            .get(struct_name)
                            .cloned()
                            .unwrap_or_default();

                        // Store each field in declaration order.
                        // FLS §6.11: Field initializers are evaluated in source
                        // order but stored in declaration order for layout stability.
                        // FLS §6.11: Fields not explicitly listed are copied from base.
                        for (field_idx, field_name) in field_names.iter().enumerate() {
                            // FLS §4.11: field slot = base + offset (not base + index)
                            // because preceding struct-type fields may span multiple slots.
                            let field_offset = field_offsets
                                .get(field_idx)
                                .copied()
                                .unwrap_or(field_idx);
                            let dst_slot = base_slot + field_offset as u8;

                            if let Some(field_init) = init_fields
                                .iter()
                                .find(|(f, _)| f.text(self.source) == field_name.as_str())
                            {
                                // FLS §6.11: Explicitly provided field — evaluate and store.
                                // If the field is itself a struct type, store recursively
                                // into `total_size` consecutive slots starting at `dst_slot`.
                                let nested_ty = field_types
                                    .get(field_idx)
                                    .cloned()
                                    .flatten();
                                if let Some(nested_type_name) = nested_ty {
                                    // Nested struct field: recursively store the struct literal
                                    // into the target slots. Only struct literals are supported
                                    // as nested struct field values at this milestone.
                                    self.store_nested_struct_lit(
                                        &field_init.1,
                                        dst_slot,
                                        &nested_type_name,
                                    )?;
                                } else {
                                    // FLS §4.2, §6.11: Use float store for f64/f32 fields.
                                    let fty = self.field_float_ty(struct_name, field_idx);
                                    match fty {
                                        Some(IrTy::F64) => {
                                            let val = self.lower_expr(&field_init.1, &IrTy::F64)?;
                                            let freg = match val {
                                                IrValue::FReg(r) => r,
                                                _ => return Err(LowerError::Unsupported(
                                                    "f64 struct field: initializer did not produce a float register".into(),
                                                )),
                                            };
                                            self.instrs.push(Instr::StoreF64 { src: freg, slot: dst_slot });
                                            self.slot_float_ty.insert(dst_slot, IrTy::F64);
                                        }
                                        Some(IrTy::F32) => {
                                            let val = self.lower_expr(&field_init.1, &IrTy::F32)?;
                                            let freg = match val {
                                                IrValue::F32Reg(r) => r,
                                                _ => return Err(LowerError::Unsupported(
                                                    "f32 struct field: initializer did not produce an f32 register".into(),
                                                )),
                                            };
                                            self.instrs.push(Instr::StoreF32 { src: freg, slot: dst_slot });
                                            self.slot_float_ty.insert(dst_slot, IrTy::F32);
                                        }
                                        _ => {
                                            let val = self.lower_expr(&field_init.1, &IrTy::I32)?;
                                            let src = self.val_to_reg(val)?;
                                            self.instrs.push(Instr::Store { src, slot: dst_slot });
                                        }
                                    }
                                }
                            } else if let Some(base_first_slot) = base_struct_slot {
                                // FLS §6.11: Copy unspecified field from the base struct.
                                // Load from `base_first_slot + field_offset`, store to `dst_slot`.
                                // FLS §4.2: If the source slot is a float, propagate the type.
                                // Cache-line note: load+store = two 4-byte instructions = 8 bytes.
                                let src_slot = base_first_slot + field_offset as u8;
                                let fty = self.field_float_ty(struct_name, field_idx);
                                match fty {
                                    Some(IrTy::F64) => {
                                        let tmp = self.alloc_reg()?;
                                        self.instrs.push(Instr::LoadF64Slot { dst: tmp, slot: src_slot });
                                        self.instrs.push(Instr::StoreF64 { src: tmp, slot: dst_slot });
                                        self.slot_float_ty.insert(dst_slot, IrTy::F64);
                                    }
                                    Some(IrTy::F32) => {
                                        let tmp = self.alloc_reg()?;
                                        self.instrs.push(Instr::LoadF32Slot { dst: tmp, slot: src_slot });
                                        self.instrs.push(Instr::StoreF32 { src: tmp, slot: dst_slot });
                                        self.slot_float_ty.insert(dst_slot, IrTy::F32);
                                    }
                                    _ => {
                                        let tmp = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: tmp, slot: src_slot });
                                        self.instrs.push(Instr::Store { src: tmp, slot: dst_slot });
                                    }
                                }
                            } else {
                                return Err(LowerError::Unsupported(format!(
                                    "missing field `{field_name}` in `{struct_name}` literal"
                                )));
                            }
                        }
                        return Ok(());
                }

                // FLS §15.3 + §6.11: Named-field enum variant construction.
                //
                // `let x = Color::Rgb { r: 255, g: 0, b: 0 }` — an `EnumVariantLit`
                // expression. Layout: slot 0 = discriminant, slots 1..N = fields in
                // declaration order (matching `EnumVariantInfo.1`).
                //
                // Fields in the literal may appear in any source order; they are
                // stored in declaration order for stable slot indexing by patterns.
                //
                // FLS §6.1.2:37–45: All stores are runtime instructions.
                // Cache-line note: N+1 consecutive slots; same layout as tuple variants.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::EnumVariantLit { path, fields: lit_fields } = &init_expr.kind
                    && path.len() == 2
                {
                    let enum_name = path[0].text(self.source);
                    let variant_name = path[1].text(self.source);
                    if let Some((discriminant, field_names)) = self.enum_defs
                        .get(enum_name)
                        .and_then(|v| v.get(variant_name))
                        .cloned()
                    {
                        let field_count = field_names.len();
                        if lit_fields.len() != field_count {
                            return Err(LowerError::Unsupported(format!(
                                "enum variant `{enum_name}::{variant_name}` expects {field_count} named fields, got {}",
                                lit_fields.len()
                            )));
                        }
                        // Allocate discriminant slot + one slot per field.
                        let base_slot = self.alloc_slot()?;
                        for _ in 0..field_count {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_enum_types.insert(base_slot, enum_name.to_owned());
                        // Store discriminant.
                        let disc_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                        self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                        // Store each field in declaration order.
                        // FLS §6.11: Fields evaluated in source order but stored in
                        // declaration order for layout stability.
                        // FLS §4.2, §15: Float fields use StoreF64/StoreF32;
                        // integer/bool fields use the regular Store instruction.
                        for (field_idx, field_name) in field_names.iter().enumerate() {
                            let slot = base_slot + 1 + field_idx as u8;
                            let field_init = lit_fields
                                .iter()
                                .find(|(f, _)| f.text(self.source) == field_name.as_str())
                                .ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "missing field `{field_name}` in `{enum_name}::{variant_name}` literal"
                                    ))
                                })?;
                            match self.enum_variant_field_float_ty(enum_name, variant_name, field_idx) {
                                Some(IrTy::F64) => {
                                    let val = self.lower_expr(&field_init.1, &IrTy::F64)?;
                                    let src = match val {
                                        IrValue::FReg(r) => r,
                                        _ => return Err(LowerError::Unsupported(
                                            "f64 named variant field did not produce a float value".into(),
                                        )),
                                    };
                                    self.instrs.push(Instr::StoreF64 { src, slot });
                                    self.slot_float_ty.insert(slot, IrTy::F64);
                                }
                                Some(IrTy::F32) => {
                                    let val = self.lower_expr(&field_init.1, &IrTy::F32)?;
                                    let src = match val {
                                        IrValue::F32Reg(r) => r,
                                        _ => return Err(LowerError::Unsupported(
                                            "f32 named variant field did not produce a float value".into(),
                                        )),
                                    };
                                    self.instrs.push(Instr::StoreF32 { src, slot });
                                    self.slot_float_ty.insert(slot, IrTy::F32);
                                }
                                _ => {
                                    let val = self.lower_expr(&field_init.1, &IrTy::I32)?;
                                    let src = self.val_to_reg(val)?;
                                    self.instrs.push(Instr::Store { src, slot });
                                }
                            }
                        }
                        return Ok(());
                    }
                }

                // FLS §15: Enum tuple variant construction — `let x = Enum::Variant(f0, f1, ...)`.
                //
                // A two-segment path callee in a call expression is an enum tuple variant
                // constructor. Layout: slot 0 = discriminant, slots 1..N = fields.
                // The variable's slot maps to the discriminant slot; `local_enum_types`
                // records the enum type so TupleStruct patterns can locate field slots.
                //
                // FLS §6.1.2:37–45: Construction stores at runtime; no const folding.
                //
                // Cache-line note: a variant with N fields occupies N+1 consecutive slots.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Call { callee, args } = &init_expr.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 2
                {
                    let enum_name = segs[0].text(self.source);
                    let variant_name = segs[1].text(self.source);
                    if let Some((discriminant, field_names)) = self.enum_defs
                        .get(enum_name)
                        .and_then(|v| v.get(variant_name))
                        .cloned()
                    {
                        let field_count = field_names.len();
                        if args.len() != field_count {
                            return Err(LowerError::Unsupported(format!(
                                "enum variant `{enum_name}::{variant_name}` expects {field_count} fields, got {}",
                                args.len()
                            )));
                        }
                        // Allocate discriminant slot + one slot per field.
                        let base_slot = self.alloc_slot()?;
                        for _ in 0..field_count {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_enum_types
                            .insert(base_slot, enum_name.to_owned());
                        // Store discriminant.
                        let disc_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                        self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                        // Store each field in source order.
                        // FLS §4.2, §15: Float fields use StoreF64/StoreF32;
                        // integer/bool fields use the regular Store instruction.
                        for (i, arg) in args.iter().enumerate() {
                            let slot = base_slot + 1 + i as u8;
                            match self.enum_variant_field_float_ty(enum_name, variant_name, i) {
                                Some(IrTy::F64) => {
                                    let val = self.lower_expr(arg, &IrTy::F64)?;
                                    let src = match val {
                                        IrValue::FReg(r) => r,
                                        _ => return Err(LowerError::Unsupported(
                                            "f64 enum variant field did not produce a float value".into(),
                                        )),
                                    };
                                    self.instrs.push(Instr::StoreF64 { src, slot });
                                    self.slot_float_ty.insert(slot, IrTy::F64);
                                }
                                Some(IrTy::F32) => {
                                    let val = self.lower_expr(arg, &IrTy::F32)?;
                                    let src = match val {
                                        IrValue::F32Reg(r) => r,
                                        _ => return Err(LowerError::Unsupported(
                                            "f32 enum variant field did not produce a float value".into(),
                                        )),
                                    };
                                    self.instrs.push(Instr::StoreF32 { src, slot });
                                    self.slot_float_ty.insert(slot, IrTy::F32);
                                }
                                _ => {
                                    let val = self.lower_expr(arg, &IrTy::I32)?;
                                    let src = self.val_to_reg(val)?;
                                    self.instrs.push(Instr::Store { src, slot });
                                }
                            }
                        }
                        return Ok(());
                    }
                }

                // FLS §15: Enum unit variant let binding — `let x = Opt::None;`
                //
                // Unit variants are path expressions (not call expressions), so they
                // are not caught by the call-based enum check above. Detect them here
                // and register the variable in `local_enum_types` so TupleStruct
                // patterns can locate the discriminant slot during match lowering.
                //
                // Allocate enough field slots to accommodate the enum's max field
                // count so that `base_slot + 1 + fi` is always a valid slot index
                // even for unit variant values (fields are uninitialized but never
                // accessed due to discriminant check in the pattern).
                //
                // FLS §6.1.2:37–45: Discriminant store is a runtime instruction.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Path(segs) = &init_expr.kind
                    && segs.len() == 2
                {
                    let enum_name = segs[0].text(self.source);
                    let variant_name = segs[1].text(self.source);
                    if let Some((discriminant, field_names)) = self.enum_defs
                        .get(enum_name)
                        .and_then(|v| v.get(variant_name))
                        .cloned()
                        .filter(|(_, names)| names.is_empty())
                    {
                        // Compute max field count across all variants of this enum.
                        let max_fields = self.enum_defs
                            .get(enum_name)
                            .map(|v| v.values().map(|(_, names)| names.len()).max().unwrap_or(0))
                            .unwrap_or(0);
                        let _ = field_names; // unit variant has no fields
                        let base_slot = self.alloc_slot()?;
                        for _ in 0..max_fields {
                            self.alloc_slot()?; // Reserve field slots (uninitialized for unit variant).
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_enum_types.insert(base_slot, enum_name.to_owned());
                        let disc_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                        self.instrs.push(Instr::Store { src: disc_reg, slot: base_slot });
                        return Ok(());
                    }
                }

                // FLS §6.8: Array literal — `let a = [e0, e1, e2];`
                //
                // An array of N elements is laid out as N consecutive 8-byte
                // stack slots. The variable name maps to the base slot (slot of
                // element 0). Elements are evaluated and stored left-to-right
                // (FLS §6.4:14).
                //
                // `local_array_lens` records the element count so that index
                // expressions can validate the base slot maps to an array.
                //
                // FLS §6.1.2:37–45: All stores are runtime instructions.
                // Cache-line note: N elements × 4-byte stores. An 8-element
                // array initializer emits 8 store instructions = 32 bytes,
                // filling half of a 64-byte cache line.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Array(elems) = &init_expr.kind
                {
                    // FLS §6.8: 2D array literal `let grid = [[r0c0, r0c1], [r1c0, r1c1]]`.
                    //
                    // Detected when every element of the outer array is itself an array
                    // literal. The result is a flattened contiguous allocation of N×M slots,
                    // stored row-major (row 0 first, then row 1, etc.).
                    //
                    // FLS §6.8: Elements are evaluated left-to-right within each row and
                    // rows are evaluated top-to-bottom (FLS §6.4:14).
                    //
                    // Cache-line note: a 4×4 i32 grid (16 slots × 8 bytes) spans two
                    // 64-byte cache lines.
                    if !elems.is_empty()
                        && elems.iter().all(|e| matches!(&e.kind, ExprKind::Array(_)))
                    {
                        let outer_n = elems.len();
                        let inner_n = match &elems[0].kind {
                            ExprKind::Array(row) => row.len(),
                            _ => unreachable!(),
                        };
                        let total = outer_n * inner_n;
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..total {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_array_lens.insert(base_slot, outer_n);
                        self.local_array_inner_lens.insert(base_slot, inner_n);
                        for (i, row_expr) in elems.iter().enumerate() {
                            let ExprKind::Array(cols) = &row_expr.kind else { unreachable!() };
                            for (j, elem_expr) in cols.iter().enumerate() {
                                let val = self.lower_expr(elem_expr, &IrTy::I32)?;
                                let src = self.val_to_reg(val)?;
                                let slot = base_slot + (i * inner_n + j) as u8;
                                self.instrs.push(Instr::Store { src, slot });
                            }
                        }
                        return Ok(());
                    }

                    let n = elems.len();
                    // Allocate N consecutive slots.
                    let base_slot = self.alloc_slot()?;
                    for _ in 1..n {
                        self.alloc_slot()?;
                    }

                    // FLS §4.5, §4.2: Detect f64/f32 element type from the let
                    // annotation (`[f64; N]` / `[f32; N]`) or the first element.
                    // Type annotation takes priority over element inference.
                    let is_f64_arr = ty.as_ref().is_some_and(|t| {
                        matches!(&t.kind, crate::ast::TyKind::Array { elem, .. }
                            if matches!(&elem.kind, crate::ast::TyKind::Path(segs)
                                if segs.len() == 1 && segs[0].text(self.source) == "f64"))
                    }) || (!elems.is_empty() && self.is_f64_expr(&elems[0]));
                    let is_f32_arr = !is_f64_arr && (ty.as_ref().is_some_and(|t| {
                        matches!(&t.kind, crate::ast::TyKind::Array { elem, .. }
                            if matches!(&elem.kind, crate::ast::TyKind::Path(segs)
                                if segs.len() == 1 && segs[0].text(self.source) == "f32"))
                    }) || (!elems.is_empty() && self.is_f32_expr(&elems[0])));

                    self.locals.insert(var_name, base_slot);
                    self.local_array_lens.insert(base_slot, n);

                    if is_f64_arr {
                        // FLS §4.5: `[f64; N]` — store each element as an f64.
                        // FLS §4.2: d-registers hold IEEE 754 double-precision values.
                        // FLS §6.1.2:37–45: All stores are runtime instructions.
                        self.local_f64_array_slots.insert(base_slot);
                        for (i, elem_expr) in elems.iter().enumerate() {
                            let val = self.lower_expr(elem_expr, &IrTy::F64)?;
                            let src = match val {
                                IrValue::FReg(r) => r,
                                _ => return Err(LowerError::Unsupported(
                                    "f64 array element did not produce a float value".into(),
                                )),
                            };
                            self.instrs.push(Instr::StoreF64 { src, slot: base_slot + i as u8 });
                        }
                    } else if is_f32_arr {
                        // FLS §4.5: `[f32; N]` — store each element as an f32.
                        // FLS §4.2: s-registers hold IEEE 754 single-precision values.
                        self.local_f32_array_slots.insert(base_slot);
                        for (i, elem_expr) in elems.iter().enumerate() {
                            let val = self.lower_expr(elem_expr, &IrTy::F32)?;
                            let src = match val {
                                IrValue::F32Reg(r) => r,
                                _ => return Err(LowerError::Unsupported(
                                    "f32 array element did not produce a float value".into(),
                                )),
                            };
                            self.instrs.push(Instr::StoreF32 { src, slot: base_slot + i as u8 });
                        }
                    } else {
                        // Integer/boolean element array (existing path).
                        // FLS §6.8: Elements are evaluated left-to-right.
                        for (i, elem_expr) in elems.iter().enumerate() {
                            let val = self.lower_expr(elem_expr, &IrTy::I32)?;
                            let src = self.val_to_reg(val)?;
                            self.instrs.push(Instr::Store { src, slot: base_slot + i as u8 });
                        }
                    }
                    return Ok(());
                }

                // FLS §6.8: Array repeat expression `let a = [value; N]`.
                //
                // Allocates N consecutive 8-byte stack slots, evaluates
                // `value` once, and stores a copy into each slot.
                //
                // FLS §6.8: "The type of an array expression is [T; N] where
                // T is the type of the element expression." N must be a
                // const expression (FLS §6.1.2:37–45). At this milestone
                // galvanic accepts only integer literal and const-item counts
                // (the common case in practice).
                //
                // FLS §6.1.2:37–45: All stores are runtime instructions.
                //
                // Cache-line note: N elements × 8-byte stack slots. An
                // 8-element repeat array exactly fills one 64-byte cache line.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::ArrayRepeat { value: val_expr, count: cnt_expr } =
                        &init_expr.kind
                {
                    // Resolve the count to a compile-time integer.
                    // FLS §6.8: The repetition operand must be a constant expression.
                    let n: usize = match &cnt_expr.kind {
                        ExprKind::LitInt(v) => {
                            usize::try_from(*v).map_err(|_| {
                                LowerError::Unsupported(format!(
                                    "array repeat count `{v}` is too large"
                                ))
                            })?
                        }
                        ExprKind::Path(segs) if segs.len() == 1 => {
                            // FLS §7.1: Allow named const items as the count.
                            let name = segs[0].text(self.source);
                            match self.const_vals.get(name) {
                                Some(&v) if v >= 0 => v as usize,
                                Some(_) => {
                                    return Err(LowerError::Unsupported(format!(
                                        "array repeat count const `{name}` is negative"
                                    )))
                                }
                                None => {
                                    return Err(LowerError::Unsupported(format!(
                                        "array repeat count must be a const expression; `{name}` not found"
                                    )))
                                }
                            }
                        }
                        _ => {
                            return Err(LowerError::Unsupported(
                                "array repeat count must be an integer literal or const item".into(),
                            ))
                        }
                    };

                    // Allocate N consecutive slots.
                    let base_slot = self.alloc_slot()?;
                    for _ in 1..n {
                        self.alloc_slot()?;
                    }
                    self.locals.insert(var_name, base_slot);
                    self.local_array_lens.insert(base_slot, n);

                    // FLS §4.5, §4.2: Detect f64/f32 fill value type.
                    // Check type annotation first, then fall back to expression type.
                    let is_f64_rep = ty.as_ref().is_some_and(|t| {
                        matches!(&t.kind, crate::ast::TyKind::Array { elem, .. }
                            if matches!(&elem.kind, crate::ast::TyKind::Path(segs)
                                if segs.len() == 1 && segs[0].text(self.source) == "f64"))
                    }) || self.is_f64_expr(val_expr);
                    let is_f32_rep = !is_f64_rep && (ty.as_ref().is_some_and(|t| {
                        matches!(&t.kind, crate::ast::TyKind::Array { elem, .. }
                            if matches!(&elem.kind, crate::ast::TyKind::Path(segs)
                                if segs.len() == 1 && segs[0].text(self.source) == "f32"))
                    }) || self.is_f32_expr(val_expr));

                    // Evaluate the fill value once.
                    // FLS §6.8: The element expression is evaluated exactly once
                    // and then copied N times (for Copy types). Here we lower
                    // the expression once and store it into each slot.
                    //
                    // FLS §6.1.2:37–45: All stores are runtime instructions.
                    if is_f64_rep {
                        // FLS §4.5: `[f64; N]` repeat — store each slot as f64.
                        self.local_f64_array_slots.insert(base_slot);
                        let val = self.lower_expr(val_expr, &IrTy::F64)?;
                        let src = match val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f64 array repeat fill did not produce a float value".into(),
                            )),
                        };
                        for i in 0..n {
                            self.instrs.push(Instr::StoreF64 { src, slot: base_slot + i as u8 });
                        }
                    } else if is_f32_rep {
                        // FLS §4.5: `[f32; N]` repeat — store each slot as f32.
                        self.local_f32_array_slots.insert(base_slot);
                        let val = self.lower_expr(val_expr, &IrTy::F32)?;
                        let src = match val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f32 array repeat fill did not produce a float value".into(),
                            )),
                        };
                        for i in 0..n {
                            self.instrs.push(Instr::StoreF32 { src, slot: base_slot + i as u8 });
                        }
                    } else {
                        let val = self.lower_expr(val_expr, &IrTy::I32)?;
                        let src = self.val_to_reg(val)?;
                        for i in 0..n {
                            self.instrs.push(Instr::Store { src, slot: base_slot + i as u8 });
                        }
                    }
                    return Ok(());
                }

                // FLS §6.10: Tuple literal `let t = (e0, e1, ...)`.
                //
                // All scalar leaves of the tuple are stored in consecutive
                // 8-byte stack slots. For flat `(1, 2, 3)` this is 3 slots;
                // for nested `(1, (2, 3))` this is also 3 slots (leaves
                // flattened left-to-right). The variable name maps to the
                // base slot (slot of the first leaf).
                //
                // `local_tuple_lens` records the **total leaf count** so that
                // the call-site argument expansion (passing a tuple variable
                // to a function expecting a tuple/nested-tuple parameter) loads
                // the correct number of slots.
                //
                // FLS §5.10.3, §9.2: A nested tuple parameter receives one
                // register per scalar leaf, so the storage layout must match.
                // FLS §6.4:14: Elements evaluated and stored left-to-right.
                // FLS §6.1.2:37–45: All stores are runtime instructions.
                //
                // Cache-line note: N leaf stores = N × 4-byte `str` instructions.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Tuple(elems) = &init_expr.kind
                {
                    let n_leaves = Self::count_tuple_leaves(elems);
                    let base_slot = self.alloc_slot()?;
                    for _ in 1..n_leaves {
                        self.alloc_slot()?;
                    }
                    self.locals.insert(var_name, base_slot);
                    self.local_tuple_lens.insert(base_slot, n_leaves);
                    // FLS §6.10, §5.10.3: Store all leaves left-to-right.
                    let mut offset: u8 = 0;
                    self.store_tuple_leaves(elems, base_slot, &mut offset)?;
                    return Ok(());
                }

                // FLS §14.2: Tuple struct constructor `let p = Point(a, b)`.
                //
                // A tuple struct constructor is syntactically a call expression
                // with a single-segment callee path that names a known tuple struct.
                // Lower it like a tuple literal: allocate N consecutive stack slots
                // and store each argument value in order.
                //
                // The base slot is registered in `local_tuple_lens` so that
                // subsequent `.0`, `.1` field access expressions (FLS §6.10) resolve
                // to `base_slot + index`, reusing the existing tuple field access path.
                //
                // FLS §6.1.2:37–45: All stores are runtime instructions — no const folding.
                // Cache-line note: N stores per construction, identical to a tuple literal.
                if let Some(init_expr) = init.as_ref()
                    && let ExprKind::Call { callee, args } = &init_expr.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 1
                {
                    let ctor_name = segs[0].text(self.source);
                    if let Some(&n) = self.tuple_struct_defs.get(ctor_name) {
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..n {
                            self.alloc_slot()?;
                        }
                        self.locals.insert(var_name, base_slot);
                        self.local_tuple_lens.insert(base_slot, n);
                        // FLS §14.2: Record the tuple struct type so method call dispatch
                        // can compute the mangled name `TypeName__method_name`.
                        self.local_tuple_struct_types.insert(base_slot, ctor_name.to_owned());
                        // FLS §4.2, §14.2: Look up per-field float types so f64/f32
                        // fields use float registers and StoreF64/StoreF32.
                        let float_field_tys = self
                            .tuple_struct_float_field_types
                            .get(ctor_name)
                            .cloned()
                            .unwrap_or_default();
                        // FLS §6.4:14 / §6.10: Arguments evaluated left-to-right.
                        for (i, arg_expr) in args.iter().enumerate() {
                            let slot = base_slot + i as u8;
                            match float_field_tys.get(i).copied().flatten() {
                                Some(IrTy::F64) => {
                                    let val = self.lower_expr(arg_expr, &IrTy::F64)?;
                                    let src = match val {
                                        IrValue::FReg(r) => r,
                                        _ => return Err(LowerError::Unsupported(
                                            "f64 tuple struct field did not produce a float value".into(),
                                        )),
                                    };
                                    self.instrs.push(Instr::StoreF64 { src, slot });
                                    self.slot_float_ty.insert(slot, IrTy::F64);
                                }
                                Some(IrTy::F32) => {
                                    let val = self.lower_expr(arg_expr, &IrTy::F32)?;
                                    let src = match val {
                                        IrValue::F32Reg(r) => r,
                                        _ => return Err(LowerError::Unsupported(
                                            "f32 tuple struct field did not produce a float value".into(),
                                        )),
                                    };
                                    self.instrs.push(Instr::StoreF32 { src, slot });
                                    self.slot_float_ty.insert(slot, IrTy::F32);
                                }
                                _ => {
                                    let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                                    let src = self.val_to_reg(val)?;
                                    self.instrs.push(Instr::Store { src, slot });
                                }
                            }
                        }
                        return Ok(());
                    }
                }

                // Normal (non-struct) let binding.
                //
                // FLS §8.1: The introduced binding comes into scope for the
                // remainder of the block *after* the let statement completes.
                // In particular, the RHS initializer is evaluated in the scope
                // that does NOT yet include the new binding. This is the
                // shadowing rule: `let x = x + 3` means "evaluate the old x,
                // add 3, bind the result to a new x". The old x remains
                // accessible during RHS evaluation.
                //
                // Ordering: alloc slot first (so the slot index is stable),
                // evaluate RHS (still sees the old `var_name` binding if any),
                // then insert the new binding into locals.
                let slot = self.alloc_slot()?;

                // FLS §4.2: If the type annotation is `f64`, use float
                // store/load instructions and register in `float_locals`.
                //
                // Type annotation drives the choice because type inference is
                // not yet implemented (FLS §8.1 AMBIGUOUS on inference rules).
                // `let x: f64 = 3.0` is unambiguous; `let x = 3.0` would need
                // inference and is not yet supported.
                let declared_f64 = ty.as_ref().is_some_and(|t| {
                    matches!(&t.kind, crate::ast::TyKind::Path(segs)
                        if segs.len() == 1 && segs[0].text(self.source) == "f64")
                });

                if declared_f64 {
                    if let Some(init_expr) = init.as_ref() {
                        // FLS §2.4.4.2: f64 initializer must produce a float reg.
                        // FLS §6.1.2:37–45: store is a runtime instruction.
                        let val = self.lower_expr(init_expr, &IrTy::F64)?;
                        let src = match val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f64 let binding: initializer did not produce a float value".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF64 { src, slot });
                    }
                    // Register as a float local (not in `locals`) so path
                    // expressions emit `LoadF64Slot` rather than `Load`.
                    self.float_locals.insert(var_name, slot);
                    return Ok(());
                }

                // FLS §4.2: If the type annotation is `f32`, use single-precision
                // float store/load instructions and register in `float32_locals`.
                let declared_f32 = ty.as_ref().is_some_and(|t| {
                    matches!(&t.kind, crate::ast::TyKind::Path(segs)
                        if segs.len() == 1 && segs[0].text(self.source) == "f32")
                });

                if declared_f32 {
                    if let Some(init_expr) = init.as_ref() {
                        // FLS §2.4.4.2: f32 initializer must produce an f32 reg.
                        // FLS §6.1.2:37–45: store is a runtime instruction.
                        let val = self.lower_expr(init_expr, &IrTy::F32)?;
                        let src = match val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f32 let binding: initializer did not produce an f32 register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF32 { src, slot });
                    }
                    // Register as an f32 local so path expressions emit
                    // `LoadF32Slot` rather than `Load` or `LoadF64Slot`.
                    self.float32_locals.insert(var_name, slot);
                    return Ok(());
                }

                if let Some(init_expr) = init.as_ref() {
                    // FLS §8.1 AMBIGUOUS: the spec does not describe how type
                    // inference resolves the type of the initializer in the
                    // absence of a type annotation. We default to i32 for
                    // integer-producing expressions.
                    //
                    // FLS §4.2: If the initializer is detectably a float expression
                    // (via is_f64_expr/is_f32_expr heuristic — covers float literals,
                    // float variables, float tuple fields, and float arithmetic) lower
                    // it in the float path even without an explicit type annotation.
                    // This supports `let x = t.0` when t's first element is f64.
                    if self.is_f64_expr(init_expr) {
                        let val = self.lower_expr(init_expr, &IrTy::F64)?;
                        let src = match val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "inferred f64 let binding: initializer did not produce a float register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF64 { src, slot });
                        self.float_locals.insert(var_name, slot);
                        return Ok(());
                    }
                    if self.is_f32_expr(init_expr) {
                        let val = self.lower_expr(init_expr, &IrTy::F32)?;
                        let src = match val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "inferred f32 let binding: initializer did not produce an f32 register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF32 { src, slot });
                        self.float32_locals.insert(var_name, slot);
                        return Ok(());
                    }
                    let val = self.lower_expr(init_expr, &IrTy::I32)?;
                    let src = self.val_to_reg(val)?;
                    self.instrs.push(Instr::Store { src, slot });
                    // FLS §4.9: If the initializer is a path naming a known
                    // function, this local holds a function pointer. Track the
                    // slot so that `f(args)` emits `CallIndirect` (blr) rather
                    // than a direct `bl f` (which would be an undefined symbol).
                    if let ExprKind::Path(segs) = &init_expr.kind
                        && segs.len() == 1
                        && self.fn_names.contains(segs[0].text(self.source))
                    {
                        self.local_fn_ptr_slots.insert(slot);
                    }
                    // FLS §6.14, §6.22: Closure expressions stored in a let binding —
                    // mark the slot as a function pointer so `f(args)` emits
                    // `CallIndirect` (blr) rather than a direct `bl f`.
                    // If the closure captured outer variables, also register the
                    // capture-slot list so the call site passes them as leading args.
                    if matches!(init_expr.kind, ExprKind::Closure { .. }) {
                        self.local_fn_ptr_slots.insert(slot);
                        if let Some(cap_slots) = self.last_closure_captures.take() {
                            self.local_capture_args.insert(slot, cap_slots);
                        }
                        // Clear trampoline side-channels (consumed by let, not by call-arg path).
                        self.last_closure_name = None;
                        self.last_closure_n_explicit = None;
                    }
                    // FLS §2.4.6: String literal stored in a let binding —
                    // mark the slot so that `.len()` can load the byte-length
                    // value without dispatching to the struct/enum method table.
                    if matches!(init_expr.kind, ExprKind::LitStr) {
                        self.local_str_slots.insert(slot);
                    }
                }
                // Bring the new binding into scope only after the initializer
                // has been fully evaluated. FLS §8.1: uninitialized let
                // bindings (no init_expr) also become visible here.
                self.locals.insert(var_name, slot);

                Ok(())
            }

            // FLS §8.2: Expression statement — evaluate for side effects, discard value.
            //
            // Assignment and call expressions are the primary expression statements
            // at this milestone. `lower_expr` is called with `IrTy::Unit` as the
            // type hint; assignment and call handlers ignore `ret_ty`, so this is
            // safe. Unsupported expression kinds will propagate their own errors.
            //
            // FLS §6.1.2:37–45: The expression executes at runtime; its result
            // (if any) is discarded.
            StmtKind::Expr(expr) => {
                self.lower_expr(expr, &IrTy::Unit)?;
                Ok(())
            }

            // FLS §8.2: Empty statements are no-ops.
            StmtKind::Empty => Ok(()),

            // FLS §3, §9: Inner function item defined inside a block body.
            //
            // An inner function is compiled as a sibling top-level function —
            // identical to a closure that captures nothing. It does not capture
            // outer locals (unlike closures). Its name is registered in
            // `self.fn_names` (which was already done in the pre-pass in
            // `lower_block_to_value`) so that call sites emit a direct `bl`.
            //
            // FLS §6.14 AMBIGUOUS: The spec does not distinguish inner functions
            // from closures in terms of name visibility; galvanic treats inner
            // function names as direct-call targets (no `blr` indirection).
            //
            // Cache-line note: the generated label is 8 bytes aligned by the
            // assembler; function bodies are naturally cache-line aligned for
            // the ARM64 I-cache (64-byte lines, 4-byte instructions).
            StmtKind::Item(item) => {
                let ItemKind::Fn(fn_def) = &item.kind else {
                    // Non-fn inner items (structs, enums, etc.) are parsed but
                    // not yet lowered. Skip silently at this milestone.
                    return Ok(());
                };

                let fn_name = fn_def.name.text(self.source).to_owned();

                // Determine return type.
                // FLS §9: Scalar return types only at this milestone.
                let ret_ty = match &fn_def.ret_ty {
                    None => IrTy::Unit,
                    Some(ty) => lower_ty(ty, self.source, self.type_aliases)
                        .map_err(|_| LowerError::Unsupported(format!(
                            "inner function `{fn_name}` has unsupported return type"
                        )))?,
                };

                let body = match &fn_def.body {
                    None => return Err(LowerError::Unsupported(
                        format!("inner function `{fn_name}` has no body")
                    )),
                    Some(b) => b,
                };

                // Build a new LowerCtx for the inner function body.
                // Labels continue from where the enclosing function left off so
                // all labels in the module's assembly output remain unique.
                let inner_start_label = self.next_label;
                let mut inner_ctx = LowerCtx::new(
                    self.source,
                    &fn_name,
                    ret_ty,
                    self.struct_defs,
                    self.tuple_struct_defs,
                    self.tuple_struct_float_field_types,
                    self.enum_defs,
                    self.enum_variant_float_field_types,
                    self.method_self_kinds,
                    self.mut_self_scalar_return_fns,
                    self.struct_return_fns,
                    self.struct_return_free_fns,
                    self.enum_return_fns,
                    self.struct_return_methods,
                    self.tuple_return_free_fns,
                    self.f64_return_fns,
                    self.f32_return_fns,
                    self.const_vals,
                    self.const_f64_vals,
                    self.const_f32_vals,
                    self.static_names,
                    self.static_f64_names,
                    self.static_f32_names,
                    &self.fn_names,
                    self.struct_field_types,
                    self.struct_field_offsets,
                    self.struct_sizes,
                    self.type_aliases,
                    self.struct_float_field_types,
                    inner_start_label,
                );

                // Spill parameters from ARM64 registers.
                // FLS §9: Parameters arrive in x0..x{n-1}.
                // Only simple ident parameters with scalar types supported.
                for (i, param) in fn_def.params.iter().enumerate() {
                    let param_ty = lower_ty(&param.ty, self.source, self.type_aliases)
                        .unwrap_or(IrTy::I32);
                    let slot = inner_ctx.alloc_slot()?;
                    match param_ty {
                        IrTy::F64 => {
                            inner_ctx.instrs.push(Instr::StoreF64 { src: i as u8, slot });
                            inner_ctx.slot_float_ty.insert(slot, IrTy::F64);
                        }
                        IrTy::F32 => {
                            inner_ctx.instrs.push(Instr::StoreF32 { src: i as u8, slot });
                            inner_ctx.slot_float_ty.insert(slot, IrTy::F32);
                        }
                        _ => {
                            inner_ctx.instrs.push(Instr::Store { src: i as u8, slot });
                            if param_ty == IrTy::FnPtr {
                                inner_ctx.local_fn_ptr_slots.insert(slot);
                            }
                        }
                    }
                    if let ParamKind::Ident(name_span) = &param.kind {
                        let name = name_span.text(self.source);
                        if name != "_" {
                            inner_ctx.locals.insert(name, slot);
                        }
                    }
                }

                // Lower the body and append Ret.
                inner_ctx.lower_block(body, &ret_ty)?;

                // Propagate the label counter back to the enclosing function.
                self.next_label = inner_ctx.next_label;

                // Collect the inner function and any closures it defines.
                let inner_fn = IrFn {
                    name: fn_name,
                    ret_ty,
                    body: inner_ctx.instrs,
                    stack_slots: inner_ctx.next_slot,
                    saves_lr: inner_ctx.has_calls,
                    float_consts: inner_ctx.float_consts,
                    float32_consts: inner_ctx.float32_consts,
                };
                self.pending_closures.push(inner_fn);
                self.pending_closures.extend(inner_ctx.pending_closures);

                Ok(())
            }
        }
    }

    // ── Expression lowering ──────────────────────────────────────────────────

    /// Lower an expression to runtime IR instructions.
    ///
    /// Returns the `IrValue` holding the result. Emits `LoadImm`, `BinOp`,
    /// `Load`, `Label`, `Branch`, `CondBranch`, etc. into `self.instrs`.
    ///
    /// `ret_ty` is the expected type of this expression. Used to select which
    /// variant of a literal or operator to emit.
    ///
    /// FLS §6.1.2:37–45: All code here emits runtime instructions.
    fn lower_expr(&mut self, expr: &Expr, ret_ty: &IrTy) -> Result<IrValue, LowerError> {
        match &expr.kind {
            // FLS §2.4.4.1: Integer literal — materialize as a runtime immediate.
            ExprKind::LitInt(n) => {
                match ret_ty {
                    IrTy::I32 => {
                        if *n > i32::MAX as u128 {
                            return Err(LowerError::Unsupported(format!(
                                "integer literal {n} out of range for i32"
                            )));
                        }
                        let n = *n as i32;
                        let r = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(r, n));
                        Ok(IrValue::Reg(r))
                    }
                    // FLS §4.1: Unsigned integer literal. Reuses LoadImm(i32)
                    // so the value must fit in i32 range at this milestone.
                    // Values in (i32::MAX, u32::MAX] require MOVZ+MOVK and are
                    // deferred (FLS §2.4.4.1 AMBIGUOUS: spec does not specify
                    // encoding limits for large unsigned literals).
                    IrTy::U32 => {
                        if *n > i32::MAX as u128 {
                            return Err(LowerError::Unsupported(format!(
                                "unsigned literal {n} > {}: MOVZ+MOVK not yet supported",
                                i32::MAX
                            )));
                        }
                        let n = *n as i32;
                        let r = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(r, n));
                        Ok(IrValue::Reg(r))
                    }
                    _ => Err(LowerError::Unsupported("integer literal with non-integer type".into())),
                }
            }

            // FLS §2.4.7: Boolean literals — `false` = 0, `true` = 1.
            //
            // Booleans are materialized as 0/1 integer immediates. The `CondBranch`
            // instruction tests for zero, matching `false` semantics naturally.
            //
            // FLS §6.1.2:37–45: Even statically-known booleans emit a `mov` at
            // runtime — no constant folding of `if true { ... }` to the then branch.
            ExprKind::LitBool(b) => {
                let r = self.alloc_reg()?;
                self.instrs.push(Instr::LoadImm(r, if *b { 1 } else { 0 }));
                Ok(IrValue::Reg(r))
            }

            // FLS §2.4.5: Character literal — materialize the Unicode scalar value
            // (code point) as a runtime immediate.
            // FLS §2.4.1: Byte literal — materialize the ASCII/byte value as a
            // runtime immediate.
            //
            // A char literal like `'A'` evaluates to code point 65.
            // A byte literal like `b'A'` evaluates to byte value 65 (type `u8`).
            // Both are parsed into `ExprKind::LitChar` by the parser.
            // The span text includes the surrounding quotes (and optional `b` prefix);
            // `parse_char_value` strips them and handles escape sequences.
            //
            // FLS §2.4.5: The type of a char literal is `char`, which galvanic
            // maps to `IrTy::U32` (see `lower_ty`).
            // FLS §2.4.1: The type of a byte literal is `u8`, which galvanic also
            // maps to `IrTy::U32` (8-bit values are zero-extended to 64-bit register).
            //
            // FLS §6.1.2:37–45: Even a literal emits a runtime `mov` — no
            // constant folding across this boundary.
            //
            // Cache-line note: one `mov` instruction = 4 bytes (half a cache slot).
            ExprKind::LitChar => {
                let text = expr.span.text(self.source);
                let code_point = parse_char_value(text)?;
                let r = self.alloc_reg()?;
                // All valid Unicode scalar values fit in i32 (max 0x10FFFF = 1,114,111
                // which is well below i32::MAX = 2,147,483,647).
                // Byte values (0–255) also fit trivially.
                self.instrs.push(Instr::LoadImm(r, code_point as i32));
                Ok(IrValue::Reg(r))
            }

            // FLS §2.4.6: String literal — materialize the UTF-8 byte length as a
            // runtime immediate.
            //
            // At this milestone galvanic materialises the *length* half of the `&str`
            // fat pointer.  The pointer half (a `.rodata` address) is deferred to a
            // future milestone when string indexing or display is needed.
            //
            // FLS §2.4.6: "A string literal is a sequence of Unicode characters …
            // its type is `&str`."
            // FLS §2.4.6.2: Raw string literals (r"..." / r#"..."#) contain no
            // escape sequences — backslash is a literal byte.  `r"hello\n"` is
            // 7 bytes; `"hello\n"` is 6 bytes.
            // FLS §2.4.2: Byte string literals (b"...") have type `&[u8]`.
            // FLS §2.4.2.2: Raw byte string literals (br"..." / br#"..."#) follow
            // the same no-escape rule as raw string literals.
            // FLS §6.1.2:37–45: Even a literal emits a runtime `mov` — no
            // constant folding across this boundary.
            //
            // Cache-line note: one `mov` instruction = 4 bytes (half a cache slot).
            // String literal length fits in i32 (max realistic string is far below
            // 2 GiB, the i32::MAX byte limit).
            ExprKind::LitStr => {
                let text = expr.span.text(self.source);
                let byte_len = parse_str_byte_len(text)?;
                let r = self.alloc_reg()?;
                self.instrs.push(Instr::LoadImm(r, byte_len as i32));
                Ok(IrValue::Reg(r))
            }

            // FLS §2.4.4.2: Float literal — load a 64-bit float constant from
            // the per-function constant pool into a float register.
            //
            // The float value is stored in `.rodata` as its raw IEEE 754 bits
            // and loaded at runtime via ADRP + ADD + LDR into `d{dst}`.
            //
            // FLS §4.2: `f64` is a 64-bit IEEE 754 floating-point type.
            // FLS §6.1.2:37–45: Even a float literal emits runtime instructions;
            // there is no constant folding for non-const code.
            //
            // Cache-line note: ADRP + ADD + LDR = 3 instructions = 12 bytes.
            // The constant itself is one 8-byte `.quad` in `.rodata`.
            ExprKind::LitFloat => {
                let text = expr.span.text(self.source);
                // FLS §2.4.4.2: A literal with `_f32` suffix, or lowered in an
                // f32 context, produces a single-precision value in an `s`-register.
                // All other float literals default to f64 in a `d`-register.
                if text.ends_with("_f32") || ret_ty == &IrTy::F32 {
                    let val = parse_float32_value(text)?;
                    let bits = val.to_bits();
                    let idx = self.float32_consts.len() as u32;
                    self.float32_consts.push(bits);
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadF32Const { dst, idx });
                    Ok(IrValue::F32Reg(dst))
                } else {
                    let val = parse_float_value(text)?;
                    let bits = val.to_bits();
                    let idx = self.float_consts.len() as u32;
                    self.float_consts.push(bits);
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadF64Const { dst, idx });
                    Ok(IrValue::FReg(dst))
                }
            }

            // FLS §4.4: Unit literal `()`.
            ExprKind::Unit => Ok(IrValue::Unit),

            // FLS §6.4: Block expression — lower statements then the tail value.
            //
            // A block expression `{ stmt; ...; tail }` evaluates each statement
            // in order and produces the tail expression's value.
            ExprKind::Block(block) => {
                self.lower_block_to_value(block, ret_ty)
            }

            // FLS §6.3: Path expression — a reference to a local variable or
            // an enum unit variant.
            //
            // A single-segment path is either a const item reference or a
            // local variable reference.
            //
            // FLS §7.1:10: Every use of a constant is replaced with its value.
            // For const items, emit `LoadImm` with the compile-time value.
            //
            // FLS §6.3: A local variable path emits `Load` at runtime.
            // FLS §6.1.2:37–45: The load is a runtime instruction — even if
            // the variable holds a statically-known value, we must load it.
            ExprKind::Path(segments) if segments.len() == 1 => {
                let var_name = segments[0].text(self.source);
                // Check const_vals first: FLS §7.1 substitution takes
                // precedence over any local shadowing (rustc warns on shadow).
                if let Some(&const_val) = self.const_vals.get(var_name) {
                    let r = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadImm(r, const_val));
                    return Ok(IrValue::Reg(r));
                }
                // Check f64 const items: FLS §7.1, §4.2 — float consts are
                // substituted as LoadF64Const (ldr d{N}) rather than LoadImm.
                if let Some(&const_val) = self.const_f64_vals.get(var_name) {
                    let dst = self.alloc_reg()?;
                    let idx = self.float_consts.len() as u32;
                    self.float_consts.push(const_val.to_bits());
                    self.instrs.push(Instr::LoadF64Const { dst, idx });
                    return Ok(IrValue::FReg(dst));
                }
                // Check f32 const items: FLS §7.1, §4.2 — f32 consts are
                // substituted as LoadF32Const (ldr s{N}).
                if let Some(&const_val) = self.const_f32_vals.get(var_name) {
                    let dst = self.alloc_reg()?;
                    let idx = self.float32_consts.len() as u32;
                    self.float32_consts.push(const_val.to_bits());
                    self.instrs.push(Instr::LoadF32Const { dst, idx });
                    return Ok(IrValue::F32Reg(dst));
                }
                // Check static_names: FLS §7.2 — all references to a static
                // go through its memory address (ADRP + ADD + LDR).
                if self.static_names.contains(var_name) {
                    let r = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadStatic { dst: r, name: var_name.to_owned() });
                    return Ok(IrValue::Reg(r));
                }
                // Check f64 static items: FLS §7.2, §4.2 — f64 statics load into
                // float registers via ADRP + ADD + LDR d{dst}.
                if self.static_f64_names.contains(var_name) {
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadStaticF64 { dst, name: var_name.to_owned() });
                    return Ok(IrValue::FReg(dst));
                }
                // Check f32 static items: FLS §7.2, §4.2 — f32 statics load into
                // float registers via ADRP + ADD + LDR s{dst}.
                if self.static_f32_names.contains(var_name) {
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadStaticF32 { dst, name: var_name.to_owned() });
                    return Ok(IrValue::F32Reg(dst));
                }
                // Check fn_names: FLS §4.9 — a path that names a function item
                // (and is not a local variable) materializes the function's address
                // via ADRP + ADD. This is how function pointers are passed as values.
                if !self.locals.contains_key(var_name) && self.fn_names.contains(var_name) {
                    let r = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadFnAddr { dst: r, name: var_name.to_owned() });
                    return Ok(IrValue::Reg(r));
                }
                // Check float32_locals: FLS §4.2 — f32 variables use s-register
                // instructions (LoadF32Slot) rather than integer or d-register loads.
                if let Some(&slot) = self.float32_locals.get(var_name) {
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadF32Slot { dst, slot });
                    return Ok(IrValue::F32Reg(dst));
                }
                // Check float_locals: FLS §4.2 — f64 variables use d-register
                // instructions (LoadF64Slot) rather than integer loads.
                if let Some(&slot) = self.float_locals.get(var_name) {
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadF64Slot { dst, slot });
                    return Ok(IrValue::FReg(dst));
                }
                let slot = self.locals.get(var_name).copied().ok_or_else(|| {
                    LowerError::Unsupported(format!("undefined variable `{var_name}`"))
                })?;
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst, slot });
                Ok(IrValue::Reg(dst))
            }

            // FLS §6.3 + §15: Two-segment path expression — an enum unit variant.
            //
            // `Color::Red` resolves to the integer discriminant of `Red` in
            // the `Color` enum. Emits `LoadImm(discriminant)` at runtime.
            //
            // FLS §6.1.2:37–45: Even though the discriminant is a compile-time
            // constant, we emit a runtime `mov` — consistent with how integer
            // literals are handled in non-const contexts.
            //
            // FLS §15 AMBIGUOUS: The spec does not specify the default discriminant
            // values for unit variants. Galvanic assigns 0, 1, 2, ... in declaration
            // order, which matches rustc's default behavior.
            ExprKind::Path(segments) if segments.len() == 2 => {
                let enum_name = segments[0].text(self.source);
                let variant_name = segments[1].text(self.source);
                let discriminant = self.enum_defs
                    .get(enum_name)
                    .and_then(|variants| variants.get(variant_name))
                    .map(|(disc, _)| *disc)
                    .ok_or_else(|| {
                        LowerError::Unsupported(format!(
                            "unknown path `{enum_name}::{variant_name}` (not an enum variant)"
                        ))
                    })?;
                let r = self.alloc_reg()?;
                self.instrs.push(Instr::LoadImm(r, discriminant));
                Ok(IrValue::Reg(r))
            }

            // FLS §6.5.5: Arithmetic binary operations — emit runtime instructions.
            // FLS §6.5.6: Bit operator expressions (`&`, `|`, `^`).
            // FLS §6.5.7: Shift operator expressions (`<<`, `>>`).
            //
            // Both operands are lowered recursively, producing LoadImm/BinOp
            // instructions. The result is in a virtual register.
            //
            // Division and remainder are included here. FLS §6.23: division by
            // zero panics at runtime; galvanic does not yet insert a check
            // (FLS §6.23 AMBIGUOUS: the panic mechanism is not yet specified).
            //
            // FLS §6.5.6: "The type of a bit expression is the type of the left
            // operand." Both operands must have the same integer type.
            //
            // FLS §6.5.7: For signed integer types, `>>` is arithmetic shift
            // (sign-extending). The shift amount is taken modulo the bit width.
            // FLS §6.5.7 AMBIGUOUS: the spec says the shift amount is taken modulo
            // the bit width, but does not specify the exact register width used for
            // the modulo (ARM64 uses 6 bits of the shift register for 64-bit shifts).
            //
            // FLS §6.1.2:37–45: All these operators emit runtime instructions —
            // no constant folding even when both operands are literals.
            ExprKind::Binary { op, lhs, rhs }
                if matches!(
                    op,
                    BinOp::Add
                        | BinOp::Sub
                        | BinOp::Mul
                        | BinOp::Div
                        | BinOp::Rem
                        | BinOp::BitAnd
                        | BinOp::BitOr
                        | BinOp::BitXor
                        | BinOp::Shl
                        | BinOp::Shr
                ) =>
            {
                match ret_ty {
                    IrTy::I32 => {
                        let lhs_val = self.lower_expr(lhs, ret_ty)?;

                        // Spill lhs to a stack slot if rhs contains a call.
                        //
                        // ARM64 calling convention: x0–x17 are caller-saved.
                        // A `bl` instruction in the rhs lowers to a call that
                        // clobbers every register in that range, including the
                        // register that holds the lhs result. Without a spill,
                        // `fib(n-1) + fib(n-2)` would compute the wrong answer
                        // because fib(n-2)'s call sequence overwrites the
                        // register that holds fib(n-1)'s result.
                        //
                        // Only spill when lhs is already in a register (not an
                        // unresolved immediate like `IrValue::I32`). Immediates
                        // are not in any register yet so they are unaffected by
                        // the call.
                        //
                        // FLS §6.12.1: Call expressions follow ARM64 AAPCS64
                        // calling convention (caller-saved: x0–x17).
                        let lhs_spill: Option<u8> = if let IrValue::Reg(r) = lhs_val {
                            if expr_contains_call(rhs) {
                                let slot = self.alloc_slot()?;
                                self.instrs.push(Instr::Store { src: r, slot });
                                Some(slot)
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let rhs_val = self.lower_expr(rhs, ret_ty)?;

                        // Reload lhs from its spill slot, if we spilled it.
                        let lhs_val = if let Some(slot) = lhs_spill {
                            let dst = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst, slot });
                            IrValue::Reg(dst)
                        } else {
                            lhs_val
                        };

                        let lhs_reg = self.val_to_reg(lhs_val)?;
                        let rhs_reg = self.val_to_reg(rhs_val)?;

                        let dst = self.alloc_reg()?;
                        let ir_op = match op {
                            BinOp::Add => IrBinOp::Add,
                            BinOp::Sub => IrBinOp::Sub,
                            BinOp::Mul => IrBinOp::Mul,
                            BinOp::Div => IrBinOp::Div,
                            BinOp::Rem => IrBinOp::Rem,
                            BinOp::BitAnd => IrBinOp::BitAnd,
                            BinOp::BitOr => IrBinOp::BitOr,
                            BinOp::BitXor => IrBinOp::BitXor,
                            BinOp::Shl => IrBinOp::Shl,
                            BinOp::Shr => IrBinOp::Shr,
                            _ => unreachable!("matched above"),
                        };
                        self.instrs.push(Instr::BinOp { op: ir_op, dst, lhs: lhs_reg, rhs: rhs_reg });
                        Ok(IrValue::Reg(dst))
                    }
                    // FLS §4.1: Unsigned integer arithmetic. Same spill/reload
                    // logic as signed, but division uses `udiv` (IrBinOp::UDiv)
                    // and right shift uses `lsr` (IrBinOp::UShr) for correct
                    // unsigned semantics. Add/sub/mul/bitwise are identical to
                    // signed at the hardware level on ARM64.
                    IrTy::U32 => {
                        let lhs_val = self.lower_expr(lhs, ret_ty)?;

                        let lhs_spill: Option<u8> = if let IrValue::Reg(r) = lhs_val {
                            if expr_contains_call(rhs) {
                                let slot = self.alloc_slot()?;
                                self.instrs.push(Instr::Store { src: r, slot });
                                Some(slot)
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let rhs_val = self.lower_expr(rhs, ret_ty)?;

                        let lhs_val = if let Some(slot) = lhs_spill {
                            let dst = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst, slot });
                            IrValue::Reg(dst)
                        } else {
                            lhs_val
                        };

                        let lhs_reg = self.val_to_reg(lhs_val)?;
                        let rhs_reg = self.val_to_reg(rhs_val)?;

                        let dst = self.alloc_reg()?;
                        // FLS §4.1: unsigned uses udiv and lsr; all others identical to signed.
                        let ir_op = match op {
                            BinOp::Add => IrBinOp::Add,
                            BinOp::Sub => IrBinOp::Sub,
                            BinOp::Mul => IrBinOp::Mul,
                            BinOp::Div => IrBinOp::UDiv,
                            BinOp::Rem => IrBinOp::Rem, // unsigned rem: sdiv step replaced by udiv in codegen
                            BinOp::BitAnd => IrBinOp::BitAnd,
                            BinOp::BitOr => IrBinOp::BitOr,
                            BinOp::BitXor => IrBinOp::BitXor,
                            BinOp::Shl => IrBinOp::Shl,
                            BinOp::Shr => IrBinOp::UShr,
                            _ => unreachable!("matched above"),
                        };
                        self.instrs.push(Instr::BinOp { op: ir_op, dst, lhs: lhs_reg, rhs: rhs_reg });
                        Ok(IrValue::Reg(dst))
                    }
                    // FLS §6.5.5: Float arithmetic on f64 operands.
                    // Only Add/Sub/Mul/Div are defined for f64; bitwise ops and
                    // shifts are integer-only (FLS §6.5.6–§6.5.8).
                    IrTy::F64 => {
                        if !matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div) {
                            return Err(LowerError::Unsupported(
                                "bitwise/shift/rem operators are not defined for f64 (FLS §6.5.6)".into(),
                            ));
                        }
                        let lhs_val = self.lower_expr(lhs, &IrTy::F64)?;
                        let lhs_freg = match lhs_val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f64 binary op: lhs did not produce a float register".into(),
                            )),
                        };
                        let rhs_val = self.lower_expr(rhs, &IrTy::F64)?;
                        let rhs_freg = match rhs_val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f64 binary op: rhs did not produce a float register".into(),
                            )),
                        };
                        let dst = self.alloc_reg()?;
                        let f_op = match op {
                            BinOp::Add => F64BinOp::Add,
                            BinOp::Sub => F64BinOp::Sub,
                            BinOp::Mul => F64BinOp::Mul,
                            BinOp::Div => F64BinOp::Div,
                            _ => unreachable!("checked above"),
                        };
                        self.instrs.push(Instr::F64BinOp { op: f_op, dst, lhs: lhs_freg, rhs: rhs_freg });
                        Ok(IrValue::FReg(dst))
                    }
                    // FLS §6.5.5: Float arithmetic on f32 operands.
                    // Only Add/Sub/Mul/Div are defined for f32; bitwise ops and
                    // shifts are integer-only (FLS §6.5.6–§6.5.8).
                    IrTy::F32 => {
                        if !matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div) {
                            return Err(LowerError::Unsupported(
                                "bitwise/shift/rem operators are not defined for f32 (FLS §6.5.6)".into(),
                            ));
                        }
                        let lhs_val = self.lower_expr(lhs, &IrTy::F32)?;
                        let lhs_freg = match lhs_val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f32 binary op: lhs did not produce an f32 register".into(),
                            )),
                        };
                        let rhs_val = self.lower_expr(rhs, &IrTy::F32)?;
                        let rhs_freg = match rhs_val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f32 binary op: rhs did not produce an f32 register".into(),
                            )),
                        };
                        let dst = self.alloc_reg()?;
                        let f_op = match op {
                            BinOp::Add => F32BinOp::Add,
                            BinOp::Sub => F32BinOp::Sub,
                            BinOp::Mul => F32BinOp::Mul,
                            BinOp::Div => F32BinOp::Div,
                            _ => unreachable!("checked above"),
                        };
                        self.instrs.push(Instr::F32BinOp { op: f_op, dst, lhs: lhs_freg, rhs: rhs_freg });
                        Ok(IrValue::F32Reg(dst))
                    }
                    _ => Err(LowerError::Unsupported("bitwise/arithmetic on non-integer type".into())),
                }
            }

            // FLS §6.17: If expressions.
            //
            // An if expression evaluates the condition, then executes exactly one
            // of the two branches at runtime. The result value of the taken branch
            // is the value of the whole expression.
            //
            // Lowering strategy:
            //   1. Allocate a result stack slot (the "phi slot") before either branch.
            //   2. Lower the condition to a register.
            //   3. Emit `CondBranch` (cbz): if condition == 0 (false), jump to else.
            //   4. Lower the then-branch, store its result to the phi slot.
            //   5. Emit `Branch` (b) to end label.
            //   6. Emit `Label` for else.
            //   7. Lower the else-branch (or unit if absent), store result to phi slot.
            //   8. Emit `Label` for end.
            //   9. Load from phi slot into a fresh register.
            //
            // FLS §6.17: "The type of the if expression is the type of the last
            // expression in the block." Both branches must have the same type.
            //
            // FLS §6.1.2:37–45: The condition and both branches emit runtime
            // instructions. A `true` condition still emits `mov x0, #1; cbz x0, ...`
            // — the branch resolves at runtime, not compile time.
            //
            ExprKind::If { cond, then_block, else_expr } => {
                match ret_ty {
                    // FLS §6.17: If expression producing an i32 or bool value.
                    //
                    // Both types use the same phi-slot lowering pattern — the
                    // result is a 0/1 or signed integer in a register. The
                    // branches write to a shared stack slot; the result is
                    // loaded once after the if expression completes.
                    //
                    // Conditions are always lowered as `IrTy::Bool` so that
                    // `!bool_var` in a condition emits logical NOT (`eor`) rather
                    // than bitwise NOT (`mvn`). FLS §6.5.4: `!` on bool is logical.
                    //
                    // FLS §6.17: "The type of the if expression is the type of
                    // the last expression in the block." Both branches must have
                    // the same type.
                    //
                    // FLS §6.1.2:37–45: Even statically-known conditions emit
                    // a `cbz` — no constant folding of `if true { ... }`.
                    //
                    // Cache-line note: the phi slot is one 8-byte stack entry;
                    // read exactly once after the if expression completes.
                    IrTy::I32 | IrTy::Bool | IrTy::U32 | IrTy::FnPtr => {
                        let else_label = self.alloc_label();
                        let end_label = self.alloc_label();

                        // Allocate the phi slot before entering either branch so
                        // both branches write to the same stack location.
                        let phi_slot = self.alloc_slot()?;

                        // Lower condition as bool so that `!bool_var` emits
                        // logical NOT (BoolNot) rather than bitwise NOT (Not).
                        // FLS §6.5.4: `!` on bool must be logical NOT.
                        let cond_val = self.lower_expr(cond, &IrTy::Bool)?;
                        let cond_reg = self.val_to_reg(cond_val)?;

                        // CondBranch: jump to else_label if condition is false (0).
                        self.instrs.push(Instr::CondBranch { reg: cond_reg, label: else_label });

                        // ── Then branch ───────────────────────────────────────────
                        let then_val = self.lower_block_to_value(then_block, ret_ty)?;
                        let then_reg = self.val_to_reg(then_val)?;
                        self.instrs.push(Instr::Store { src: then_reg, slot: phi_slot });
                        self.instrs.push(Instr::Branch(end_label));

                        // ── Else branch ───────────────────────────────────────────
                        self.instrs.push(Instr::Label(else_label));
                        let else_val = match else_expr {
                            Some(e) => self.lower_expr(e, ret_ty)?,
                            None => {
                                // FLS §6.17: if without else has type `()`. Using it
                                // where i32/bool is expected is a type error — unsupported.
                                return Err(LowerError::Unsupported(
                                    "if expression without else in non-unit context".into(),
                                ));
                            }
                        };
                        let else_reg = self.val_to_reg(else_val)?;
                        self.instrs.push(Instr::Store { src: else_reg, slot: phi_slot });

                        // ── End ───────────────────────────────────────────────────
                        self.instrs.push(Instr::Label(end_label));
                        let result_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: result_reg, slot: phi_slot });
                        Ok(IrValue::Reg(result_reg))
                    }

                    // FLS §6.17: If expression with unit type (no value needed).
                    //
                    // Used when the if expression is a statement (value discarded)
                    // or when both branches produce `()`. The body of a loop uses
                    // this path, so `if cond { break; }` lowers correctly.
                    //
                    // No phi slot is allocated — the branches run for side effects.
                    //
                    // Conditions are lowered as `IrTy::Bool` so that `!b` as an
                    // if condition emits logical NOT. FLS §6.5.4.
                    //
                    // FLS §6.17: "If an if expression does not have an else expression,
                    // its type is the unit type."
                    //
                    // FLS §6.1.2:37–45: The condition still emits a runtime `cbz`.
                    IrTy::Unit => {
                        let else_label = self.alloc_label();
                        let end_label = self.alloc_label();

                        // Lower condition as bool — ensures `!bool_var` uses BoolNot.
                        // FLS §6.5.4: `!` on bool is logical NOT (eor), not bitwise (mvn).
                        let cond_val = self.lower_expr(cond, &IrTy::Bool)?;
                        let cond_reg = self.val_to_reg(cond_val)?;

                        // Jump to else (or end) if condition is false.
                        self.instrs.push(Instr::CondBranch { reg: cond_reg, label: else_label });

                        // ── Then branch (unit, side effects only) ─────────────────
                        self.lower_block_to_value(then_block, &IrTy::Unit)?;
                        self.instrs.push(Instr::Branch(end_label));

                        // ── Else branch ───────────────────────────────────────────
                        self.instrs.push(Instr::Label(else_label));
                        if let Some(e) = else_expr {
                            self.lower_expr(e, &IrTy::Unit)?;
                        }

                        self.instrs.push(Instr::Label(end_label));
                        Ok(IrValue::Unit)
                    }

                    // FLS §6.17: If expressions returning f64.
                    //
                    // Same phi-slot strategy as the I32 case, but using
                    // `StoreF64`/`LoadF64Slot` so the stack slot holds an
                    // 8-byte IEEE 754 double-precision value.
                    //
                    // FLS §4.2: Both branches must have type f64.
                    // FLS §6.1.2:37–45: All instructions emitted at runtime.
                    //
                    // Cache-line note: phi slot is one 8-byte entry; the
                    // `str d{r}` / `ldr d{r}` pair occupies 8 bytes total in
                    // the instruction stream (2 × 4-byte ARM64 instructions).
                    IrTy::F64 => {
                        let else_label = self.alloc_label();
                        let end_label = self.alloc_label();
                        let phi_slot = self.alloc_slot()?;

                        let cond_val = self.lower_expr(cond, &IrTy::Bool)?;
                        let cond_reg = self.val_to_reg(cond_val)?;
                        self.instrs.push(Instr::CondBranch { reg: cond_reg, label: else_label });

                        // Then branch.
                        let then_val = self.lower_block_to_value(then_block, &IrTy::F64)?;
                        let then_freg = match then_val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "if then-branch did not produce an f64 register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF64 { src: then_freg, slot: phi_slot });
                        self.instrs.push(Instr::Branch(end_label));

                        // Else branch.
                        self.instrs.push(Instr::Label(else_label));
                        let else_val = match else_expr {
                            Some(e) => self.lower_expr(e, &IrTy::F64)?,
                            None => return Err(LowerError::Unsupported(
                                "if expression without else in f64 context".into(),
                            )),
                        };
                        let else_freg = match else_val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "if else-branch did not produce an f64 register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF64 { src: else_freg, slot: phi_slot });

                        self.instrs.push(Instr::Label(end_label));
                        let result_freg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadF64Slot { dst: result_freg, slot: phi_slot });
                        Ok(IrValue::FReg(result_freg))
                    }

                    // FLS §6.17: If expressions returning f32.
                    //
                    // Same strategy as F64 but uses `StoreF32`/`LoadF32Slot`
                    // (4-byte slot values, same 8-byte slot alignment).
                    //
                    // FLS §4.2: Both branches must have type f32.
                    // FLS §6.1.2:37–45: All instructions emitted at runtime.
                    //
                    // Cache-line note: same 8-byte slot footprint as F64.
                    IrTy::F32 => {
                        let else_label = self.alloc_label();
                        let end_label = self.alloc_label();
                        let phi_slot = self.alloc_slot()?;

                        let cond_val = self.lower_expr(cond, &IrTy::Bool)?;
                        let cond_reg = self.val_to_reg(cond_val)?;
                        self.instrs.push(Instr::CondBranch { reg: cond_reg, label: else_label });

                        // Then branch.
                        let then_val = self.lower_block_to_value(then_block, &IrTy::F32)?;
                        let then_freg = match then_val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "if then-branch did not produce an f32 register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF32 { src: then_freg, slot: phi_slot });
                        self.instrs.push(Instr::Branch(end_label));

                        // Else branch.
                        self.instrs.push(Instr::Label(else_label));
                        let else_val = match else_expr {
                            Some(e) => self.lower_expr(e, &IrTy::F32)?,
                            None => return Err(LowerError::Unsupported(
                                "if expression without else in f32 context".into(),
                            )),
                        };
                        let else_freg = match else_val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "if else-branch did not produce an f32 register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF32 { src: else_freg, slot: phi_slot });

                        self.instrs.push(Instr::Label(end_label));
                        let result_freg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadF32Slot { dst: result_freg, slot: phi_slot });
                        Ok(IrValue::F32Reg(result_freg))
                    }
                }
            }

            // FLS §6.17: If-let expression.
            //
            // `if let pat = scrutinee { then_block } [else { else_expr }]`
            //
            // Lowering strategy:
            //   1. Lower scrutinee → spill to a stack slot.
            //   2. Emit a pattern check (same logic as match arm) with a
            //      conditional branch to else_label on no-match.
            //   3. If the pattern has an identifier binding, install it before
            //      lowering the then block and remove it after.
            //   4. Lower the then block.
            //   5. Branch to end_label.
            //   6. Emit else_label, lower else_expr (if any).
            //   7. Emit end_label; return result via phi slot (i32/bool) or unit.
            //
            // FLS §6.17: "An if let expression is syntactic sugar for a match
            // expression with a single arm." Lowered directly for clarity.
            // FLS §6.1.2:37–45: All checks emit runtime instructions.
            // Cache-line note: same instruction count as a 2-arm match.
            ExprKind::IfLet { pat, scrutinee, then_block, else_expr } => {
                // FLS §15: If the scrutinee is a plain variable holding an enum
                // value, record its base slot for TupleStruct/StructVariant pattern
                // field access. `scrut_slot` holds the discriminant copy; fields are
                // at `enum_base_slot + 1..N`.
                let enum_base_slot: Option<u8> =
                    if let ExprKind::Path(segs) = &scrutinee.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        self.locals
                            .get(var_name)
                            .copied()
                            .filter(|s| self.local_enum_types.contains_key(s))
                    } else {
                        None
                    };
                // FLS §5.3: If the scrutinee is a plain struct variable, record its
                // base slot for struct pattern field access. Fields are at base_slot +
                // field_idx (no discriminant offset, unlike enum variants).
                let struct_base_slot: Option<u8> =
                    if let ExprKind::Path(segs) = &scrutinee.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        self.locals
                            .get(var_name)
                            .copied()
                            .filter(|s| self.local_struct_types.contains_key(s))
                    } else {
                        None
                    };
                // Infer scrutinee type from the pattern (bool literal → Bool, else I32).
                let scrut_ty = match pat {
                    Pat::LitBool(_) => IrTy::Bool,
                    Pat::Or(alts) if alts.iter().any(|p| matches!(p, Pat::LitBool(_))) => {
                        IrTy::Bool
                    }
                    _ => IrTy::I32,
                };
                let scrut_val = self.lower_expr(scrutinee, &scrut_ty)?;
                let scrut_reg = self.val_to_reg(scrut_val)?;
                let scrut_slot = self.alloc_slot()?;
                self.instrs.push(Instr::Store { src: scrut_reg, slot: scrut_slot });

                // Bindings introduced by TupleStruct/StructVariant/Ident patterns;
                // all are removed after the then block (before the else branch).
                let mut bound_names: Vec<&str> = Vec::new();

                let else_label = self.alloc_label();
                let end_label = self.alloc_label();

                // Emit pattern check: branch to else_label on no-match.
                //
                // FLS §6.17: Wildcard and identifier patterns always match;
                // literal and range patterns require a runtime comparison.
                match pat {
                    Pat::Wildcard => {
                        // Always matches — no conditional branch needed.
                    }
                    Pat::Ident(_) => {
                        // Always matches — no conditional branch needed.
                        // Binding is installed below, before the then block.
                    }
                    Pat::LitInt(n) => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, *n as i32));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: else_label });
                    }
                    Pat::NegLitInt(n) => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, -(*n as i32)));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: else_label });
                    }
                    Pat::LitBool(b) => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, *b as i32));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: else_label });
                    }
                    // FLS §5.5 + §15: Path pattern — enum unit variant.
                    // Resolves to the variant's integer discriminant; then compares
                    // against the scrutinee exactly like a LitInt pattern.
                    Pat::Path(segs) if segs.len() == 2 => {
                        let enum_name = segs[0].text(self.source);
                        let variant_name = segs[1].text(self.source);
                        let discriminant = self.enum_defs
                            .get(enum_name)
                            .and_then(|v| v.get(variant_name))
                            .map(|(disc, _)| *disc)
                            .ok_or_else(|| LowerError::Unsupported(format!(
                                "unknown enum variant `{enum_name}::{variant_name}` in pattern"
                            )))?;
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: else_label });
                    }
                    Pat::Path(_) => {
                        return Err(LowerError::Unsupported(
                            "path pattern must have exactly two segments (EnumName::Variant)".into(),
                        ));
                    }
                    Pat::RangeInclusive { lo, hi } => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let lo_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                        let cmp1 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Ge,
                            dst: cmp1,
                            lhs: s_reg,
                            rhs: lo_reg,
                        });
                        let hi_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                        let cmp2 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Le,
                            dst: cmp2,
                            lhs: s_reg,
                            rhs: hi_reg,
                        });
                        let matched = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::BitAnd,
                            dst: matched,
                            lhs: cmp1,
                            rhs: cmp2,
                        });
                        self.instrs.push(Instr::CondBranch { reg: matched, label: else_label });
                    }
                    Pat::RangeExclusive { lo, hi } => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let lo_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                        let cmp1 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Ge,
                            dst: cmp1,
                            lhs: s_reg,
                            rhs: lo_reg,
                        });
                        let hi_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                        let cmp2 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Lt,
                            dst: cmp2,
                            lhs: s_reg,
                            rhs: hi_reg,
                        });
                        let matched = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::BitAnd,
                            dst: matched,
                            lhs: cmp1,
                            rhs: cmp2,
                        });
                        self.instrs.push(Instr::CondBranch { reg: matched, label: else_label });
                    }
                    Pat::Or(alts) => {
                        // FLS §5.1.11: OR pattern in if-let — match if any alternative matches.
                        // Strategy: accumulate equality into matched_reg (starts at 0).
                        let matched_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(matched_reg, 0));
                        for alt in alts {
                            match alt {
                                Pat::Wildcard => {
                                    self.instrs.push(Instr::LoadImm(matched_reg, 1));
                                    break;
                                }
                                Pat::Or(_) | Pat::Ident(_) => {
                                    return Err(LowerError::Unsupported(
                                        "nested OR or identifier inside if-let OR pattern".into(),
                                    ));
                                }
                                _ => {
                                    let alt_imm = self.pat_scalar_imm(alt)?;
                                    let si_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load {
                                        dst: si_reg,
                                        slot: scrut_slot,
                                    });
                                    let alt_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(alt_reg, alt_imm));
                                    let eq_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: eq_reg,
                                        lhs: si_reg,
                                        rhs: alt_reg,
                                    });
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::BitOr,
                                        dst: matched_reg,
                                        lhs: matched_reg,
                                        rhs: eq_reg,
                                    });
                                }
                            }
                        }
                        self.instrs.push(Instr::CondBranch {
                            reg: matched_reg,
                            label: else_label,
                        });
                    }
                    // FLS §5.4 + §15: TupleStruct pattern in if-let.
                    //
                    // `if let Enum::Variant(f0, f1, ..) = x { then } else { else }`
                    // Strategy: compare discriminant at scrut_slot against the variant's
                    // discriminant; branch to else_label on mismatch; then install
                    // positional field bindings from enum_base_slot + 1 + idx.
                    //
                    // FLS §6.1.2:37–45: All instructions are runtime.
                    // Cache-line note: ~5 instructions (ldr + mov + cmp + cset + cbz)
                    // for the discriminant check, plus 2×N for N field bindings.
                    Pat::TupleStruct { path: segs, fields } => {
                        if segs.len() != 2 {
                            return Err(LowerError::Unsupported(
                                "tuple struct pattern path must have two segments".into(),
                            ));
                        }
                        let enum_name = segs[0].text(self.source);
                        let variant_name = segs[1].text(self.source);
                        let discriminant = self
                            .enum_defs
                            .get(enum_name)
                            .and_then(|v| v.get(variant_name))
                            .map(|(disc, _)| *disc)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown enum variant `{enum_name}::{variant_name}`"
                                ))
                            })?;
                        let base = enum_base_slot.ok_or_else(|| {
                            LowerError::Unsupported(
                                "TupleStruct pattern in if-let requires enum variable scrutinee"
                                    .into(),
                            )
                        })?;
                        // Discriminant check — branch to else on mismatch.
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                        let cmp_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: cmp_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: else_label });
                        // Install positional field bindings into bound_names.
                        // FLS §4.2, §15: Float fields use LoadF64Slot/LoadF32Slot.
                        // FLS §4.2: Register f64/f32 bindings in float_locals/float32_locals so
                        // that path expressions like `x as i32` correctly emit fcvtzs (F64ToI32).
                        for (fi, fp) in fields.iter().enumerate() {
                            if let Pat::Ident(span) = fp {
                                let fname = span.text(self.source);
                                let fslot = base + 1 + fi as u8;
                                let bslot = self.alloc_slot()?;
                                match self.enum_variant_field_float_ty(enum_name, variant_name, fi) {
                                    Some(IrTy::F64) => {
                                        let freg = self.alloc_reg()?;
                                        self.instrs.push(Instr::LoadF64Slot { dst: freg, slot: fslot });
                                        self.instrs.push(Instr::StoreF64 { src: freg, slot: bslot });
                                        self.slot_float_ty.insert(bslot, IrTy::F64);
                                        self.float_locals.insert(fname, bslot);
                                    }
                                    Some(IrTy::F32) => {
                                        let freg = self.alloc_reg()?;
                                        self.instrs.push(Instr::LoadF32Slot { dst: freg, slot: fslot });
                                        self.instrs.push(Instr::StoreF32 { src: freg, slot: bslot });
                                        self.slot_float_ty.insert(bslot, IrTy::F32);
                                        self.float32_locals.insert(fname, bslot);
                                    }
                                    _ => {
                                        let breg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                        self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                    }
                                }
                                self.locals.insert(fname, bslot);
                                bound_names.push(fname);
                            } else if !matches!(fp, Pat::Wildcard) {
                                return Err(LowerError::Unsupported(
                                    "only ident/wildcard fields in TupleStruct if-let pattern"
                                        .into(),
                                ));
                            }
                        }
                    }
                    // FLS §5.3 + §15.3: Named-field struct or variant pattern in if-let.
                    //
                    // One-segment path (`Point { x, y }`): plain struct pattern — irrefutable,
                    // no discriminant check, fields at struct_base_slot + field_idx.
                    // Two-segment path (`Enum::Variant { field, … }`): enum variant — compare
                    // discriminant, branch to else_label on mismatch, bind fields at
                    // enum_base_slot + 1 + field_idx.
                    //
                    // FLS §5.3: "A struct pattern matches a struct or enum struct variant
                    // by its field patterns."
                    // FLS §6.1.2:37–45: All instructions are runtime.
                    // Cache-line note: 2×N instructions (plain struct); ~5 + 2×N (enum variant).
                    Pat::StructVariant { path: segs, fields: pat_fields } => {
                        if segs.len() == 1 {
                            // FLS §5.3: Plain struct pattern — always matches (irrefutable).
                            let struct_name = segs[0].text(self.source);
                            let base = struct_base_slot.ok_or_else(|| {
                                LowerError::Unsupported(
                                    "plain struct pattern in if-let requires struct variable scrutinee".into(),
                                )
                            })?;
                            let field_names = self.struct_defs
                                .get(struct_name)
                                .cloned()
                                .ok_or_else(|| LowerError::Unsupported(format!(
                                    "unknown struct `{struct_name}`"
                                )))?;
                            // No CondBranch — plain struct patterns are irrefutable.
                            for (fname_span, fp) in pat_fields.iter() {
                                let fname = fname_span.text(self.source);
                                let field_idx = field_names.iter().position(|n| n == fname)
                                    .ok_or_else(|| LowerError::Unsupported(format!(
                                        "struct `{struct_name}` has no field `{fname}`"
                                    )))?;
                                match fp {
                                    Pat::Ident(bind_span) => {
                                        let bind_name = bind_span.text(self.source);
                                        let fslot = base + field_idx as u8;
                                        let bslot = self.alloc_slot()?;
                                        let breg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                        self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                        self.locals.insert(bind_name, bslot);
                                        bound_names.push(bind_name);
                                    }
                                    Pat::Wildcard => {}
                                    _ => return Err(LowerError::Unsupported(
                                        "only ident/wildcard sub-patterns in struct if-let patterns".into(),
                                    )),
                                }
                            }
                        } else if segs.len() == 2 {
                            let enum_name = segs[0].text(self.source);
                            let variant_name = segs[1].text(self.source);
                            let (discriminant, field_names) = self
                                .enum_defs
                                .get(enum_name)
                                .and_then(|v| v.get(variant_name))
                                .cloned()
                                .ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "unknown enum variant `{enum_name}::{variant_name}`"
                                    ))
                                })?;
                            let base = enum_base_slot.ok_or_else(|| {
                                LowerError::Unsupported(
                                    "StructVariant pattern in if-let requires enum variable scrutinee"
                                        .into(),
                                )
                            })?;
                            // Discriminant check — branch to else on mismatch.
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                            let cmp_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp {
                                op: IrBinOp::Eq,
                                dst: cmp_reg,
                                lhs: s_reg,
                                rhs: p_reg,
                            });
                            self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: else_label });
                            // Install named field bindings by declaration order.
                            // FLS §4.2, §15: Float fields use LoadF64Slot/LoadF32Slot
                            // and register in float_locals so path expressions emit
                            // the correct float instructions downstream.
                            for (fname_span, fp) in pat_fields.iter() {
                                let fname = fname_span.text(self.source);
                                let field_idx = field_names
                                    .iter()
                                    .position(|n| n == fname)
                                    .ok_or_else(|| {
                                        LowerError::Unsupported(format!(
                                            "enum variant `{enum_name}::{variant_name}` has no field `{fname}`"
                                        ))
                                    })?;
                                match fp {
                                    Pat::Ident(bind_span) => {
                                        let bind_name = bind_span.text(self.source);
                                        let fslot = base + 1 + field_idx as u8;
                                        let bslot = self.alloc_slot()?;
                                        match self.enum_variant_field_float_ty(enum_name, variant_name, field_idx) {
                                            Some(IrTy::F64) => {
                                                let freg = self.alloc_reg()?;
                                                self.instrs.push(Instr::LoadF64Slot { dst: freg, slot: fslot });
                                                self.instrs.push(Instr::StoreF64 { src: freg, slot: bslot });
                                                self.slot_float_ty.insert(bslot, IrTy::F64);
                                                self.float_locals.insert(bind_name, bslot);
                                            }
                                            Some(IrTy::F32) => {
                                                let freg = self.alloc_reg()?;
                                                self.instrs.push(Instr::LoadF32Slot { dst: freg, slot: fslot });
                                                self.instrs.push(Instr::StoreF32 { src: freg, slot: bslot });
                                                self.slot_float_ty.insert(bslot, IrTy::F32);
                                                self.float32_locals.insert(bind_name, bslot);
                                            }
                                            _ => {
                                                let breg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                            }
                                        }
                                        self.locals.insert(bind_name, bslot);
                                        bound_names.push(bind_name);
                                    }
                                    Pat::Wildcard => {}
                                    _ => return Err(LowerError::Unsupported(
                                        "only ident/wildcard sub-patterns in StructVariant if-let fields".into(),
                                    )),
                                }
                            }
                        } else {
                            return Err(LowerError::Unsupported(
                                "struct/variant if-let pattern path must have 1 or 2 segments".into(),
                            ));
                        }
                    }
                    Pat::Tuple(_) => {
                        return Err(LowerError::Unsupported(
                            "tuple pattern in if-let not yet supported".into(),
                        ));
                    }
                    Pat::Slice(_) => {
                        return Err(LowerError::Unsupported(
                            "slice/array pattern in if-let not yet supported".into(),
                        ));
                    }
                }

                // Install identifier binding (if any) before the then block.
                // FLS §5.1.4: The binding is in scope for the then block only.
                // TupleStruct/StructVariant field bindings were already pushed to
                // bound_names inside the pattern check above.
                if let Pat::Ident(span) = pat {
                    let name = span.text(self.source);
                    let bind_slot = self.alloc_slot()?;
                    let bind_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                    self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                    self.locals.insert(name, bind_slot);
                    bound_names.push(name);
                }

                match ret_ty {
                    IrTy::I32 | IrTy::Bool | IrTy::U32 | IrTy::FnPtr => {
                        let phi_slot = self.alloc_slot()?;

                        // Then branch.
                        let then_val = self.lower_block_to_value(then_block, ret_ty)?;
                        for name in &bound_names {
                            self.locals.remove(*name);
                            self.float_locals.remove(*name);
                            self.float32_locals.remove(*name);
                        }
                        let then_reg = self.val_to_reg(then_val)?;
                        self.instrs.push(Instr::Store { src: then_reg, slot: phi_slot });
                        self.instrs.push(Instr::Branch(end_label));

                        // Else branch.
                        self.instrs.push(Instr::Label(else_label));
                        let else_val = match else_expr {
                            Some(e) => self.lower_expr(e, ret_ty)?,
                            None => {
                                return Err(LowerError::Unsupported(
                                    "if-let without else in non-unit context".into(),
                                ));
                            }
                        };
                        let else_reg = self.val_to_reg(else_val)?;
                        self.instrs.push(Instr::Store { src: else_reg, slot: phi_slot });

                        self.instrs.push(Instr::Label(end_label));
                        let result_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: result_reg, slot: phi_slot });
                        Ok(IrValue::Reg(result_reg))
                    }
                    IrTy::Unit => {
                        // Then branch (side effects only).
                        self.lower_block_to_value(then_block, &IrTy::Unit)?;
                        for name in &bound_names {
                            self.locals.remove(*name);
                            self.float_locals.remove(*name);
                            self.float32_locals.remove(*name);
                        }
                        self.instrs.push(Instr::Branch(end_label));

                        // Else branch.
                        self.instrs.push(Instr::Label(else_label));
                        if let Some(e) = else_expr {
                            self.lower_expr(e, &IrTy::Unit)?;
                        }

                        self.instrs.push(Instr::Label(end_label));
                        Ok(IrValue::Unit)
                    }
                    // FLS §6.17: If-let expressions returning f64.
                    //
                    // Same phi-slot strategy as I32 but using float store/load.
                    // FLS §4.2: Both branches must have type f64.
                    // FLS §6.1.2:37–45: All instructions emitted at runtime.
                    IrTy::F64 => {
                        let phi_slot = self.alloc_slot()?;

                        // Then branch.
                        let then_val = self.lower_block_to_value(then_block, &IrTy::F64)?;
                        for name in &bound_names {
                            self.locals.remove(*name);
                            self.float_locals.remove(*name);
                            self.float32_locals.remove(*name);
                        }
                        let then_freg = match then_val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "if-let then-branch did not produce an f64 register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF64 { src: then_freg, slot: phi_slot });
                        self.instrs.push(Instr::Branch(end_label));

                        // Else branch.
                        self.instrs.push(Instr::Label(else_label));
                        let else_val = match else_expr {
                            Some(e) => self.lower_expr(e, &IrTy::F64)?,
                            None => return Err(LowerError::Unsupported(
                                "if-let without else in f64 context".into(),
                            )),
                        };
                        let else_freg = match else_val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "if-let else-branch did not produce an f64 register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF64 { src: else_freg, slot: phi_slot });

                        self.instrs.push(Instr::Label(end_label));
                        let result_freg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadF64Slot { dst: result_freg, slot: phi_slot });
                        Ok(IrValue::FReg(result_freg))
                    }

                    // FLS §6.17: If-let expressions returning f32.
                    //
                    // Same phi-slot strategy as F64 but using f32 store/load.
                    // FLS §4.2: Both branches must have type f32.
                    // FLS §6.1.2:37–45: All instructions emitted at runtime.
                    IrTy::F32 => {
                        let phi_slot = self.alloc_slot()?;

                        // Then branch.
                        let then_val = self.lower_block_to_value(then_block, &IrTy::F32)?;
                        for name in &bound_names {
                            self.locals.remove(*name);
                            self.float_locals.remove(*name);
                            self.float32_locals.remove(*name);
                        }
                        let then_freg = match then_val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "if-let then-branch did not produce an f32 register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF32 { src: then_freg, slot: phi_slot });
                        self.instrs.push(Instr::Branch(end_label));

                        // Else branch.
                        self.instrs.push(Instr::Label(else_label));
                        let else_val = match else_expr {
                            Some(e) => self.lower_expr(e, &IrTy::F32)?,
                            None => return Err(LowerError::Unsupported(
                                "if-let without else in f32 context".into(),
                            )),
                        };
                        let else_freg = match else_val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "if-let else-branch did not produce an f32 register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF32 { src: else_freg, slot: phi_slot });

                        self.instrs.push(Instr::Label(end_label));
                        let result_freg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadF32Slot { dst: result_freg, slot: phi_slot });
                        Ok(IrValue::F32Reg(result_freg))
                    }
                }
            }

            // FLS §6.18: Match expression.
            // FLS §5.1.4: Identifier patterns — bind scrutinee to a name.
            //
            // `match scrutinee { pat0 => body0, ..., default => bodyN }`
            //
            // Lowering strategy (comparison chain):
            //   1. Lower scrutinee → spill to a stack slot so it is safe across
            //      subsequent instruction sequences (each arm re-loads it).
            //   2. For each arm except the last: test `scrutinee == pat_val`
            //      using the existing BinOp(Eq) path and CondBranch; if the
            //      check fails, jump to the next arm's label.
            //   3. The last arm is emitted unconditionally (the compiler assumes
            //      exhaustiveness; a future type-checking pass will enforce it).
            //   4. Each arm stores its result to a phi slot (non-unit result)
            //      and branches to exit_label, or executes for side effects
            //      (unit result).
            //
            // FLS §6.18: "A match expression evaluates the scrutinee and then
            // matches it against each pattern in order. The first matching arm
            // executes."
            //
            // FLS §6.1.2:37–45: The scrutinee load and all pattern comparisons
            // emit runtime instructions — even a match on a literal constant
            // emits `ldr` + `mov` + `cmp` + `cset` + `cbz`.
            //
            // FLS §6.18 AMBIGUOUS: The spec requires exhaustiveness but does not
            // specify the checking algorithm. Galvanic defers exhaustiveness
            // checking to a future pass; the last arm is always emitted
            // unconditionally.
            //
            // Cache-line note: each arm costs ~5 instructions (ldr scrut + mov
            // pat + cmp + cset + cbz = 20 bytes); with the branch (4 bytes) and
            // label (0 bytes) that is 24 bytes per arm — two arms per 64-byte line.
            ExprKind::Match { scrutinee, arms } => {
                if arms.is_empty() {
                    return Err(LowerError::Unsupported("match expression with no arms".into()));
                }

                // Lower the scrutinee and spill to a stack slot.
                // The scrutinee may be i32 or bool; both use integer registers.
                // We infer the scrutinee type from context: use i32 by default,
                // and use bool if any arm has a bool literal pattern (including
                // bool literals inside OR patterns, FLS §5.1.11).
                let has_bool_pat = |pat: &Pat| -> bool {
                    match pat {
                        Pat::LitBool(_) => true,
                        // FLS §5.1.11: OR patterns can contain bool literal alternatives.
                        Pat::Or(alts) => alts.iter().any(|p| matches!(p, Pat::LitBool(_))),
                        // FLS §5.1.4: Identifier patterns are type-agnostic; they
                        // don't determine the scrutinee type on their own.
                        _ => false,
                    }
                };
                let scrut_ty = if arms.iter().any(|a| has_bool_pat(&a.pat)) {
                    IrTy::Bool
                } else {
                    IrTy::I32
                };
                // FLS §15: If the scrutinee is a plain variable holding an enum
                // tuple variant, record its base slot for TupleStruct pattern field
                // access. `scrut_slot` holds the discriminant copy; fields are at
                // `enum_base_slot + 1..N`.
                let enum_base_slot: Option<u8> =
                    if let ExprKind::Path(segs) = &scrutinee.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        self.locals
                            .get(var_name)
                            .copied()
                            .filter(|s| self.local_enum_types.contains_key(s))
                    } else {
                        None
                    };
                // FLS §5.3: If the scrutinee is a plain struct variable, record its
                // base slot for struct pattern field access. Fields are at base_slot +
                // field_idx (no discriminant offset, unlike enum variants).
                let struct_base_slot: Option<u8> =
                    if let ExprKind::Path(segs) = &scrutinee.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        self.locals
                            .get(var_name)
                            .copied()
                            .filter(|s| self.local_struct_types.contains_key(s))
                    } else {
                        None
                    };

                let scrut_val = self.lower_expr(scrutinee, &scrut_ty)?;
                let scrut_reg = self.val_to_reg(scrut_val)?;
                let scrut_slot = self.alloc_slot()?;
                self.instrs.push(Instr::Store { src: scrut_reg, slot: scrut_slot });

                // Split into checked arms (all but the last) and the default arm.
                // The default arm is emitted unconditionally (exhaustiveness deferred).
                let (checked_arms, default_arm) = arms.split_at(arms.len() - 1);

                let exit_label = self.alloc_label();

                match ret_ty {
                    IrTy::I32 | IrTy::Bool | IrTy::U32 | IrTy::FnPtr => {
                        let phi_slot = self.alloc_slot()?;

                        for arm in checked_arms {
                            let next_label = self.alloc_label();

                            match &arm.pat {
                                Pat::Wildcard => {
                                    // Wildcard in non-last position — pattern always matches.
                                    // Check guard (if any) before executing the body.
                                    //
                                    // FLS §6.18: "A match arm guard is an additional condition
                                    // that must hold for the arm to be selected."
                                    // FLS §6.1.2:37–45: Guard check emits runtime instructions.
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch {
                                            reg: gr,
                                            label: next_label,
                                        });
                                    }
                                    let body_val = self.lower_expr(&arm.body, ret_ty)?;
                                    let body_reg = self.val_to_reg(body_val)?;
                                    self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });
                                    self.instrs.push(Instr::Branch(exit_label));
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.1.4: Identifier pattern in non-last position —
                                // always matches (like Wildcard) and binds scrutinee to name.
                                //
                                // Load scrutinee into a new slot keyed by the identifier name.
                                // The arm body accesses it via a normal path expression (Load).
                                // The binding is removed after the arm to avoid leaking into
                                // subsequent arms (correct scoping per FLS §5.1.4).
                                //
                                // FLS §6.18: Guard (if present) is evaluated after the binding
                                // is installed; the guard expression may reference the bound name.
                                // If the guard fails, the arm is skipped (CondBranch to next_label)
                                // and the binding is removed from `self.locals` before the label.
                                //
                                // FLS §6.1.2:37–45: The ldr/str pair emits at runtime.
                                // Cache-line note: 2 instructions (ldr + str = 8 bytes).
                                Pat::Ident(span) => {
                                    let name = span.text(self.source);
                                    let bind_slot = self.alloc_slot()?;
                                    let bind_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                                    self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                                    self.locals.insert(name, bind_slot);
                                    // Guard check: binding is visible to the guard expression.
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch {
                                            reg: gr,
                                            label: next_label,
                                        });
                                    }
                                    let body_val = self.lower_expr(&arm.body, ret_ty)?;
                                    let body_reg = self.val_to_reg(body_val)?;
                                    self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });
                                    self.instrs.push(Instr::Branch(exit_label));
                                    // Remove binding before the label so subsequent arms
                                    // do not see a stale entry in `self.locals`.
                                    self.locals.remove(name);
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.1.11: OR pattern — accumulate equality results.
                                //
                                // Strategy: matched_reg starts at 0; for each alternative,
                                // load scrutinee, compare with pattern value, OR the 0/1
                                // equality result into matched_reg. After all alternatives,
                                // cbz matched_reg → next_label (arm not taken).
                                //
                                // FLS §6.1.2:37–45: All comparisons emit runtime instructions.
                                //
                                // Cache-line note: each alternative adds ~4 instructions
                                // (ldr + mov + cmp/cset + orr = 16 bytes), so 4 alternatives
                                // fit in a 64-byte instruction cache line.
                                Pat::Or(alts) => {
                                    let matched_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(matched_reg, 0));
                                    for alt in alts {
                                        match alt {
                                            Pat::Wildcard => {
                                                // Wildcard inside OR — always matches.
                                                self.instrs.push(Instr::LoadImm(matched_reg, 1));
                                                break;
                                            }
                                            Pat::Or(_) => {
                                                return Err(LowerError::Unsupported(
                                                    "nested OR patterns".into(),
                                                ));
                                            }
                                            // FLS §5.1.4: Identifier patterns inside OR are
                                            // not yet supported (binding semantics are complex
                                            // when combined with OR alternatives).
                                            Pat::Ident(_) => {
                                                return Err(LowerError::Unsupported(
                                                    "identifier pattern inside OR pattern".into(),
                                                ));
                                            }
                                            _ => {
                                                let alt_imm = self.pat_scalar_imm(alt)?;
                                                let si_reg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load {
                                                    dst: si_reg,
                                                    slot: scrut_slot,
                                                });
                                                let alt_reg = self.alloc_reg()?;
                                                self.instrs.push(Instr::LoadImm(alt_reg, alt_imm));
                                                let eq_reg = self.alloc_reg()?;
                                                self.instrs.push(Instr::BinOp {
                                                    op: IrBinOp::Eq,
                                                    dst: eq_reg,
                                                    lhs: si_reg,
                                                    rhs: alt_reg,
                                                });
                                                self.instrs.push(Instr::BinOp {
                                                    op: IrBinOp::BitOr,
                                                    dst: matched_reg,
                                                    lhs: matched_reg,
                                                    rhs: eq_reg,
                                                });
                                            }
                                        }
                                    }
                                    // cbz: skip arm if no alternative matched.
                                    self.instrs.push(Instr::CondBranch {
                                        reg: matched_reg,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.1.9: Inclusive range pattern `lo..=hi`.
                                //
                                // Emit: lo <= scrutinee && scrutinee <= hi.
                                // Strategy: compare scrutinee >= lo (IrBinOp::Ge) → cmp1,
                                // compare scrutinee <= hi (IrBinOp::Le) → cmp2,
                                // AND the two results → matched, cbz on matched.
                                //
                                // FLS §6.1.2:37–45: All comparisons emit runtime instructions.
                                //
                                // Cache-line note: 7 instructions per range arm (ldr + 2×mov
                                // + 2×cmp + and + cbz = 28 bytes) — two arms per 64-byte line.
                                Pat::RangeInclusive { lo, hi } => {
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let lo_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                                    let cmp1 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Ge,
                                        dst: cmp1,
                                        lhs: s_reg,
                                        rhs: lo_reg,
                                    });
                                    let hi_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                                    let cmp2 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Le,
                                        dst: cmp2,
                                        lhs: s_reg,
                                        rhs: hi_reg,
                                    });
                                    let matched = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::BitAnd,
                                        dst: matched,
                                        lhs: cmp1,
                                        rhs: cmp2,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: matched,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.1.9: Exclusive range pattern `lo..hi`.
                                //
                                // Emit: lo <= scrutinee && scrutinee < hi.
                                // Same as inclusive but uses Lt for the upper bound.
                                Pat::RangeExclusive { lo, hi } => {
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let lo_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                                    let cmp1 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Ge,
                                        dst: cmp1,
                                        lhs: s_reg,
                                        rhs: lo_reg,
                                    });
                                    let hi_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                                    let cmp2 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Lt,
                                        dst: cmp2,
                                        lhs: s_reg,
                                        rhs: hi_reg,
                                    });
                                    let matched = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::BitAnd,
                                        dst: matched,
                                        lhs: cmp1,
                                        rhs: cmp2,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: matched,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.5 + §15: Path pattern — enum unit variant.
                                // Resolve to discriminant; emit equality check like LitInt.
                                // FLS §6.1.2:37–45: Comparison emits runtime instructions.
                                Pat::Path(segs) => {
                                    let pat_imm = if segs.len() == 2 {
                                        let enum_name = segs[0].text(self.source);
                                        let variant_name = segs[1].text(self.source);
                                        self.enum_defs
                                            .get(enum_name)
                                            .and_then(|v| v.get(variant_name))
                                            .map(|(disc, _)| *disc)
                                            .ok_or_else(|| LowerError::Unsupported(format!(
                                                "unknown enum variant `{enum_name}::{variant_name}` in match pattern"
                                            )))?
                                    } else {
                                        return Err(LowerError::Unsupported(
                                            "path pattern must have exactly two segments".into(),
                                        ));
                                    };
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let pat_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(pat_reg, pat_imm));
                                    let cmp_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: cmp_reg,
                                        lhs: s_reg,
                                        rhs: pat_reg,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: cmp_reg,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.4 + §15: Tuple struct/variant pattern.
                                //
                                // Strategy: compare discriminant at scrut_slot against
                                // the variant's discriminant, branch on mismatch, then
                                // install field bindings from enum_base_slot + 1 + idx.
                                //
                                // FLS §6.1.2:37–45: All instructions are runtime.
                                // Cache-line note: ~5 instructions (ldr + mov + cmp +
                                // cbz + N×ldr per field binding = 20+ bytes).
                                Pat::TupleStruct { path: segs, fields } => {
                                    if segs.len() != 2 {
                                        return Err(LowerError::Unsupported(
                                            "tuple struct pattern path must have two segments".into(),
                                        ));
                                    }
                                    let enum_name = segs[0].text(self.source);
                                    let variant_name = segs[1].text(self.source);
                                    let discriminant = self.enum_defs
                                        .get(enum_name)
                                        .and_then(|v| v.get(variant_name))
                                        .map(|(disc, _)| *disc)
                                        .ok_or_else(|| LowerError::Unsupported(format!(
                                            "unknown enum variant `{enum_name}::{variant_name}`"
                                        )))?;
                                    let base = enum_base_slot.ok_or_else(|| {
                                        LowerError::Unsupported(
                                            "TupleStruct pattern requires enum variable scrutinee".into(),
                                        )
                                    })?;
                                    // Discriminant check.
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let p_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                                    let cmp_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: cmp_reg,
                                        lhs: s_reg,
                                        rhs: p_reg,
                                    });
                                    self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: next_label });
                                    // Install field bindings.
                                    // FLS §4.2, §15: Float fields use LoadF64Slot/LoadF32Slot.
                                    // FLS §4.2: Register f64/f32 bindings in float_locals/float32_locals.
                                    let mut bound: Vec<&str> = Vec::new();
                                    for (fi, fp) in fields.iter().enumerate() {
                                        if let Pat::Ident(span) = fp {
                                            let fname = span.text(self.source);
                                            let fslot = base + 1 + fi as u8;
                                            let bslot = self.alloc_slot()?;
                                            match self.enum_variant_field_float_ty(enum_name, variant_name, fi) {
                                                Some(IrTy::F64) => {
                                                    let freg = self.alloc_reg()?;
                                                    self.instrs.push(Instr::LoadF64Slot { dst: freg, slot: fslot });
                                                    self.instrs.push(Instr::StoreF64 { src: freg, slot: bslot });
                                                    self.slot_float_ty.insert(bslot, IrTy::F64);
                                                    self.float_locals.insert(fname, bslot);
                                                }
                                                Some(IrTy::F32) => {
                                                    let freg = self.alloc_reg()?;
                                                    self.instrs.push(Instr::LoadF32Slot { dst: freg, slot: fslot });
                                                    self.instrs.push(Instr::StoreF32 { src: freg, slot: bslot });
                                                    self.slot_float_ty.insert(bslot, IrTy::F32);
                                                    self.float32_locals.insert(fname, bslot);
                                                }
                                                _ => {
                                                    let breg = self.alloc_reg()?;
                                                    self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                    self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                }
                                            }
                                            self.locals.insert(fname, bslot);
                                            bound.push(fname);
                                        } else if !matches!(fp, Pat::Wildcard) {
                                            return Err(LowerError::Unsupported(
                                                "only ident/wildcard field patterns in TupleStruct".into(),
                                            ));
                                        }
                                    }
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                                    }
                                    let body_val = self.lower_expr(&arm.body, ret_ty)?;
                                    let body_reg = self.val_to_reg(body_val)?;
                                    self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });
                                    self.instrs.push(Instr::Branch(exit_label));
                                    for name in &bound {
                                        self.locals.remove(*name);
                                        self.float_locals.remove(*name);
                                        self.float32_locals.remove(*name);
                                    }
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.3 + §15.3: Named-field struct or enum variant pattern
                                // (value-returning match).
                                //
                                // One-segment path (`Point { x, y }`): plain struct pattern —
                                // irrefutable, no discriminant check, fields at base_slot + idx.
                                // Two-segment path (`Enum::Variant { field, … }`): enum variant —
                                // check discriminant, then bind fields at enum_base_slot + 1 + idx.
                                //
                                // FLS §5.3: "A struct pattern matches a struct or enum struct variant
                                // by its field patterns."
                                // FLS §6.1.2:37–45: All instructions are runtime.
                                // Cache-line note: ~5 + 2×N instructions per arm (enum variant);
                                // 2×N instructions per arm (plain struct, no discriminant check).
                                Pat::StructVariant { path: segs, fields: pat_fields } => {
                                    let mut bound_sv: Vec<&str> = Vec::new();
                                    if segs.len() == 1 {
                                        // FLS §5.3: Plain struct pattern — always matches.
                                        let struct_name = segs[0].text(self.source);
                                        let base = struct_base_slot.ok_or_else(|| {
                                            LowerError::Unsupported(
                                                "plain struct pattern requires struct variable scrutinee".into(),
                                            )
                                        })?;
                                        let field_names = self.struct_defs
                                            .get(struct_name)
                                            .cloned()
                                            .ok_or_else(|| LowerError::Unsupported(format!(
                                                "unknown struct `{struct_name}`"
                                            )))?;
                                        // No CondBranch — plain struct patterns are irrefutable.
                                        for (fname_span, fp) in pat_fields.iter() {
                                            let fname = fname_span.text(self.source);
                                            let field_idx = field_names.iter().position(|n| n == fname)
                                                .ok_or_else(|| LowerError::Unsupported(format!(
                                                    "struct `{struct_name}` has no field `{fname}`"
                                                )))?;
                                            match fp {
                                                Pat::Ident(bind_span) => {
                                                    let bind_name = bind_span.text(self.source);
                                                    let fslot = base + field_idx as u8;
                                                    let bslot = self.alloc_slot()?;
                                                    let breg = self.alloc_reg()?;
                                                    self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                    self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                    self.locals.insert(bind_name, bslot);
                                                    bound_sv.push(bind_name);
                                                }
                                                Pat::Wildcard => {}
                                                _ => return Err(LowerError::Unsupported(
                                                    "only ident/wildcard sub-patterns in struct patterns".into(),
                                                )),
                                            }
                                        }
                                    } else if segs.len() == 2 {
                                        let enum_name = segs[0].text(self.source);
                                        let variant_name = segs[1].text(self.source);
                                        let (discriminant, field_names) = self.enum_defs
                                            .get(enum_name)
                                            .and_then(|v| v.get(variant_name))
                                            .cloned()
                                            .ok_or_else(|| LowerError::Unsupported(format!(
                                                "unknown enum variant `{enum_name}::{variant_name}`"
                                            )))?;
                                        let base = enum_base_slot.ok_or_else(|| {
                                            LowerError::Unsupported(
                                                "StructVariant pattern requires enum variable scrutinee".into(),
                                            )
                                        })?;
                                        // Discriminant check.
                                        let s_reg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                        let p_reg = self.alloc_reg()?;
                                        self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                                        let cmp_reg = self.alloc_reg()?;
                                        self.instrs.push(Instr::BinOp {
                                            op: IrBinOp::Eq,
                                            dst: cmp_reg,
                                            lhs: s_reg,
                                            rhs: p_reg,
                                        });
                                        self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: next_label });
                                        // Install named field bindings by declaration order.
                                        // FLS §4.2, §15: Float fields use float loads/stores.
                                        for (fname_span, fp) in pat_fields.iter() {
                                            let fname = fname_span.text(self.source);
                                            let field_idx = field_names.iter().position(|n| n == fname)
                                                .ok_or_else(|| LowerError::Unsupported(format!(
                                                    "enum variant `{enum_name}::{variant_name}` has no field `{fname}`"
                                                )))?;
                                            match fp {
                                                Pat::Ident(bind_span) => {
                                                    let bind_name = bind_span.text(self.source);
                                                    let fslot = base + 1 + field_idx as u8;
                                                    let bslot = self.alloc_slot()?;
                                                    match self.enum_variant_field_float_ty(enum_name, variant_name, field_idx) {
                                                        Some(IrTy::F64) => {
                                                            let freg = self.alloc_reg()?;
                                                            self.instrs.push(Instr::LoadF64Slot { dst: freg, slot: fslot });
                                                            self.instrs.push(Instr::StoreF64 { src: freg, slot: bslot });
                                                            self.slot_float_ty.insert(bslot, IrTy::F64);
                                                            self.float_locals.insert(bind_name, bslot);
                                                        }
                                                        Some(IrTy::F32) => {
                                                            let freg = self.alloc_reg()?;
                                                            self.instrs.push(Instr::LoadF32Slot { dst: freg, slot: fslot });
                                                            self.instrs.push(Instr::StoreF32 { src: freg, slot: bslot });
                                                            self.slot_float_ty.insert(bslot, IrTy::F32);
                                                            self.float32_locals.insert(bind_name, bslot);
                                                        }
                                                        _ => {
                                                            let breg = self.alloc_reg()?;
                                                            self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                            self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                        }
                                                    }
                                                    self.locals.insert(bind_name, bslot);
                                                    bound_sv.push(bind_name);
                                                }
                                                Pat::Wildcard => {}
                                                _ => return Err(LowerError::Unsupported(
                                                    "only ident/wildcard sub-patterns in StructVariant fields".into(),
                                                )),
                                            }
                                        }
                                    } else {
                                        return Err(LowerError::Unsupported(
                                            "struct/variant pattern path must have 1 or 2 segments".into(),
                                        ));
                                    }
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                                    }
                                    let body_val = self.lower_expr(&arm.body, ret_ty)?;
                                    let body_reg = self.val_to_reg(body_val)?;
                                    self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });
                                    self.instrs.push(Instr::Branch(exit_label));
                                    for name in &bound_sv { self.locals.remove(*name); }
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                _ => {
                                    // Single literal pattern: load scrutinee, compare, cbz.
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs
                                        .push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let pat_imm = match &arm.pat {
                                        Pat::LitInt(n) => *n as i32,
                                        // FLS §5.2: Negative literal pattern.
                                        Pat::NegLitInt(n) => -(*n as i32),
                                        Pat::LitBool(b) => *b as i32,
                                        other => return Err(LowerError::Unsupported(format!("unsupported literal pattern kind: {other:?}"))),
                                    };
                                    let pat_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(pat_reg, pat_imm));
                                    // Compare scrutinee == pattern → 1 if equal, 0 if not.
                                    let cmp_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: cmp_reg,
                                        lhs: s_reg,
                                        rhs: pat_reg,
                                    });
                                    // cbz: skip this arm if condition is 0 (not equal).
                                    self.instrs.push(Instr::CondBranch {
                                        reg: cmp_reg,
                                        label: next_label,
                                    });
                                }
                            }

                            // Guard check (if any): emitted after the pattern check.
                            // The pattern check already branched to next_label on failure;
                            // if we reach here, the pattern matched. Now check the guard.
                            //
                            // FLS §6.18: "A match arm guard is an additional condition
                            // that must hold for the arm to be selected."
                            // FLS §6.1.2:37–45: Guard evaluation emits runtime instructions.
                            // Cache-line note: guard check adds 1 CondBranch instruction (4 bytes).
                            if let Some(guard) = &arm.guard {
                                let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                let gr = self.val_to_reg(gv)?;
                                self.instrs.push(Instr::CondBranch {
                                    reg: gr,
                                    label: next_label,
                                });
                            }

                            // Arm body (reached when pattern matched AND guard passed).
                            let body_val = self.lower_expr(&arm.body, ret_ty)?;
                            let body_reg = self.val_to_reg(body_val)?;
                            self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });
                            self.instrs.push(Instr::Branch(exit_label));

                            self.instrs.push(Instr::Label(next_label));
                        }

                        // Default arm — unconditional (guard on last arm is not yet supported).
                        // FLS §6.18 AMBIGUOUS: If the last arm has a guard that fails, the
                        // match is non-exhaustive. Galvanic defers this check to a future
                        // exhaustiveness pass.
                        if default_arm[0].guard.is_some() {
                            return Err(LowerError::Unsupported(
                                "guard on last match arm (exhaustiveness deferred; use `_` without guard as last arm)".into(),
                            ));
                        }

                        // FLS §5.1.4: If the default arm has an identifier pattern,
                        // bind the scrutinee to the name before lowering the body.
                        // FLS §5.4 + §15: If the default arm has a TupleStruct pattern,
                        // install field bindings from enum_base_slot.
                        let default_bindings: Vec<&str> = match &default_arm[0].pat {
                            Pat::Ident(span) => {
                                let name = span.text(self.source);
                                let bind_slot = self.alloc_slot()?;
                                let bind_reg = self.alloc_reg()?;
                                self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                                self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                                self.locals.insert(name, bind_slot);
                                vec![name]
                            }
                            Pat::TupleStruct { path: segs, fields } if segs.len() == 2 => {
                                let enum_name_def = segs[0].text(self.source);
                                let variant_name_def = segs[1].text(self.source);
                                let base = enum_base_slot.ok_or_else(|| LowerError::Unsupported(
                                    "TupleStruct default arm requires enum variable scrutinee".into(),
                                ))?;
                                let mut names = Vec::new();
                                // FLS §4.2, §15: Float fields use LoadF64Slot/LoadF32Slot.
                                // FLS §4.2: Register f64/f32 bindings in float_locals/float32_locals.
                                for (fi, fp) in fields.iter().enumerate() {
                                    if let Pat::Ident(span) = fp {
                                        let fname = span.text(self.source);
                                        let fslot = base + 1 + fi as u8;
                                        let bslot = self.alloc_slot()?;
                                        match self.enum_variant_field_float_ty(enum_name_def, variant_name_def, fi) {
                                            Some(IrTy::F64) => {
                                                let freg = self.alloc_reg()?;
                                                self.instrs.push(Instr::LoadF64Slot { dst: freg, slot: fslot });
                                                self.instrs.push(Instr::StoreF64 { src: freg, slot: bslot });
                                                self.slot_float_ty.insert(bslot, IrTy::F64);
                                                self.float_locals.insert(fname, bslot);
                                            }
                                            Some(IrTy::F32) => {
                                                let freg = self.alloc_reg()?;
                                                self.instrs.push(Instr::LoadF32Slot { dst: freg, slot: fslot });
                                                self.instrs.push(Instr::StoreF32 { src: freg, slot: bslot });
                                                self.slot_float_ty.insert(bslot, IrTy::F32);
                                                self.float32_locals.insert(fname, bslot);
                                            }
                                            _ => {
                                                let breg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                            }
                                        }
                                        self.locals.insert(fname, bslot);
                                        names.push(fname);
                                    }
                                }
                                names
                            }
                            // FLS §5.3 + §15.3: Named-field struct or variant default arm
                            // (value-returning match).
                            // One-segment: plain struct pattern (irrefutable, fields at base + idx).
                            // Two-segment: enum variant pattern (fields at enum_base + 1 + idx).
                            Pat::StructVariant { path: segs, fields: pat_fields } => {
                                let mut names: Vec<&str> = Vec::new();
                                if segs.len() == 1 {
                                    let struct_name = segs[0].text(self.source);
                                    let base = struct_base_slot.ok_or_else(|| LowerError::Unsupported(
                                        "plain struct default-arm pattern requires struct variable scrutinee".into(),
                                    ))?;
                                    let field_names = self.struct_defs
                                        .get(struct_name)
                                        .cloned()
                                        .ok_or_else(|| LowerError::Unsupported(format!(
                                            "unknown struct `{struct_name}`"
                                        )))?;
                                    for (fname_span, fp) in pat_fields.iter() {
                                        let fname = fname_span.text(self.source);
                                        if let Pat::Ident(bind_span) = fp {
                                            let bind_name = bind_span.text(self.source);
                                            if let Some(idx) = field_names.iter().position(|n| n == fname) {
                                                let fslot = base + idx as u8;
                                                let bslot = self.alloc_slot()?;
                                                let breg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                self.locals.insert(bind_name, bslot);
                                                names.push(bind_name);
                                            }
                                        }
                                    }
                                } else if segs.len() == 2 {
                                    let enum_name = segs[0].text(self.source);
                                    let variant_name = segs[1].text(self.source);
                                    let field_names = self.enum_defs
                                        .get(enum_name)
                                        .and_then(|v| v.get(variant_name))
                                        .map(|(_, names)| names.clone())
                                        .unwrap_or_default();
                                    let base = enum_base_slot.ok_or_else(|| LowerError::Unsupported(
                                        "StructVariant default arm requires enum variable scrutinee".into(),
                                    ))?;
                                    for (fname_span, fp) in pat_fields.iter() {
                                        let fname = fname_span.text(self.source);
                                        if let Pat::Ident(bind_span) = fp {
                                            let bind_name = bind_span.text(self.source);
                                            if let Some(idx) = field_names.iter().position(|n| n == fname) {
                                                let fslot = base + 1 + idx as u8;
                                                let bslot = self.alloc_slot()?;
                                                let breg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                self.locals.insert(bind_name, bslot);
                                                names.push(bind_name);
                                            }
                                        }
                                    }
                                }
                                names
                            }
                            _ => vec![],
                        };
                        let body_val = self.lower_expr(&default_arm[0].body, ret_ty)?;
                        for name in &default_bindings {
                            self.locals.remove(*name);
                            self.float_locals.remove(*name);
                            self.float32_locals.remove(*name);
                        }
                        let body_reg = self.val_to_reg(body_val)?;
                        self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });

                        self.instrs.push(Instr::Label(exit_label));
                        let result_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: result_reg, slot: phi_slot });
                        Ok(IrValue::Reg(result_reg))
                    }

                    IrTy::Unit => {
                        for arm in checked_arms {
                            let next_label = self.alloc_label();

                            match &arm.pat {
                                Pat::Wildcard => {
                                    // FLS §6.18: Guard check before body execution.
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch {
                                            reg: gr,
                                            label: next_label,
                                        });
                                    }
                                    self.lower_expr(&arm.body, &IrTy::Unit)?;
                                    self.instrs.push(Instr::Branch(exit_label));
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.1.4: Identifier pattern — always matches, binds name.
                                // FLS §6.18: Guard (if present) may reference the bound name.
                                Pat::Ident(span) => {
                                    let name = span.text(self.source);
                                    let bind_slot = self.alloc_slot()?;
                                    let bind_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                                    self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                                    self.locals.insert(name, bind_slot);
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch {
                                            reg: gr,
                                            label: next_label,
                                        });
                                    }
                                    self.lower_expr(&arm.body, &IrTy::Unit)?;
                                    self.locals.remove(name);
                                    self.instrs.push(Instr::Branch(exit_label));
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.1.11: OR pattern — same accumulation strategy as
                                // the I32|Bool branch above, but for unit-result arms.
                                Pat::Or(alts) => {
                                    let matched_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(matched_reg, 0));
                                    for alt in alts {
                                        match alt {
                                            Pat::Wildcard => {
                                                self.instrs.push(Instr::LoadImm(matched_reg, 1));
                                                break;
                                            }
                                            Pat::Or(_) => {
                                                return Err(LowerError::Unsupported(
                                                    "nested OR patterns".into(),
                                                ));
                                            }
                                            Pat::Ident(_) => {
                                                return Err(LowerError::Unsupported(
                                                    "identifier pattern inside OR pattern".into(),
                                                ));
                                            }
                                            _ => {
                                                let alt_imm = self.pat_scalar_imm(alt)?;
                                                let si_reg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load {
                                                    dst: si_reg,
                                                    slot: scrut_slot,
                                                });
                                                let alt_reg = self.alloc_reg()?;
                                                self.instrs.push(Instr::LoadImm(alt_reg, alt_imm));
                                                let eq_reg = self.alloc_reg()?;
                                                self.instrs.push(Instr::BinOp {
                                                    op: IrBinOp::Eq,
                                                    dst: eq_reg,
                                                    lhs: si_reg,
                                                    rhs: alt_reg,
                                                });
                                                self.instrs.push(Instr::BinOp {
                                                    op: IrBinOp::BitOr,
                                                    dst: matched_reg,
                                                    lhs: matched_reg,
                                                    rhs: eq_reg,
                                                });
                                            }
                                        }
                                    }
                                    self.instrs.push(Instr::CondBranch {
                                        reg: matched_reg,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.1.9: Inclusive range pattern `lo..=hi` (unit branch).
                                Pat::RangeInclusive { lo, hi } => {
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let lo_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                                    let cmp1 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Ge,
                                        dst: cmp1,
                                        lhs: s_reg,
                                        rhs: lo_reg,
                                    });
                                    let hi_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                                    let cmp2 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Le,
                                        dst: cmp2,
                                        lhs: s_reg,
                                        rhs: hi_reg,
                                    });
                                    let matched = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::BitAnd,
                                        dst: matched,
                                        lhs: cmp1,
                                        rhs: cmp2,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: matched,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.1.9: Exclusive range pattern `lo..hi` (unit branch).
                                Pat::RangeExclusive { lo, hi } => {
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let lo_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                                    let cmp1 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Ge,
                                        dst: cmp1,
                                        lhs: s_reg,
                                        rhs: lo_reg,
                                    });
                                    let hi_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                                    let cmp2 = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Lt,
                                        dst: cmp2,
                                        lhs: s_reg,
                                        rhs: hi_reg,
                                    });
                                    let matched = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::BitAnd,
                                        dst: matched,
                                        lhs: cmp1,
                                        rhs: cmp2,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: matched,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.5 + §15: Path pattern in unit-return match arm.
                                Pat::Path(segs) => {
                                    let pat_imm = if segs.len() == 2 {
                                        let enum_name = segs[0].text(self.source);
                                        let variant_name = segs[1].text(self.source);
                                        self.enum_defs
                                            .get(enum_name)
                                            .and_then(|v| v.get(variant_name))
                                            .map(|(disc, _)| *disc)
                                            .ok_or_else(|| LowerError::Unsupported(format!(
                                                "unknown enum variant `{enum_name}::{variant_name}` in match pattern"
                                            )))?
                                    } else {
                                        return Err(LowerError::Unsupported(
                                            "path pattern must have exactly two segments".into(),
                                        ));
                                    };
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let pat_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(pat_reg, pat_imm));
                                    let cmp_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: cmp_reg,
                                        lhs: s_reg,
                                        rhs: pat_reg,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: cmp_reg,
                                        label: next_label,
                                    });
                                }
                                // FLS §5.4 + §15: Tuple struct/variant pattern (unit return).
                                Pat::TupleStruct { path: segs, fields } => {
                                    if segs.len() != 2 {
                                        return Err(LowerError::Unsupported(
                                            "tuple struct pattern path must have two segments".into(),
                                        ));
                                    }
                                    let enum_name = segs[0].text(self.source);
                                    let variant_name = segs[1].text(self.source);
                                    let discriminant = self.enum_defs
                                        .get(enum_name)
                                        .and_then(|v| v.get(variant_name))
                                        .map(|(disc, _)| *disc)
                                        .ok_or_else(|| LowerError::Unsupported(format!(
                                            "unknown enum variant `{enum_name}::{variant_name}`"
                                        )))?;
                                    let base = enum_base_slot.ok_or_else(|| LowerError::Unsupported(
                                        "TupleStruct pattern requires enum variable scrutinee".into(),
                                    ))?;
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let p_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                                    let cmp_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: cmp_reg,
                                        lhs: s_reg,
                                        rhs: p_reg,
                                    });
                                    self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: next_label });
                                    // Install field bindings.
                                    // FLS §4.2, §15: Float fields use LoadF64Slot/LoadF32Slot.
                                    // FLS §4.2: Register f64/f32 bindings in float_locals/float32_locals.
                                    let mut bound: Vec<&str> = Vec::new();
                                    for (fi, fp) in fields.iter().enumerate() {
                                        if let Pat::Ident(span) = fp {
                                            let fname = span.text(self.source);
                                            let fslot = base + 1 + fi as u8;
                                            let bslot = self.alloc_slot()?;
                                            match self.enum_variant_field_float_ty(enum_name, variant_name, fi) {
                                                Some(IrTy::F64) => {
                                                    let freg = self.alloc_reg()?;
                                                    self.instrs.push(Instr::LoadF64Slot { dst: freg, slot: fslot });
                                                    self.instrs.push(Instr::StoreF64 { src: freg, slot: bslot });
                                                    self.slot_float_ty.insert(bslot, IrTy::F64);
                                                    self.float_locals.insert(fname, bslot);
                                                }
                                                Some(IrTy::F32) => {
                                                    let freg = self.alloc_reg()?;
                                                    self.instrs.push(Instr::LoadF32Slot { dst: freg, slot: fslot });
                                                    self.instrs.push(Instr::StoreF32 { src: freg, slot: bslot });
                                                    self.slot_float_ty.insert(bslot, IrTy::F32);
                                                    self.float32_locals.insert(fname, bslot);
                                                }
                                                _ => {
                                                    let breg = self.alloc_reg()?;
                                                    self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                    self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                }
                                            }
                                            self.locals.insert(fname, bslot);
                                            bound.push(fname);
                                        } else if !matches!(fp, Pat::Wildcard) {
                                            return Err(LowerError::Unsupported(
                                                "only ident/wildcard fields in TupleStruct pattern".into(),
                                            ));
                                        }
                                    }
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                                    }
                                    self.lower_expr(&arm.body, &IrTy::Unit)?;
                                    self.instrs.push(Instr::Branch(exit_label));
                                    for name in &bound {
                                        self.locals.remove(*name);
                                        self.float_locals.remove(*name);
                                        self.float32_locals.remove(*name);
                                    }
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.3 + §15.3: Named-field struct or variant pattern
                                // (unit-return match). Same 1/2-segment dispatch as I32 branch.
                                Pat::StructVariant { path: segs, fields: pat_fields } => {
                                    let mut bound_sv: Vec<&str> = Vec::new();
                                    if segs.len() == 1 {
                                        // FLS §5.3: Plain struct pattern — irrefutable.
                                        let struct_name = segs[0].text(self.source);
                                        let base = struct_base_slot.ok_or_else(|| LowerError::Unsupported(
                                            "plain struct pattern requires struct variable scrutinee".into(),
                                        ))?;
                                        let field_names = self.struct_defs
                                            .get(struct_name)
                                            .cloned()
                                            .ok_or_else(|| LowerError::Unsupported(format!(
                                                "unknown struct `{struct_name}`"
                                            )))?;
                                        for (fname_span, fp) in pat_fields.iter() {
                                            let fname = fname_span.text(self.source);
                                            let field_idx = field_names.iter().position(|n| n == fname)
                                                .ok_or_else(|| LowerError::Unsupported(format!(
                                                    "struct `{struct_name}` has no field `{fname}`"
                                                )))?;
                                            match fp {
                                                Pat::Ident(bind_span) => {
                                                    let bind_name = bind_span.text(self.source);
                                                    let fslot = base + field_idx as u8;
                                                    let bslot = self.alloc_slot()?;
                                                    let breg = self.alloc_reg()?;
                                                    self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                    self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                    self.locals.insert(bind_name, bslot);
                                                    bound_sv.push(bind_name);
                                                }
                                                Pat::Wildcard => {}
                                                _ => return Err(LowerError::Unsupported(
                                                    "only ident/wildcard sub-patterns in struct patterns".into(),
                                                )),
                                            }
                                        }
                                    } else if segs.len() == 2 {
                                        let enum_name = segs[0].text(self.source);
                                        let variant_name = segs[1].text(self.source);
                                        let (discriminant, field_names) = self.enum_defs
                                            .get(enum_name)
                                            .and_then(|v| v.get(variant_name))
                                            .cloned()
                                            .ok_or_else(|| LowerError::Unsupported(format!(
                                                "unknown enum variant `{enum_name}::{variant_name}`"
                                            )))?;
                                        let base = enum_base_slot.ok_or_else(|| LowerError::Unsupported(
                                            "StructVariant pattern requires enum variable scrutinee".into(),
                                        ))?;
                                        let s_reg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                        let p_reg = self.alloc_reg()?;
                                        self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                                        let cmp_reg = self.alloc_reg()?;
                                        self.instrs.push(Instr::BinOp {
                                            op: IrBinOp::Eq,
                                            dst: cmp_reg,
                                            lhs: s_reg,
                                            rhs: p_reg,
                                        });
                                        self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: next_label });
                                        // FLS §4.2, §15: Float fields use float loads/stores.
                                        for (fname_span, fp) in pat_fields.iter() {
                                            let fname = fname_span.text(self.source);
                                            let field_idx = field_names.iter().position(|n| n == fname)
                                                .ok_or_else(|| LowerError::Unsupported(format!(
                                                    "enum variant `{enum_name}::{variant_name}` has no field `{fname}`"
                                                )))?;
                                            match fp {
                                                Pat::Ident(bind_span) => {
                                                    let bind_name = bind_span.text(self.source);
                                                    let fslot = base + 1 + field_idx as u8;
                                                    let bslot = self.alloc_slot()?;
                                                    match self.enum_variant_field_float_ty(enum_name, variant_name, field_idx) {
                                                        Some(IrTy::F64) => {
                                                            let freg = self.alloc_reg()?;
                                                            self.instrs.push(Instr::LoadF64Slot { dst: freg, slot: fslot });
                                                            self.instrs.push(Instr::StoreF64 { src: freg, slot: bslot });
                                                            self.slot_float_ty.insert(bslot, IrTy::F64);
                                                            self.float_locals.insert(bind_name, bslot);
                                                        }
                                                        Some(IrTy::F32) => {
                                                            let freg = self.alloc_reg()?;
                                                            self.instrs.push(Instr::LoadF32Slot { dst: freg, slot: fslot });
                                                            self.instrs.push(Instr::StoreF32 { src: freg, slot: bslot });
                                                            self.slot_float_ty.insert(bslot, IrTy::F32);
                                                            self.float32_locals.insert(bind_name, bslot);
                                                        }
                                                        _ => {
                                                            let breg = self.alloc_reg()?;
                                                            self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                            self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                        }
                                                    }
                                                    self.locals.insert(bind_name, bslot);
                                                    bound_sv.push(bind_name);
                                                }
                                                Pat::Wildcard => {}
                                                _ => return Err(LowerError::Unsupported(
                                                    "only ident/wildcard sub-patterns in StructVariant fields".into(),
                                                )),
                                            }
                                        }
                                    } else {
                                        return Err(LowerError::Unsupported(
                                            "struct/variant pattern path must have 1 or 2 segments".into(),
                                        ));
                                    }
                                    if let Some(guard) = &arm.guard {
                                        let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                        let gr = self.val_to_reg(gv)?;
                                        self.instrs.push(Instr::CondBranch { reg: gr, label: next_label });
                                    }
                                    self.lower_expr(&arm.body, &IrTy::Unit)?;
                                    self.instrs.push(Instr::Branch(exit_label));
                                    for name in &bound_sv { self.locals.remove(*name); }
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                _ => {
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs
                                        .push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let pat_imm = match &arm.pat {
                                        Pat::LitInt(n) => *n as i32,
                                        Pat::NegLitInt(n) => -(*n as i32),
                                        Pat::LitBool(b) => *b as i32,
                                        other => return Err(LowerError::Unsupported(format!("unsupported literal pattern kind: {other:?}"))),
                                    };
                                    let pat_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(pat_reg, pat_imm));
                                    let cmp_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: cmp_reg,
                                        lhs: s_reg,
                                        rhs: pat_reg,
                                    });
                                    self.instrs.push(Instr::CondBranch {
                                        reg: cmp_reg,
                                        label: next_label,
                                    });
                                }
                            }

                            // Guard check for fall-through patterns (unit result).
                            // FLS §6.18: Guard evaluated after pattern matches.
                            if let Some(guard) = &arm.guard {
                                let gv = self.lower_expr(guard, &IrTy::Bool)?;
                                let gr = self.val_to_reg(gv)?;
                                self.instrs.push(Instr::CondBranch {
                                    reg: gr,
                                    label: next_label,
                                });
                            }

                            self.lower_expr(&arm.body, &IrTy::Unit)?;
                            self.instrs.push(Instr::Branch(exit_label));
                            self.instrs.push(Instr::Label(next_label));
                        }

                        // Default arm — unconditional (guard on last arm not yet supported).
                        if default_arm[0].guard.is_some() {
                            return Err(LowerError::Unsupported(
                                "guard on last match arm (exhaustiveness deferred)".into(),
                            ));
                        }

                        // FLS §5.1.4: If the default arm has an identifier pattern, bind.
                        // FLS §5.4 + §15: TupleStruct default arm installs field bindings.
                        let default_bindings_unit: Vec<&str> = match &default_arm[0].pat {
                            Pat::Ident(span) => {
                                let name = span.text(self.source);
                                let bind_slot = self.alloc_slot()?;
                                let bind_reg = self.alloc_reg()?;
                                self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                                self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                                self.locals.insert(name, bind_slot);
                                vec![name]
                            }
                            Pat::TupleStruct { path: segs, fields } if segs.len() == 2 => {
                                let enum_name_def2 = segs[0].text(self.source);
                                let variant_name_def2 = segs[1].text(self.source);
                                let base = enum_base_slot.ok_or_else(|| LowerError::Unsupported(
                                    "TupleStruct default arm requires enum variable scrutinee".into(),
                                ))?;
                                let mut names = Vec::new();
                                // FLS §4.2, §15: Float fields use LoadF64Slot/LoadF32Slot.
                                // FLS §4.2: Register f64/f32 bindings in float_locals/float32_locals.
                                for (fi, fp) in fields.iter().enumerate() {
                                    if let Pat::Ident(span) = fp {
                                        let fname = span.text(self.source);
                                        let fslot = base + 1 + fi as u8;
                                        let bslot = self.alloc_slot()?;
                                        match self.enum_variant_field_float_ty(enum_name_def2, variant_name_def2, fi) {
                                            Some(IrTy::F64) => {
                                                let freg = self.alloc_reg()?;
                                                self.instrs.push(Instr::LoadF64Slot { dst: freg, slot: fslot });
                                                self.instrs.push(Instr::StoreF64 { src: freg, slot: bslot });
                                                self.slot_float_ty.insert(bslot, IrTy::F64);
                                                self.float_locals.insert(fname, bslot);
                                            }
                                            Some(IrTy::F32) => {
                                                let freg = self.alloc_reg()?;
                                                self.instrs.push(Instr::LoadF32Slot { dst: freg, slot: fslot });
                                                self.instrs.push(Instr::StoreF32 { src: freg, slot: bslot });
                                                self.slot_float_ty.insert(bslot, IrTy::F32);
                                                self.float32_locals.insert(fname, bslot);
                                            }
                                            _ => {
                                                let breg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                            }
                                        }
                                        self.locals.insert(fname, bslot);
                                        names.push(fname);
                                    }
                                }
                                names
                            }
                            // FLS §5.3 + §15.3: Named-field struct or variant default arm
                            // (unit-return match). 1-segment: plain struct; 2-segment: enum variant.
                            Pat::StructVariant { path: segs, fields: pat_fields } => {
                                let mut names: Vec<&str> = Vec::new();
                                if segs.len() == 1 {
                                    let struct_name = segs[0].text(self.source);
                                    let base = struct_base_slot.ok_or_else(|| LowerError::Unsupported(
                                        "plain struct default-arm pattern requires struct variable scrutinee".into(),
                                    ))?;
                                    let field_names = self.struct_defs
                                        .get(struct_name)
                                        .cloned()
                                        .ok_or_else(|| LowerError::Unsupported(format!(
                                            "unknown struct `{struct_name}`"
                                        )))?;
                                    for (fname_span, fp) in pat_fields.iter() {
                                        let fname = fname_span.text(self.source);
                                        if let Pat::Ident(bind_span) = fp {
                                            let bind_name = bind_span.text(self.source);
                                            if let Some(idx) = field_names.iter().position(|n| n == fname) {
                                                let fslot = base + idx as u8;
                                                let bslot = self.alloc_slot()?;
                                                let breg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                self.locals.insert(bind_name, bslot);
                                                names.push(bind_name);
                                            }
                                        }
                                    }
                                } else if segs.len() == 2 {
                                    let enum_name = segs[0].text(self.source);
                                    let variant_name = segs[1].text(self.source);
                                    let field_names = self.enum_defs
                                        .get(enum_name)
                                        .and_then(|v| v.get(variant_name))
                                        .map(|(_, names)| names.clone())
                                        .unwrap_or_default();
                                    let base = enum_base_slot.ok_or_else(|| LowerError::Unsupported(
                                        "StructVariant default arm requires enum variable scrutinee".into(),
                                    ))?;
                                    for (fname_span, fp) in pat_fields.iter() {
                                        let fname = fname_span.text(self.source);
                                        if let Pat::Ident(bind_span) = fp {
                                            let bind_name = bind_span.text(self.source);
                                            if let Some(idx) = field_names.iter().position(|n| n == fname) {
                                                let fslot = base + 1 + idx as u8;
                                                let bslot = self.alloc_slot()?;
                                                let breg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                                self.locals.insert(bind_name, bslot);
                                                names.push(bind_name);
                                            }
                                        }
                                    }
                                }
                                names
                            }
                            _ => vec![],
                        };
                        self.lower_expr(&default_arm[0].body, &IrTy::Unit)?;
                        for name in &default_bindings_unit {
                            self.locals.remove(*name);
                            self.float_locals.remove(*name);
                            self.float32_locals.remove(*name);
                        }
                        self.instrs.push(Instr::Label(exit_label));
                        Ok(IrValue::Unit)
                    }
                    // FLS §4.2: Match expressions producing float types are not yet
                    // supported at this milestone.
                    IrTy::F64 | IrTy::F32 => Err(LowerError::Unsupported(
                        "match expression producing float type not yet supported (FLS §4.2)".into(),
                    )),
                }
            }

            // FLS §6.12.1: Call expression — lower each argument, then emit
            // `Instr::Call`. The callee must be a simple path (function name).
            //
            // ARM64 ABI: arguments go into x0–x{n-1}. If the argument's current
            // virtual register happens to already be register i (the required
            // ARM64 slot for argument i), no move is needed — this is tracked
            // in the `args` field of `Instr::Call` and resolved in codegen.
            //
            // FLS §6.4:14: Arguments are evaluated left-to-right before the
            // call. The sequential lowering loop preserves this order because
            // `lower_expr` for each argument emits its instructions before
            // moving on to the next.
            //
            // Limitation: only direct (named) calls are supported; function
            // pointers and method calls are deferred to a future milestone.
            ExprKind::Call { callee, args } => {
                // FLS §4.9: If the callee is a single-segment path that names a
                // local variable holding a function pointer, emit CallIndirect
                // (`blr x9`) rather than a direct call (`bl {name}`).
                if let ExprKind::Path(segments) = &callee.kind
                    && segments.len() == 1
                {
                    let var_name = segments[0].text(self.source);
                    if let Some(&ptr_slot) = self.locals.get(var_name)
                        && self.local_fn_ptr_slots.contains(&ptr_slot)
                    {
                        // FLS §6.22: Load captured outer-scope variables and
                        // prepend them as hidden leading arguments before the
                        // explicit arguments. The hidden closure function receives
                        // captures in x0..x{k-1} and explicit args in x{k}..
                        let cap_slots: Vec<u8> = self.local_capture_args
                            .get(&ptr_slot)
                            .cloned()
                            .unwrap_or_default();
                        let n_caps = cap_slots.len();
                        let total_args = n_caps + args.len();
                        if total_args > 8 {
                            return Err(LowerError::Unsupported(format!(
                                "indirect call with {total_args} arguments (captures + explicit) \
                                 exceeds 8-register ARM64 window"
                            )));
                        }
                        // Load captured values first (they must stay stable while
                        // explicit args are evaluated, so load them before the
                        // explicit argument expressions).
                        let mut all_regs = Vec::with_capacity(total_args);
                        for cap_slot in cap_slots {
                            let r = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: r, slot: cap_slot });
                            all_regs.push(r);
                        }
                        // Lower explicit arguments left-to-right (FLS §6.4:14).
                        for arg in args {
                            let v = self.lower_expr(arg, ret_ty)?;
                            let r = self.val_to_reg(v)?;
                            all_regs.push(r);
                        }
                        let dst = self.alloc_reg()?;
                        self.has_calls = true;
                        self.instrs.push(Instr::CallIndirect { dst, ptr_slot, args: all_regs });
                        return Ok(IrValue::Reg(dst));
                    }
                }

                // Resolve the callee to a function name.
                // FLS §15: Two-segment path callees are enum tuple variant constructors.
                // As standalone expressions they are only supported inside let-binding
                // initializers (handled in lower_stmt). Other contexts are future work.
                let fn_name = match &callee.kind {
                    ExprKind::Path(segments) if segments.len() == 1 => {
                        segments[0].text(self.source).to_owned()
                    }
                    ExprKind::Path(segments) if segments.len() == 2 => {
                        let tn = segments[0].text(self.source);
                        let fn_seg = segments[1].text(self.source);
                        // Check for enum variant constructor first.
                        if self.enum_defs.get(tn).and_then(|v| v.get(fn_seg)).is_some() {
                            return Err(LowerError::Unsupported(format!(
                                "enum variant `{tn}::{fn_seg}(...)` as expression; use in `let` binding"
                            )));
                        }
                        let mangled = format!("{tn}__{fn_seg}");
                        // Struct-returning associated functions cannot be used as scalar
                        // expressions — the caller needs a destination slot.
                        if self.struct_return_fns.contains_key(&mangled) {
                            return Err(LowerError::Unsupported(format!(
                                "`{tn}::{fn_seg}` returns a struct; use it in a `let` binding"
                            )));
                        }
                        // FLS §10.1: Scalar-returning associated function call.
                        // Resolve to the mangled name `TypeName__fn_name`.
                        mangled
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "call expression with non-path callee".into(),
                        ));
                    }
                };

                if args.len() > 8 {
                    return Err(LowerError::Unsupported(
                        "call with more than 8 arguments (exceeds ARM64 register window)".into(),
                    ));
                }

                // Lower each argument to virtual registers, left-to-right.
                //
                // FLS §11 / §6.12.2: If an argument is a struct variable
                // (recorded in `local_struct_types`), it expands to N registers:
                // one per field in declaration order. This matches the struct
                // parameter calling convention in `lower_fn`.
                //
                // FLS §15: If an argument is an enum variable (recorded in
                // `local_enum_types`), it expands to multiple registers:
                // discriminant in the first, fields in subsequent registers.
                // This matches the enum parameter calling convention in `lower_fn`.
                //
                // FLS §6.1.2:37–45: All loads are runtime instructions.
                let mut arg_regs = Vec::with_capacity(args.len());
                // FLS §4.2: Float arguments go in d0–d7; collected separately.
                let mut float_arg_regs: Vec<u8> = Vec::new();
                for arg in args {
                    // Check whether this argument is a struct variable.
                    //
                    // For nested structs, use the total slot count from `struct_sizes`
                    // (e.g., Rect with two Point fields has 4 total slots, not 2).
                    // This ensures the correct number of registers are loaded and
                    // passed to the callee, matching the parameter spill in `lower_fn`.
                    //
                    // FLS §4.11: Nested struct fields occupy consecutive slots in
                    // declaration order of the outermost struct.
                    // FLS §6.1.2:37–45: All loads are runtime instructions.
                    let struct_info: Option<(u8, usize)> = if let ExprKind::Path(segs) = &arg.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        if let Some(&base_slot) = self.locals.get(var_name) {
                            if let Some(st_name) = self.local_struct_types.get(&base_slot) {
                                // Use struct_sizes for total slot count (handles nested structs).
                                let n_slots = self.struct_sizes
                                    .get(st_name.as_str())
                                    .copied()
                                    .unwrap_or_else(|| {
                                        self.struct_defs
                                            .get(st_name.as_str())
                                            .map(|f| f.len())
                                            .unwrap_or(0)
                                    });
                                Some((base_slot, n_slots))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some((base_slot, n_fields)) = struct_info {
                        // Pass each struct field as a separate register.
                        // FLS §11: field 0 → x{i}, field 1 → x{i+1}, etc.
                        for fi in 0..n_fields {
                            let field_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load {
                                dst: field_reg,
                                slot: base_slot + fi as u8,
                            });
                            arg_regs.push(field_reg);
                        }
                        // Unit struct: no registers to pass; continue.
                        continue;
                    }

                    // Check whether this argument is an enum variable.
                    let enum_info: Option<(u8, usize)> = if let ExprKind::Path(segs) = &arg.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        let base_slot_opt = self.locals.get(var_name).copied();
                        if let Some(base_slot) = base_slot_opt {
                            if let Some(en) = self.local_enum_types.get(&base_slot) {
                                let en_name = en.clone();
                                let max_fields = self.enum_defs
                                    .get(en_name.as_str())
                                    .map(|v| v.values().map(|(_, names)| names.len()).max().unwrap_or(0))
                                    .unwrap_or(0);
                                Some((base_slot, max_fields))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some((base_slot, max_fields)) = enum_info {
                        // Pass discriminant register.
                        let disc_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: disc_reg, slot: base_slot });
                        arg_regs.push(disc_reg);
                        // Pass field registers (uninitialized for unit variants, but
                        // discriminant check in callee prevents any use of them).
                        for fi in 0..max_fields {
                            let field_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load {
                                dst: field_reg,
                                slot: base_slot + 1 + fi as u8,
                            });
                            arg_regs.push(field_reg);
                        }
                    } else if let ExprKind::Call { callee, args: variant_args } = &arg.kind
                        && let ExprKind::Path(segs) = &callee.kind
                        && segs.len() == 2
                    {
                        // FLS §15: Enum tuple variant constructor used directly as
                        // a function argument — e.g., `compute(Shape::Circle(7))`.
                        //
                        // Inline-construct the enum value into registers without
                        // allocating a named variable. Emit the discriminant as an
                        // immediate, then each field expression, then zero-pad to
                        // the enum's max_fields count so the callee's slot layout
                        // matches regardless of which variant is passed.
                        //
                        // FLS §6.1.2:37–45: All instructions emitted at runtime.
                        let tn = segs[0].text(self.source);
                        let vn = segs[1].text(self.source);
                        if let Some((discriminant, field_names)) = self.enum_defs
                            .get(tn)
                            .and_then(|v| v.get(vn))
                            .cloned()
                        {
                            let field_count = field_names.len();
                            let max_fields_enum = self.enum_defs
                                .get(tn)
                                .map(|v| v.values().map(|(_, n)| n.len()).max().unwrap_or(0))
                                .unwrap_or(0);
                            // Discriminant register.
                            let disc_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(disc_reg, discriminant));
                            arg_regs.push(disc_reg);
                            // Field registers.
                            for variant_arg in variant_args.iter() {
                                let val = self.lower_expr(variant_arg, &IrTy::I32)?;
                                let reg = self.val_to_reg(val)?;
                                arg_regs.push(reg);
                            }
                            // Padding for unused field slots.
                            for _ in field_count..max_fields_enum {
                                let pad_reg = self.alloc_reg()?;
                                self.instrs.push(Instr::LoadImm(pad_reg, 0));
                                arg_regs.push(pad_reg);
                            }
                        } else {
                            let val = self.lower_expr(arg, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }
                    } else if let ExprKind::Call { callee: ctor_callee, args: ctor_args } = &arg.kind
                        && let ExprKind::Path(ctor_segs) = &ctor_callee.kind
                        && ctor_segs.len() == 1
                        && let Some(&n_fields) = self.tuple_struct_defs.get(ctor_segs[0].text(self.source))
                    {
                        // FLS §14.2 + §6.12.1: Tuple struct constructor used directly as a
                        // function argument — e.g., `sum(Point(5, 8))`.
                        //
                        // Inline-evaluate each positional argument and pass the results as
                        // separate registers. This mirrors the tuple struct parameter calling
                        // convention set up in `lower_fn` (field 0 → x{i}, field 1 → x{i+1},
                        // …) without allocating a named slot or emitting a `bl` to a
                        // (non-existent) constructor function.
                        //
                        // FLS §6.1.2:37–45: All evaluations emit runtime instructions.
                        // Cache-line note: N field registers = N × 4-byte load/mov instructions.
                        let ctor_name = ctor_segs[0].text(self.source);
                        if ctor_args.len() != n_fields {
                            return Err(LowerError::Unsupported(format!(
                                "constructor `{ctor_name}` called with {} args but expects {n_fields}",
                                ctor_args.len()
                            )));
                        }
                        // FLS §4.2: f64/f32 fields go in float_arg_regs (d/s registers).
                        let float_field_tys = self
                            .tuple_struct_float_field_types
                            .get(ctor_name)
                            .cloned()
                            .unwrap_or_default();
                        for (i, ctor_arg) in ctor_args.iter().enumerate() {
                            match float_field_tys.get(i).copied().flatten() {
                                Some(IrTy::F64) => {
                                    let val = self.lower_expr(ctor_arg, &IrTy::F64)?;
                                    match val {
                                        IrValue::FReg(r) => float_arg_regs.push(r),
                                        _ => return Err(LowerError::Unsupported(
                                            "f64 tuple struct ctor arg did not produce float".into(),
                                        )),
                                    }
                                }
                                Some(IrTy::F32) => {
                                    let val = self.lower_expr(ctor_arg, &IrTy::F32)?;
                                    match val {
                                        IrValue::F32Reg(r) => float_arg_regs.push(r),
                                        _ => return Err(LowerError::Unsupported(
                                            "f32 tuple struct ctor arg did not produce float".into(),
                                        )),
                                    }
                                }
                                _ => {
                                    let val = self.lower_expr(ctor_arg, &IrTy::I32)?;
                                    let reg = self.val_to_reg(val)?;
                                    arg_regs.push(reg);
                                }
                            }
                        }
                    } else if let ExprKind::StructLit {
                        name: struct_name_span,
                        ..
                    } = &arg.kind
                    {
                        // FLS §6.11 + §11: Struct literal used directly as a function
                        // argument — e.g., `sum(Point { x: 3, y: 4 })` or a nested
                        // struct literal `f(Outer { x: 1, inner: Inner { a: 2 } })`.
                        //
                        // Uses `push_struct_lit_arg_regs` to recursively expand nested
                        // struct-type fields into their leaf registers, matching the
                        // N-slot calling convention established in `lower_fn`.
                        //
                        // FLS §6.11: Field initializers may appear in any source order;
                        // galvanic stores and passes them in struct declaration order.
                        // FLS §6.1.2:37–45: All field evaluations emit runtime instructions.
                        // Cache-line note: N leaf registers = N × 4-byte instructions,
                        // one per leaf field of the outermost struct.
                        let struct_name = struct_name_span.text(self.source).to_owned();
                        self.push_struct_lit_arg_regs(arg, &struct_name, &mut arg_regs)?;
                    } else if let ExprKind::Tuple(elements) = &arg.kind {
                        // FLS §5.10.3, §6.10, §9.2: Tuple literal used directly as a
                        // function argument — e.g., `sum_pair((3, 4))` or
                        // `sum3((1, (2, 3)))` (nested tuple).
                        //
                        // Each scalar leaf evaluates to one register, matching the
                        // tuple parameter calling convention in `lower_fn`. Nested
                        // tuples are flattened recursively: `(1, (2, 3))` produces
                        // three register arguments x0=1, x1=2, x2=3.
                        //
                        // FLS §6.1.2:37–45: All evaluations emit runtime instructions.
                        // Cache-line note: N leaves → N × 4-byte mov/ldr instructions.
                        self.push_tuple_lit_arg_regs(elements, &mut arg_regs)?;
                    } else if let ExprKind::Path(segs) = &arg.kind
                        && segs.len() == 1
                        && let Some(&base_slot) = self.locals.get(segs[0].text(self.source))
                        && let Some(&n_elems) = self.local_tuple_lens.get(&base_slot)
                    {
                        // FLS §5.10.3, §9.2: Tuple / tuple struct variable used as a
                        // function argument. Load each element from consecutive stack slots
                        // into registers, using float registers (d/s) for f64/f32 elements
                        // identified via slot_float_ty.
                        //
                        // FLS §4.2: float elements go in d0-d7; integer elements in x0-x7.
                        // FLS §6.1.2:37–45: All loads emit runtime instructions.
                        for i in 0..n_elems {
                            let slot = base_slot + i as u8;
                            match self.slot_float_ty.get(&slot).copied() {
                                Some(IrTy::F64) => {
                                    let reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadF64Slot { dst: reg, slot });
                                    float_arg_regs.push(reg);
                                }
                                Some(IrTy::F32) => {
                                    let reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadF32Slot { dst: reg, slot });
                                    float_arg_regs.push(reg);
                                }
                                _ => {
                                    let reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: reg, slot });
                                    arg_regs.push(reg);
                                }
                            }
                        }
                    } else if let ExprKind::Array(elems) = &arg.kind {
                        // FLS §6.8, §9.2: Array literal used directly as a function argument —
                        // e.g., `sum_arr([3, 7, 2, 8])`.
                        //
                        // Pass each element as a separate register in index order, matching
                        // the array parameter calling convention established in `lower_fn`
                        // for `arr: [T; N]` parameters (N consecutive registers x{i}..x{i+N-1}).
                        //
                        // FLS §6.1.2:37–45: All element evaluations emit runtime instructions.
                        // Cache-line note: N elements → N × 4-byte mov instructions.
                        for elem in elems.iter() {
                            let val = self.lower_expr(elem, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }
                    } else if let ExprKind::Path(segs) = &arg.kind
                        && segs.len() == 1
                        && let Some(&base_slot) = self.locals.get(segs[0].text(self.source))
                        && let Some(&n_elems) = self.local_array_lens.get(&base_slot)
                    {
                        // FLS §6.8, §9.2: Array variable used as a function argument —
                        // e.g., `let arr = [1, 2, 3]; sum_arr(arr)`.
                        //
                        // Load each element from consecutive stack slots and pass as
                        // separate registers, matching the `[T; N]` parameter convention.
                        //
                        // FLS §6.1.2:37–45: All loads emit runtime instructions.
                        // Cache-line note: N × 4-byte `ldr` instructions per argument.
                        for i in 0..n_elems {
                            let reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: reg, slot: base_slot + i as u8 });
                            arg_regs.push(reg);
                        }
                    } else if let ExprKind::Call { callee: inner_callee, args: inner_args } = &arg.kind
                        && let ExprKind::Path(inner_segs) = &inner_callee.kind
                        && inner_segs.len() == 1
                        && self.struct_return_free_fns.contains_key(inner_segs[0].text(self.source))
                    {
                        // FLS §9, §6.12.1: Struct-returning free function used directly as a
                        // function argument — e.g., `sum(make(1))` where `make` returns a struct.
                        //
                        // Problem: a plain `Instr::Call` captures only x0 (the scalar return
                        // register). After `bl make`, x0..x{N-1} hold the N struct fields
                        // (via `RetFields`). If we capture only x0 and then emit
                        // `mov x{dst}, x0`, that overwrites x1 (which held field[1]) before
                        // we can pass it to the outer call. The result is that the outer
                        // function receives x1 = field[0] instead of x1 = field[1].
                        //
                        // Fix: use `CallMut` to store all N return registers into temporary
                        // stack slots, then load them back as individual argument registers.
                        // This matches the `let p = make(1); sum(p)` path but without a
                        // named binding.
                        //
                        // FLS §6.1.2:37–45: All instructions are runtime.
                        // Cache-line note: CallMut emits bl + N stores; the subsequent N
                        // loads re-materialize the fields. For N=2: 4 instructions = 16 bytes.
                        let inner_fn_name = inner_segs[0].text(self.source);
                        let struct_name = self
                            .struct_return_free_fns
                            .get(inner_fn_name)
                            .cloned()
                            .unwrap();
                        let field_names = self
                            .struct_defs
                            .get(&struct_name)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "struct-returning arg: unknown struct `{struct_name}`"
                                ))
                            })?
                            .clone();
                        let n_fields = field_names.len();

                        // Allocate N temporary slots for the struct return value.
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..n_fields {
                            self.alloc_slot()?;
                        }

                        // Evaluate inner call arguments.
                        let mut inner_arg_regs: Vec<u8> = Vec::new();
                        for inner_arg in inner_args.iter() {
                            let val = self.lower_expr(inner_arg, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            inner_arg_regs.push(reg);
                        }

                        self.has_calls = true;
                        // Emit CallMut: bl inner_fn, then store x0..x{N-1} to temp slots.
                        self.instrs.push(Instr::CallMut {
                            name: inner_fn_name.to_owned(),
                            args: inner_arg_regs,
                            write_back_slot: base_slot,
                            n_fields: n_fields as u8,
                        });

                        // Load fields from temp slots as arguments to the outer call.
                        for fi in 0..n_fields {
                            let field_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load {
                                dst: field_reg,
                                slot: base_slot + fi as u8,
                            });
                            arg_regs.push(field_reg);
                        }
                    } else {
                        // FLS §4.2: Float arguments go in the float register bank.
                        // Lower the arg; if it produces a float value, add it to
                        // float_arg_regs instead of arg_regs.
                        let val = self.lower_expr(arg, &IrTy::I32)?;

                        // FLS §6.22, §4.13: If the arg was a capturing closure, we
                        // cannot pass the closure address directly because the callee
                        // (e.g., `apply(f: impl Fn, …)`) calls `f` with only the
                        // explicit arguments, unaware of the captured values.
                        //
                        // Fix: generate a trampoline that (1) has the correct arity
                        // for the `impl Fn` position, (2) reads captures from ARM64
                        // callee-saved registers x27/x26/… set by the caller before
                        // `bl apply`, and (3) tail-calls the actual closure.
                        //
                        // Cache-line note: trampoline is 3–6 instructions (12–24 bytes).
                        if let Some(cap_slots) = self.last_closure_captures.take() {
                            let closure_name = self.last_closure_name.take()
                                .expect("last_closure_name set when last_closure_captures is set");
                            let n_explicit = self.last_closure_n_explicit.take().unwrap_or(0);

                            // Emit loads of each capture into callee-saved registers
                            // x27 (cap 0), x26 (cap 1), x25 (cap 2), …
                            // These precede the Call instruction, so they execute
                            // before `bl apply` and are preserved through it.
                            for (cap_idx, &cap_slot) in cap_slots.iter().enumerate() {
                                let dest_reg = 27u8.saturating_sub(cap_idx as u8);
                                self.instrs.push(Instr::Load { dst: dest_reg, slot: cap_slot });
                            }

                            // Record the trampoline to be emitted.
                            let trampoline_name = format!("{closure_name}_trampoline");
                            self.pending_trampolines.push(crate::ir::ClosureTrampoline {
                                name: trampoline_name.clone(),
                                closure_name,
                                n_caps: cap_slots.len(),
                                n_explicit,
                            });

                            // Load the trampoline address instead of the closure address.
                            let tramp_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadFnAddr { dst: tramp_reg, name: trampoline_name });
                            arg_regs.push(tramp_reg);
                        } else {
                            match val {
                                IrValue::FReg(r) => float_arg_regs.push(r),
                                IrValue::F32Reg(r) => float_arg_regs.push(r),
                                _ => {
                                    let reg = self.val_to_reg(val)?;
                                    arg_regs.push(reg);
                                }
                            }
                        }
                    }
                }

                // Allocate the destination register for the return value.
                let dst = self.alloc_reg()?;

                // FLS §4.2: Determine whether the callee returns a float.
                // Float-returning functions place the result in d0 (f64) or s0 (f32)
                // rather than x0. The call site must capture from the correct register.
                let float_ret = if self.f64_return_fns.contains(&fn_name) {
                    Some(true)
                } else if self.f32_return_fns.contains(&fn_name) {
                    Some(false)
                } else {
                    None
                };

                self.has_calls = true;
                self.instrs.push(Instr::Call {
                    dst,
                    name: fn_name,
                    args: arg_regs,
                    float_args: float_arg_regs,
                    float_ret,
                });

                // FLS §4.2: Return value type depends on the callee's return type.
                match float_ret {
                    Some(true) => Ok(IrValue::FReg(dst)),
                    Some(false) => Ok(IrValue::F32Reg(dst)),
                    None => Ok(IrValue::Reg(dst)),
                }
            }

            // FLS §6.5.10: Assignment expression `place = value`.
            //
            // The LHS must be a local variable path (a place expression). The
            // RHS is evaluated at runtime and stored to the variable's stack
            // slot, updating its value in place for subsequent reads.
            //
            // FLS §6.5.10: "The type of an assignment expression is the unit
            // type ()."
            // FLS §6.1.2:37–45: The store is a runtime instruction; no
            // compile-time constant folding of the RHS is permitted.
            // FLS §14.1 AMBIGUOUS: The spec does not enumerate valid place
            // expressions for assignment; we restrict to simple variable paths.
            //
            // Cache-line note: the emitted `str` is 4 bytes — same footprint
            // as the `str` emitted by a let-binding initializer.
            ExprKind::Binary { op: BinOp::Assign, lhs, rhs } => {
                // FLS §4.2: Handle f64/f32 variable assignment as early returns.
                // f64/f32 locals are stored in `float_locals`/`float32_locals`,
                // not `locals`, so they must be dispatched before the general
                // slot-resolution block below.
                //
                // FLS §6.5.10: The assignment is a runtime store instruction
                // (FLS §6.1.2:37–45); no compile-time constant folding.
                if let ExprKind::Path(segments) = &lhs.kind
                    && segments.len() == 1
                {
                    let var_name = segments[0].text(self.source);
                    if let Some(&slot) = self.float_locals.get(var_name) {
                        // f64 assignment: lower RHS as F64, emit StoreF64.
                        let val = self.lower_expr(rhs, &IrTy::F64)?;
                        let src = match val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f64 assignment: RHS did not produce a float register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF64 { src, slot });
                        return Ok(IrValue::Unit);
                    }
                    if let Some(&slot) = self.float32_locals.get(var_name) {
                        // f32 assignment: lower RHS as F32, emit StoreF32.
                        let val = self.lower_expr(rhs, &IrTy::F32)?;
                        let src = match val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f32 assignment: RHS did not produce an f32 register".into(),
                            )),
                        };
                        self.instrs.push(Instr::StoreF32 { src, slot });
                        return Ok(IrValue::Unit);
                    }
                }

                // Resolve the LHS to a stack slot (must be a declared local or field).
                //
                // FLS §6.5.10: The left operand must be a place expression. Galvanic
                // supports two place expression forms here:
                //   1. A simple variable path (e.g., `x = value`)
                //   2. A struct field access (e.g., `s.field = value`) — FLS §6.13
                //
                // FLS §6.13: Field access on a struct variable resolves to the slot
                // at `base_slot + field_index`, where the layout mirrors struct literal
                // construction (field 0 at base_slot, field 1 at base_slot+1, etc.).
                //
                // Cache-line note: field store emits one `str` instruction (4 bytes),
                // identical in cost to a plain variable store.
                let slot = match &lhs.kind {
                    ExprKind::Path(segments) if segments.len() == 1 => {
                        let var_name = segments[0].text(self.source);
                        self.locals.get(var_name).copied().ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "assignment to undefined variable `{var_name}`"
                            ))
                        })?
                    }
                    ExprKind::FieldAccess { .. } => {
                        // FLS §6.5.10: The left operand must be a place expression.
                        // FLS §6.13: Struct field assignment — resolve using the same
                        // `resolve_place` helper as field read access, which supports
                        // chained access (`r.b.x = value`) via recursive slot computation.
                        //
                        // Cache-line note: field store emits one `str` instruction (4 bytes),
                        // identical in cost to a plain variable store.
                        let (slot, _) = self.resolve_place(lhs)?;
                        slot
                    }
                    // FLS §6.5.10: Assignment to an indexed place expression `arr[index] = value`.
                    // FLS §6.9: The base must be a known array variable; the index is a runtime value.
                    //
                    // Lowering strategy:
                    // 1. Resolve base to its array stack slot.
                    // 2. Lower index to a virtual register.
                    // 3. Lower RHS to a virtual register.
                    // 4. Allocate a scratch register for base address computation.
                    // 5. Emit `StoreIndexed { src, base_slot, index_reg, scratch }`.
                    //
                    // Evaluation order: FLS §6.5.10 evaluates the place (base, index)
                    // before the value, but since both are side-effect-free in the
                    // current subset, we lower RHS first to keep register allocation
                    // linear (index_reg < scratch so scratch doesn't alias either).
                    //
                    // FLS §14.1 AMBIGUOUS: The spec does not enumerate which place
                    // expressions are valid LHS for assignment. We restrict the base
                    // to a simple variable path (consistent with plain assignment).
                    ExprKind::Index { base, index } => {
                        // FLS §6.9: 2D index store `grid[i][j] = val` where `grid` is `[[T; M]; N]`.
                        //
                        // Detected when `base` is itself an index expression with a variable base.
                        // Linear slot = base_slot + i * M + j. Emits LoadImm + Mul + Add + StoreIndexed.
                        // FLS §6.1.2:37–45: All instructions are runtime.
                        if let ExprKind::Index { base: inner_base, index: i_expr } = &base.kind
                            && let ExprKind::Path(segs) = &inner_base.kind
                            && segs.len() == 1
                        {
                            let var_name = segs[0].text(self.source);
                            let base_slot =
                                self.locals.get(var_name).copied().ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "undefined variable `{var_name}` in 2D index assignment"
                                    ))
                                })?;
                            let inner_len =
                                *self.local_array_inner_lens.get(&base_slot).ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "variable `{var_name}` is not a 2D array"
                                    ))
                                })?;
                            // Lower RHS first.
                            let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                            let src_reg = self.val_to_reg(rhs_val)?;
                            // Lower indices.
                            let i_val = self.lower_expr(i_expr, &IrTy::I32)?;
                            let i_reg = self.val_to_reg(i_val)?;
                            let j_val = self.lower_expr(index, &IrTy::I32)?;
                            let j_reg = self.val_to_reg(j_val)?;
                            // Compute linear index.
                            let m_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(m_reg, inner_len as i32));
                            let prod_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp {
                                op: IrBinOp::Mul,
                                dst: prod_reg,
                                lhs: i_reg,
                                rhs: m_reg,
                            });
                            let linear_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp {
                                op: IrBinOp::Add,
                                dst: linear_reg,
                                lhs: prod_reg,
                                rhs: j_reg,
                            });
                            let scratch = self.alloc_reg()?;
                            self.instrs.push(Instr::StoreIndexed {
                                src: src_reg,
                                base_slot,
                                index_reg: linear_reg,
                                scratch,
                            });
                            return Ok(IrValue::Unit);
                        }

                        // Resolve base array variable to its stack slot.
                        let base_slot = match &base.kind {
                            ExprKind::Path(segs) if segs.len() == 1 => {
                                let var_name = segs[0].text(self.source);
                                let slot =
                                    self.locals.get(var_name).copied().ok_or_else(|| {
                                        LowerError::Unsupported(format!(
                                            "undefined variable `{var_name}` in index assignment"
                                        ))
                                    })?;
                                if !self.local_array_lens.contains_key(&slot) {
                                    return Err(LowerError::Unsupported(format!(
                                        "variable `{var_name}` is not an array (index assignment on non-arrays not supported)"
                                    )));
                                }
                                slot
                            }
                            _ => {
                                return Err(LowerError::Unsupported(
                                    "index assignment on non-variable base not yet supported"
                                        .into(),
                                ));
                            }
                        };

                        // Lower the RHS (value to store).
                        let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                        let src_reg = self.val_to_reg(rhs_val)?;

                        // Lower the index.
                        let idx_val = self.lower_expr(index, &IrTy::I32)?;
                        let index_reg = self.val_to_reg(idx_val)?;

                        // Allocate scratch for base-address computation.
                        let scratch = self.alloc_reg()?;

                        self.instrs.push(Instr::StoreIndexed {
                            src: src_reg,
                            base_slot,
                            index_reg,
                            scratch,
                        });

                        // FLS §6.5.10: assignment expressions have type `()`.
                        return Ok(IrValue::Unit);
                    }
                    // FLS §6.5.10: Assignment through a mutable reference `*ref_var = value`.
                    //
                    // When the LHS is a dereference expression, the operand must resolve to
                    // a register holding a pointer (an `&mut T` reference). The value is
                    // stored through that pointer using `StorePtr`.
                    //
                    // Lowering strategy:
                    // 1. Lower the pointer operand to a register (e.g., the `&mut` parameter).
                    // 2. Lower the RHS value to a register.
                    // 3. Emit `StorePtr { src: rhs_reg, addr: ptr_reg }`.
                    //
                    // FLS §6.1.2:37–45: Both the pointer computation and the store are
                    // runtime instructions; no compile-time evaluation is permitted.
                    //
                    // Cache-line note: one `str` instruction (4 bytes) — identical footprint
                    // to a plain variable store (`str xS, [sp, #offset]`).
                    ExprKind::Unary {
                        op: crate::ast::UnaryOp::Deref,
                        operand,
                    } => {
                        // Lower the pointer (e.g., the &mut parameter).
                        let ptr_val = self.lower_expr(operand, &IrTy::I32)?;
                        let ptr_reg = self.val_to_reg(ptr_val)?;

                        // Lower the RHS.
                        let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                        let src_reg = self.val_to_reg(rhs_val)?;

                        self.instrs.push(Instr::StorePtr { src: src_reg, addr: ptr_reg });

                        // FLS §6.5.10: assignment has type `()`.
                        return Ok(IrValue::Unit);
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "assignment to non-variable place expression not yet supported".into(),
                        ));
                    }
                };

                // FLS §9, §15: If the LHS is an enum variable and the RHS is a call
                // to an enum-returning free function, use CallMut-style write-back
                // to populate the enum variable's discriminant + field slots.
                if self.local_enum_types.contains_key(&slot)
                    && let ExprKind::Call { callee, args } = &rhs.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 1
                {
                    let fn_name = segs[0].text(self.source);
                    if let Some(enum_name) = self.enum_return_fns.get(fn_name).cloned() {
                        let max_fields = self.enum_defs
                            .get(enum_name.as_str())
                            .map(|v| v.values().map(|(_, names)| names.len()).max().unwrap_or(0))
                            .unwrap_or(0);
                        let n_ret = 1 + max_fields as u8;
                        // Lower arguments.
                        let mut arg_regs = Vec::with_capacity(args.len());
                        for arg in args.iter() {
                            let val = self.lower_expr(arg, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }
                        self.has_calls = true;
                        // CallMut: bl fn_name; str x0..[sp+slot*8..]
                        self.instrs.push(Instr::CallMut {
                            name: fn_name.to_owned(),
                            args: arg_regs,
                            write_back_slot: slot,
                            n_fields: n_ret,
                        });
                        return Ok(IrValue::Unit);
                    }
                }

                // Lower RHS as i32 — all current locals are i32.
                // FLS §8.1 AMBIGUOUS: The spec does not describe how type
                // inference constrains the RHS type at the assignment site
                // when no annotation is present. We assume i32 to match the
                // existing let-binding convention.
                let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                let src = self.val_to_reg(rhs_val)?;
                self.instrs.push(Instr::Store { src, slot });

                // FLS §6.5.10: assignment expressions have type `()`.
                Ok(IrValue::Unit)
            }

            // FLS §6.5.11: Compound assignment expression `target op= value`.
            //
            // Desugars at the IR level to: load target, binop, store target.
            // No new IR instructions needed — reuses Load, BinOp, Store.
            //
            // FLS §6.5.11: "The type of a compound assignment expression is the unit type ()."
            // FLS §6.1.2:37–45: The load, binop, and store must all emit runtime instructions —
            // even `x += 1` where x and 1 are statically known must emit ldr + add + str.
            //
            // FLS §14.1 AMBIGUOUS: The spec does not enumerate which place expressions are
            // valid on the left-hand side of compound assignment. We restrict to simple
            // variable paths, consistent with the restriction on plain assignment.
            //
            // Cache-line note: emits 3 instructions (ldr + binop + str) = 12 bytes.
            // For a simple add, this is three sequential 4-byte instructions that will
            // often land in the same 64-byte cache line.
            ExprKind::CompoundAssign { op, target, value } => {
                // Handle compound assignment through a pointer: `*ptr op= value`.
                //
                // FLS §6.5.11: The LHS of compound assignment may be a dereference
                // expression `*ptr`. Desugars to LoadPtr + BinOp + StorePtr.
                //
                // FLS §6.5.10: `*ptr = value` writes through the pointer; compound
                // assignment adds a LoadPtr before the store.
                //
                // FLS §6.1.2:37–45: All three instructions are runtime — no folding.
                //
                // Cache-line note: LoadPtr + BinOp + StorePtr = 3 instructions (12 bytes),
                // same footprint as the stack-slot variant (Load + BinOp + Store).
                if let ExprKind::Unary { op: crate::ast::UnaryOp::Deref, operand } = &target.kind {
                    // Lower the pointer operand to a register.
                    let ptr_val = self.lower_expr(operand, &IrTy::I32)?;
                    let ptr_reg = self.val_to_reg(ptr_val)?;

                    // Load current value through the pointer.
                    let lhs_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadPtr { dst: lhs_reg, src: ptr_reg });

                    // Lower the RHS.
                    let rhs_val = self.lower_expr(value, &IrTy::I32)?;
                    let rhs_reg = self.val_to_reg(rhs_val)?;

                    // Apply the binary operation.
                    let ir_op = match op {
                        BinOp::Add    => IrBinOp::Add,
                        BinOp::Sub    => IrBinOp::Sub,
                        BinOp::Mul    => IrBinOp::Mul,
                        BinOp::Div    => IrBinOp::Div,
                        BinOp::Rem    => IrBinOp::Rem,
                        BinOp::BitAnd => IrBinOp::BitAnd,
                        BinOp::BitOr  => IrBinOp::BitOr,
                        BinOp::BitXor => IrBinOp::BitXor,
                        BinOp::Shl    => IrBinOp::Shl,
                        BinOp::Shr    => IrBinOp::Shr,
                        _ => unreachable!("compound assignment operator must be arithmetic or bitwise"),
                    };
                    let res_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::BinOp { op: ir_op, dst: res_reg, lhs: lhs_reg, rhs: rhs_reg });

                    // Store the result back through the pointer.
                    self.instrs.push(Instr::StorePtr { src: res_reg, addr: ptr_reg });

                    return Ok(IrValue::Unit);
                }

                // FLS §6.5.11, §4.2: Float compound assignment — `x += 1.0` where `x: f64` or `x: f32`.
                //
                // Float locals live in `float_locals` (f64) / `float32_locals` (f32), not `locals`.
                // Must dispatch before the general integer slot-resolution block below.
                // Only arithmetic ops (+, -, *, /) are valid for floating-point types (FLS §6.5.5).
                // Bitwise ops and shifts are not defined for f64/f32 (FLS §6.5.6–§6.5.8).
                //
                // FLS §6.1.2:37–45: All three instructions (load + binop + store) are runtime.
                // ARM64 cache-line note: LoadF64Slot + F64BinOp + StoreF64 = 3 instructions (12 bytes).
                if let ExprKind::Path(segments) = &target.kind
                    && segments.len() == 1
                {
                    let var_name = segments[0].text(self.source);
                    if let Some(&slot) = self.float_locals.get(var_name) {
                        // f64 compound assignment: LoadF64Slot + F64BinOp + StoreF64.
                        let lhs_freg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadF64Slot { dst: lhs_freg, slot });
                        let rhs_val = self.lower_expr(value, &IrTy::F64)?;
                        let rhs_freg = match rhs_val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f64 compound assignment: RHS did not produce a float register".into(),
                            )),
                        };
                        let f_op = match op {
                            BinOp::Add => F64BinOp::Add,
                            BinOp::Sub => F64BinOp::Sub,
                            BinOp::Mul => F64BinOp::Mul,
                            BinOp::Div => F64BinOp::Div,
                            _ => return Err(LowerError::Unsupported(format!(
                                "operator `{op:?}` not defined for f64 (FLS §6.5.5)"
                            ))),
                        };
                        let dst_freg = self.alloc_reg()?;
                        self.instrs.push(Instr::F64BinOp { op: f_op, dst: dst_freg, lhs: lhs_freg, rhs: rhs_freg });
                        self.instrs.push(Instr::StoreF64 { src: dst_freg, slot });
                        return Ok(IrValue::Unit);
                    }
                    if let Some(&slot) = self.float32_locals.get(var_name) {
                        // f32 compound assignment: LoadF32Slot + F32BinOp + StoreF32.
                        let lhs_freg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadF32Slot { dst: lhs_freg, slot });
                        let rhs_val = self.lower_expr(value, &IrTy::F32)?;
                        let rhs_freg = match rhs_val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f32 compound assignment: RHS did not produce an f32 register".into(),
                            )),
                        };
                        let f_op = match op {
                            BinOp::Add => F32BinOp::Add,
                            BinOp::Sub => F32BinOp::Sub,
                            BinOp::Mul => F32BinOp::Mul,
                            BinOp::Div => F32BinOp::Div,
                            _ => return Err(LowerError::Unsupported(format!(
                                "operator `{op:?}` not defined for f32 (FLS §6.5.5)"
                            ))),
                        };
                        let dst_freg = self.alloc_reg()?;
                        self.instrs.push(Instr::F32BinOp { op: f_op, dst: dst_freg, lhs: lhs_freg, rhs: rhs_freg });
                        self.instrs.push(Instr::StoreF32 { src: dst_freg, slot });
                        return Ok(IrValue::Unit);
                    }
                }

                // Resolve target to a stack slot.
                //
                // FLS §6.5.11: Supports two place expression forms on the LHS:
                //   1. Simple variable path (`x += 1`)
                //   2. Struct field access (`self.n += 1`) — FLS §6.13
                //
                // FLS §10.1: `&mut self` field mutations via `self.n += 1` use
                // this field-access path to locate the field's stack slot.
                let slot = match &target.kind {
                    ExprKind::Path(segments) if segments.len() == 1 => {
                        let var_name = segments[0].text(self.source);
                        self.locals.get(var_name).copied().ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "compound assignment to undefined variable `{var_name}`"
                            ))
                        })?
                    }
                    ExprKind::FieldAccess { .. } => {
                        // FLS §6.5.11: Compound assignment on a struct field or chained
                        // field access — resolve using `resolve_place` which handles
                        // both `self.field += n` and `r.b.x += n` uniformly.
                        let (slot, _) = self.resolve_place(target)?;
                        slot
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "compound assignment to non-variable place expression not yet supported".into(),
                        ));
                    }
                };

                // Load current value of target at runtime.
                // FLS §6.1.2:37–45: this load is a runtime instruction.
                let lhs_reg = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: lhs_reg, slot });

                // Lower the RHS value expression.
                let rhs_val = self.lower_expr(value, &IrTy::I32)?;
                let rhs_reg = self.val_to_reg(rhs_val)?;

                // Map the compound operator to its IR binary op.
                let ir_op = match op {
                    BinOp::Add    => IrBinOp::Add,
                    BinOp::Sub    => IrBinOp::Sub,
                    BinOp::Mul    => IrBinOp::Mul,
                    BinOp::Div    => IrBinOp::Div,
                    BinOp::Rem    => IrBinOp::Rem,
                    BinOp::BitAnd => IrBinOp::BitAnd,
                    BinOp::BitOr  => IrBinOp::BitOr,
                    BinOp::BitXor => IrBinOp::BitXor,
                    BinOp::Shl    => IrBinOp::Shl,
                    BinOp::Shr    => IrBinOp::Shr,
                    _ => unreachable!(
                        "compound assignment operator must be arithmetic or bitwise (got {op:?})"
                    ),
                };
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::BinOp { op: ir_op, dst, lhs: lhs_reg, rhs: rhs_reg });

                // Store the result back to the same stack slot.
                // FLS §6.5.11: "Compound assignment is equivalent to the binary expression
                // followed by assignment." The store makes this observable to subsequent reads.
                self.instrs.push(Instr::Store { src: dst, slot });

                // FLS §6.5.11: compound assignment has type `()`.
                Ok(IrValue::Unit)
            }

            // FLS §6.5.3: Comparison operator expressions.
            //
            // Comparisons evaluate both operands (i32) at runtime and produce a
            // boolean result as 0 (false) or 1 (true). ARM64 codegen emits
            // `cmp x{lhs}, x{rhs}` followed by `cset x{dst}, <cond>` to
            // materialise the result into a register.
            //
            // FLS §6.1.2:37–45: Even statically-known comparisons emit runtime
            // instructions — `5 < 10` emits `cmp`+`cset`, not `mov x0, #1`.
            //
            // The result type is boolean (represented as i32: 0 or 1). This matches
            // the representation used by `CondBranch` (cbz tests for zero).
            ExprKind::Binary { op, lhs, rhs }
                if matches!(
                    op,
                    BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne
                ) =>
            {
                // FLS §6.5.3: Comparison operator expressions.
                //
                // Dispatch: if both operands are f64, emit FCmpF64 (fcmp + cset);
                // if both are f32, emit FCmpF32; otherwise treat as i32.
                //
                // FLS §6.5.3 AMBIGUOUS: The spec does not describe type-checking
                // rules for comparisons without type inference. Galvanic uses the
                // is_f64/f32_expr heuristic to classify operands.
                let fcmp_op = match op {
                    BinOp::Lt => Some(FCmpOp::Lt),
                    BinOp::Le => Some(FCmpOp::Le),
                    BinOp::Gt => Some(FCmpOp::Gt),
                    BinOp::Ge => Some(FCmpOp::Ge),
                    BinOp::Eq => Some(FCmpOp::Eq),
                    BinOp::Ne => Some(FCmpOp::Ne),
                    _ => None,
                };

                if let Some(fcmp) = fcmp_op {
                    if self.is_f64_expr(lhs) {
                        // FLS §6.5.3: f64 comparison → `fcmp d{lhs}, d{rhs}` + `cset`.
                        let lhs_val = self.lower_expr(lhs, &IrTy::F64)?;
                        let lhs_freg = match lhs_val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f64 comparison: expected float register for lhs".into(),
                            )),
                        };
                        let rhs_val = self.lower_expr(rhs, &IrTy::F64)?;
                        let rhs_freg = match rhs_val {
                            IrValue::FReg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f64 comparison: expected float register for rhs".into(),
                            )),
                        };
                        let dst = self.alloc_reg()?;
                        self.instrs.push(Instr::FCmpF64 { op: fcmp, dst, lhs: lhs_freg, rhs: rhs_freg });
                        return Ok(IrValue::Reg(dst));
                    }
                    if self.is_f32_expr(lhs) {
                        // FLS §6.5.3: f32 comparison → `fcmp s{lhs}, s{rhs}` + `cset`.
                        let lhs_val = self.lower_expr(lhs, &IrTy::F32)?;
                        let lhs_freg = match lhs_val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f32 comparison: expected f32 register for lhs".into(),
                            )),
                        };
                        let rhs_val = self.lower_expr(rhs, &IrTy::F32)?;
                        let rhs_freg = match rhs_val {
                            IrValue::F32Reg(r) => r,
                            _ => return Err(LowerError::Unsupported(
                                "f32 comparison: expected f32 register for rhs".into(),
                            )),
                        };
                        let dst = self.alloc_reg()?;
                        self.instrs.push(Instr::FCmpF32 { op: fcmp, dst, lhs: lhs_freg, rhs: rhs_freg });
                        return Ok(IrValue::Reg(dst));
                    }
                }

                // Integer comparison path.
                let lhs_val = self.lower_expr(lhs, &IrTy::I32)?;
                // Spill lhs register if rhs contains a call (ARM64 caller-save
                // convention; same rationale as arithmetic BinOp above).
                let lhs_spill: Option<u8> = if let IrValue::Reg(r) = lhs_val {
                    if expr_contains_call(rhs) {
                        let slot = self.alloc_slot()?;
                        self.instrs.push(Instr::Store { src: r, slot });
                        Some(slot)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                let lhs_val = if let Some(slot) = lhs_spill {
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst, slot });
                    IrValue::Reg(dst)
                } else {
                    lhs_val
                };
                let lhs_reg = self.val_to_reg(lhs_val)?;
                let rhs_reg = self.val_to_reg(rhs_val)?;
                let dst = self.alloc_reg()?;
                let ir_op = match op {
                    BinOp::Lt => IrBinOp::Lt,
                    BinOp::Le => IrBinOp::Le,
                    BinOp::Gt => IrBinOp::Gt,
                    BinOp::Ge => IrBinOp::Ge,
                    BinOp::Eq => IrBinOp::Eq,
                    BinOp::Ne => IrBinOp::Ne,
                    _ => unreachable!("matched above"),
                };
                self.instrs.push(Instr::BinOp { op: ir_op, dst, lhs: lhs_reg, rhs: rhs_reg });
                Ok(IrValue::Reg(dst))
            }

            // FLS §6.15.3: While loop expression.
            //
            // A while loop evaluates the condition before each iteration. If the
            // condition is true, the body executes; the loop then repeats. If the
            // condition is false, the loop terminates with value `()`.
            //
            // Lowering strategy (standard "loop with pre-test" pattern):
            //   1. Emit header label — the branch-back target for the loop back-edge.
            //   2. Lower condition → cond_reg.
            //   3. Emit CondBranch (cbz): jump to exit_label if cond_reg == 0 (false).
            //   4. Lower body block (statements; tail value discarded — type is `()`).
            //   5. Emit unconditional Branch back to header_label.
            //   6. Emit exit_label.
            //   7. Return IrValue::Unit (while loops always have type `()`).
            //
            // FLS §6.15.3: "A while expression has the unit type."
            // FLS §6.1.2:37–45: The condition is evaluated at runtime every iteration,
            // even when statically known — `while true { ... }` emits a `mov`+`cbz`.
            //
            // Cache-line note: the header and exit labels carry no instruction cost.
            // The back-edge `b .L{header}` is one 4-byte instruction — it fits in
            // the same cache line as the last instruction of the body.
            ExprKind::While { cond, body, label } => {
                let header_label = self.alloc_label();
                let exit_label = self.alloc_label();

                // Save the register watermark before entering the loop.
                // After the loop exits (exit_label), all registers allocated inside
                // the loop are dead — the loop returns unit and carries no value
                // forward. Resetting next_reg here allows subsequent code (e.g., the
                // function tail expression) to reuse those register numbers, keeping
                // the total register count well below x30 (the ARM64 link register).
                let reg_mark = self.next_reg;

                // Push a loop context so that `break`/`continue` inside the body
                // can resolve to the correct labels.
                // FLS §6.15.3, §6.15.6, §6.15.7.
                // `while` loops do not support break-with-value (FLS §6.15.6).
                self.loop_stack.push(LoopCtx { label: label.clone(), header_label, exit_label, break_slot: None, break_ret_ty: IrTy::Unit });

                // Loop top: the branch target for the back-edge.
                self.instrs.push(Instr::Label(header_label));

                // Evaluate condition as IrTy::Bool so that `!bool_var` used
                // as a while condition emits logical NOT (BoolNot/eor) rather
                // than bitwise NOT (Not/mvn). FLS §6.5.4.
                // FLS §6.15.3: the condition of a while expression must be bool.
                let cond_val = self.lower_expr(cond, &IrTy::Bool)?;
                let cond_reg = self.val_to_reg(cond_val)?;

                // cbz: exit if condition is false (0).
                self.instrs.push(Instr::CondBranch { reg: cond_reg, label: exit_label });

                // Execute body. The body has type `()` and its value is discarded.
                // FLS §6.15.3: "The block of the while loop is repeatedly executed
                // as long as the condition is true."
                self.lower_block_to_value(body, &IrTy::Unit)?;

                // Back-edge: unconditionally re-evaluate the condition.
                self.instrs.push(Instr::Branch(header_label));

                // Exit: the while expression produces `()`.
                self.instrs.push(Instr::Label(exit_label));

                self.loop_stack.pop();

                // Restore register watermark. Registers allocated inside the loop
                // are only referenced by instructions between header_label and the
                // back-edge Branch. After exit_label, none of them are live.
                self.next_reg = reg_mark;

                // FLS §6.15.3: "The type of a while expression is the unit type ()."
                Ok(IrValue::Unit)
            }

            // FLS §6.15.4: While-let loop expression.
            //
            // `while let Pattern = scrutinee { body }`
            //
            // FLS §6.15.4: "A while let loop expression is syntactic sugar for
            // a loop expression containing a match expression that breaks on
            // mismatch." Lowered directly to a header + pattern-check + body
            // structure for clarity and efficiency.
            //
            // Lowering strategy:
            //   1. Emit header_label (back-edge target for both continue and
            //      each new iteration).
            //   2. Lower scrutinee → spill to scrut_slot (re-evaluated each
            //      iteration, consistent with FLS §6.1.2:37–45).
            //   3. Pattern check: emit comparison and CondBranch to exit_label
            //      on no-match (same logic as IfLet pattern check).
            //   4. Install identifier binding (if any) before the body.
            //   5. Lower the body as unit.
            //   6. Remove binding (if any).
            //   7. Branch to header_label.
            //   8. Emit exit_label.
            //   9. Return Unit — FLS §6.15.4 says the loop type is `()`.
            //
            // `break` in body → exit_label (no break-with-value).
            // `continue` in body → header_label (re-evaluates scrutinee).
            //
            // FLS §6.15.6: while-let does not support break-with-value.
            // FLS §6.1.2:37–45: All checks emit runtime instructions.
            // Cache-line note: loop header costs ~2 instr (str scrut + pattern
            // cmp) plus body; back-edge is 1 branch (4 bytes).
            ExprKind::WhileLet { pat, scrutinee, body, label } => {
                let header_label = self.alloc_label();
                let exit_label = self.alloc_label();

                // FLS §15: If the scrutinee is a plain variable holding an enum
                // value, record its base slot for TupleStruct/StructVariant pattern
                // field access. The base slot is a compile-time constant; each
                // loop iteration loads field values from that fixed stack location.
                let enum_base_slot: Option<u8> =
                    if let ExprKind::Path(segs) = &scrutinee.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        self.locals
                            .get(var_name)
                            .copied()
                            .filter(|s| self.local_enum_types.contains_key(s))
                    } else {
                        None
                    };
                // FLS §5.3: Plain struct variable scrutinee for struct pattern.
                let struct_base_slot: Option<u8> =
                    if let ExprKind::Path(segs) = &scrutinee.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        self.locals
                            .get(var_name)
                            .copied()
                            .filter(|s| self.local_struct_types.contains_key(s))
                    } else {
                        None
                    };

                // Save register watermark — same rationale as While above.
                let reg_mark = self.next_reg;

                // Push loop context — while-let has no break-with-value.
                // FLS §6.15.4, §6.15.6.
                self.loop_stack.push(LoopCtx {
                    label: label.clone(),
                    header_label,
                    exit_label,
                    break_slot: None,
                    break_ret_ty: IrTy::Unit,
                });

                self.instrs.push(Instr::Label(header_label));

                // Infer scrutinee type from the pattern.
                let scrut_ty = match pat {
                    Pat::LitBool(_) => IrTy::Bool,
                    Pat::Or(alts) if alts.iter().any(|p| matches!(p, Pat::LitBool(_))) => {
                        IrTy::Bool
                    }
                    _ => IrTy::I32,
                };
                let scrut_val = self.lower_expr(scrutinee, &scrut_ty)?;
                let scrut_reg = self.val_to_reg(scrut_val)?;
                let scrut_slot = self.alloc_slot()?;
                self.instrs.push(Instr::Store { src: scrut_reg, slot: scrut_slot });

                // Bindings introduced by TupleStruct/StructVariant/Ident patterns;
                // all are removed before the back-edge (end of loop body).
                let mut bound_names: Vec<&str> = Vec::new();

                // Pattern check — branch to exit_label on mismatch.
                // Uses the same per-pattern logic as IfLet.
                // FLS §6.15.4: mismatch breaks out of the loop.
                match pat {
                    Pat::Wildcard | Pat::Ident(_) => {
                        // Always matches — no conditional branch.
                    }
                    Pat::LitInt(n) => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, *n as i32));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: exit_label });
                    }
                    Pat::NegLitInt(n) => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, -(*n as i32)));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: exit_label });
                    }
                    Pat::LitBool(b) => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, *b as i32));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: exit_label });
                    }
                    Pat::RangeInclusive { lo, hi } => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let lo_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                        let cmp1 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Ge,
                            dst: cmp1,
                            lhs: s_reg,
                            rhs: lo_reg,
                        });
                        let hi_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                        let cmp2 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Le,
                            dst: cmp2,
                            lhs: s_reg,
                            rhs: hi_reg,
                        });
                        let matched = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::BitAnd,
                            dst: matched,
                            lhs: cmp1,
                            rhs: cmp2,
                        });
                        self.instrs.push(Instr::CondBranch { reg: matched, label: exit_label });
                    }
                    Pat::RangeExclusive { lo, hi } => {
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let lo_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(lo_reg, *lo as i32));
                        let cmp1 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Ge,
                            dst: cmp1,
                            lhs: s_reg,
                            rhs: lo_reg,
                        });
                        let hi_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(hi_reg, *hi as i32));
                        let cmp2 = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Lt,
                            dst: cmp2,
                            lhs: s_reg,
                            rhs: hi_reg,
                        });
                        let matched = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::BitAnd,
                            dst: matched,
                            lhs: cmp1,
                            rhs: cmp2,
                        });
                        self.instrs.push(Instr::CondBranch { reg: matched, label: exit_label });
                    }
                    Pat::Or(alts) => {
                        let matched_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(matched_reg, 0));
                        for alt in alts {
                            match alt {
                                Pat::Wildcard => {
                                    self.instrs.push(Instr::LoadImm(matched_reg, 1));
                                    break;
                                }
                                Pat::Or(_) | Pat::Ident(_) => {
                                    return Err(LowerError::Unsupported(
                                        "nested OR or identifier inside while-let OR pattern".into(),
                                    ));
                                }
                                _ => {
                                    let alt_imm = self.pat_scalar_imm(alt)?;
                                    let si_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load {
                                        dst: si_reg,
                                        slot: scrut_slot,
                                    });
                                    let alt_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(alt_reg, alt_imm));
                                    let eq_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::Eq,
                                        dst: eq_reg,
                                        lhs: si_reg,
                                        rhs: alt_reg,
                                    });
                                    self.instrs.push(Instr::BinOp {
                                        op: IrBinOp::BitOr,
                                        dst: matched_reg,
                                        lhs: matched_reg,
                                        rhs: eq_reg,
                                    });
                                }
                            }
                        }
                        self.instrs.push(Instr::CondBranch {
                            reg: matched_reg,
                            label: exit_label,
                        });
                    }
                    // FLS §5.5 + §15: Path pattern in while-let — enum unit variant.
                    Pat::Path(segs) => {
                        let pat_imm = if segs.len() == 2 {
                            let enum_name = segs[0].text(self.source);
                            let variant_name = segs[1].text(self.source);
                            self.enum_defs
                                .get(enum_name)
                                .and_then(|v| v.get(variant_name))
                                .map(|(disc, _)| *disc)
                                .ok_or_else(|| LowerError::Unsupported(format!(
                                    "unknown enum variant `{enum_name}::{variant_name}` in while-let pattern"
                                )))?
                        } else {
                            return Err(LowerError::Unsupported(
                                "path pattern must have exactly two segments".into(),
                            ));
                        };
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, pat_imm));
                        let eq_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: eq_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: eq_reg, label: exit_label });
                    }
                    // FLS §5.4 + §15: TupleStruct pattern in while-let.
                    //
                    // `while let Enum::Variant(f0, f1, ..) = x { body }`
                    // Strategy: compare discriminant at scrut_slot against the variant's
                    // discriminant; branch to exit_label on mismatch; then install
                    // positional field bindings from enum_base_slot + 1 + idx.
                    // Bindings are removed before the back-edge each iteration.
                    //
                    // FLS §6.15.4: mismatch terminates the loop.
                    // FLS §6.1.2:37–45: All instructions are runtime.
                    // Cache-line note: ~5 + 2×N instructions per iteration header.
                    Pat::TupleStruct { path: segs, fields } => {
                        if segs.len() != 2 {
                            return Err(LowerError::Unsupported(
                                "tuple struct pattern path must have two segments".into(),
                            ));
                        }
                        let enum_name = segs[0].text(self.source);
                        let variant_name = segs[1].text(self.source);
                        let discriminant = self
                            .enum_defs
                            .get(enum_name)
                            .and_then(|v| v.get(variant_name))
                            .map(|(disc, _)| *disc)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown enum variant `{enum_name}::{variant_name}`"
                                ))
                            })?;
                        let base = enum_base_slot.ok_or_else(|| {
                            LowerError::Unsupported(
                                "TupleStruct pattern in while-let requires enum variable scrutinee"
                                    .into(),
                            )
                        })?;
                        // Discriminant check — branch to exit on mismatch.
                        let s_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                        let p_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                        let cmp_reg = self.alloc_reg()?;
                        self.instrs.push(Instr::BinOp {
                            op: IrBinOp::Eq,
                            dst: cmp_reg,
                            lhs: s_reg,
                            rhs: p_reg,
                        });
                        self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: exit_label });
                        // Install positional field bindings into bound_names.
                        // FLS §4.2, §15: Float fields use LoadF64Slot/LoadF32Slot.
                        // FLS §4.2: Register f64/f32 bindings in float_locals/float32_locals.
                        for (fi, fp) in fields.iter().enumerate() {
                            if let Pat::Ident(span) = fp {
                                let fname = span.text(self.source);
                                let fslot = base + 1 + fi as u8;
                                let bslot = self.alloc_slot()?;
                                match self.enum_variant_field_float_ty(enum_name, variant_name, fi) {
                                    Some(IrTy::F64) => {
                                        let freg = self.alloc_reg()?;
                                        self.instrs.push(Instr::LoadF64Slot { dst: freg, slot: fslot });
                                        self.instrs.push(Instr::StoreF64 { src: freg, slot: bslot });
                                        self.slot_float_ty.insert(bslot, IrTy::F64);
                                        self.float_locals.insert(fname, bslot);
                                    }
                                    Some(IrTy::F32) => {
                                        let freg = self.alloc_reg()?;
                                        self.instrs.push(Instr::LoadF32Slot { dst: freg, slot: fslot });
                                        self.instrs.push(Instr::StoreF32 { src: freg, slot: bslot });
                                        self.slot_float_ty.insert(bslot, IrTy::F32);
                                        self.float32_locals.insert(fname, bslot);
                                    }
                                    _ => {
                                        let breg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                        self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                    }
                                }
                                self.locals.insert(fname, bslot);
                                bound_names.push(fname);
                            } else if !matches!(fp, Pat::Wildcard) {
                                return Err(LowerError::Unsupported(
                                    "only ident/wildcard fields in TupleStruct while-let pattern"
                                        .into(),
                                ));
                            }
                        }
                    }
                    // FLS §5.3 + §15.3: Named-field struct or variant pattern in while-let.
                    // 1-segment: plain struct (irrefutable — loop runs forever unless broken).
                    // 2-segment: enum variant (terminates loop on discriminant mismatch).
                    Pat::StructVariant { path: segs, fields: pat_fields } => {
                        if segs.len() == 1 {
                            // FLS §5.3: Plain struct pattern — always matches.
                            let struct_name = segs[0].text(self.source);
                            let base = struct_base_slot.ok_or_else(|| {
                                LowerError::Unsupported(
                                    "plain struct pattern in while-let requires struct variable scrutinee".into(),
                                )
                            })?;
                            let field_names = self.struct_defs
                                .get(struct_name)
                                .cloned()
                                .ok_or_else(|| LowerError::Unsupported(format!(
                                    "unknown struct `{struct_name}`"
                                )))?;
                            // No CondBranch — plain struct patterns are irrefutable.
                            for (fname_span, fp) in pat_fields.iter() {
                                let fname = fname_span.text(self.source);
                                let field_idx = field_names.iter().position(|n| n == fname)
                                    .ok_or_else(|| LowerError::Unsupported(format!(
                                        "struct `{struct_name}` has no field `{fname}`"
                                    )))?;
                                match fp {
                                    Pat::Ident(bind_span) => {
                                        let bind_name = bind_span.text(self.source);
                                        let fslot = base + field_idx as u8;
                                        let bslot = self.alloc_slot()?;
                                        let breg = self.alloc_reg()?;
                                        self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                        self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                        self.locals.insert(bind_name, bslot);
                                        bound_names.push(bind_name);
                                    }
                                    Pat::Wildcard => {}
                                    _ => return Err(LowerError::Unsupported(
                                        "only ident/wildcard sub-patterns in struct while-let patterns".into(),
                                    )),
                                }
                            }
                        } else if segs.len() == 2 {
                            let enum_name = segs[0].text(self.source);
                            let variant_name = segs[1].text(self.source);
                            let (discriminant, field_names) = self
                                .enum_defs
                                .get(enum_name)
                                .and_then(|v| v.get(variant_name))
                                .cloned()
                                .ok_or_else(|| {
                                    LowerError::Unsupported(format!(
                                        "unknown enum variant `{enum_name}::{variant_name}`"
                                    ))
                                })?;
                            let base = enum_base_slot.ok_or_else(|| {
                                LowerError::Unsupported(
                                    "StructVariant pattern in while-let requires enum variable scrutinee"
                                        .into(),
                                )
                            })?;
                            // Discriminant check — branch to exit on mismatch.
                            let s_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::Load { dst: s_reg, slot: scrut_slot });
                            let p_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(p_reg, discriminant));
                            let cmp_reg = self.alloc_reg()?;
                            self.instrs.push(Instr::BinOp {
                                op: IrBinOp::Eq,
                                dst: cmp_reg,
                                lhs: s_reg,
                                rhs: p_reg,
                            });
                            self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: exit_label });
                            // Install named field bindings by declaration order.
                            // FLS §4.2, §15: Float fields use float loads/stores.
                            for (fname_span, fp) in pat_fields.iter() {
                                let fname = fname_span.text(self.source);
                                let field_idx = field_names
                                    .iter()
                                    .position(|n| n == fname)
                                    .ok_or_else(|| {
                                        LowerError::Unsupported(format!(
                                            "enum variant `{enum_name}::{variant_name}` has no field `{fname}`"
                                        ))
                                    })?;
                                match fp {
                                    Pat::Ident(bind_span) => {
                                        let bind_name = bind_span.text(self.source);
                                        let fslot = base + 1 + field_idx as u8;
                                        let bslot = self.alloc_slot()?;
                                        match self.enum_variant_field_float_ty(enum_name, variant_name, field_idx) {
                                            Some(IrTy::F64) => {
                                                let freg = self.alloc_reg()?;
                                                self.instrs.push(Instr::LoadF64Slot { dst: freg, slot: fslot });
                                                self.instrs.push(Instr::StoreF64 { src: freg, slot: bslot });
                                                self.slot_float_ty.insert(bslot, IrTy::F64);
                                                self.float_locals.insert(bind_name, bslot);
                                            }
                                            Some(IrTy::F32) => {
                                                let freg = self.alloc_reg()?;
                                                self.instrs.push(Instr::LoadF32Slot { dst: freg, slot: fslot });
                                                self.instrs.push(Instr::StoreF32 { src: freg, slot: bslot });
                                                self.slot_float_ty.insert(bslot, IrTy::F32);
                                                self.float32_locals.insert(bind_name, bslot);
                                            }
                                            _ => {
                                                let breg = self.alloc_reg()?;
                                                self.instrs.push(Instr::Load { dst: breg, slot: fslot });
                                                self.instrs.push(Instr::Store { src: breg, slot: bslot });
                                            }
                                        }
                                        self.locals.insert(bind_name, bslot);
                                        bound_names.push(bind_name);
                                    }
                                    Pat::Wildcard => {}
                                    _ => return Err(LowerError::Unsupported(
                                        "only ident/wildcard sub-patterns in StructVariant while-let fields".into(),
                                    )),
                                }
                            }
                        } else {
                            return Err(LowerError::Unsupported(
                                "struct/variant while-let pattern path must have 1 or 2 segments".into(),
                            ));
                        }
                    }
                    Pat::Tuple(_) => {
                        return Err(LowerError::Unsupported(
                            "tuple pattern in while-let not yet supported".into(),
                        ));
                    }
                    Pat::Slice(_) => {
                        return Err(LowerError::Unsupported(
                            "slice/array pattern in while-let not yet supported".into(),
                        ));
                    }
                }

                // Install identifier binding (if any) — in scope for body only.
                // FLS §5.1.4: identifier pattern binds the scrutinee.
                // TupleStruct/StructVariant field bindings were already pushed to
                // bound_names inside the pattern check above.
                if let Pat::Ident(span) = pat {
                    let name = span.text(self.source);
                    let bind_slot = self.alloc_slot()?;
                    let bind_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                    self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                    self.locals.insert(name, bind_slot);
                    bound_names.push(name);
                }

                // Execute body.
                self.lower_block_to_value(body, &IrTy::Unit)?;

                // Remove all bindings before back-edge.
                for name in &bound_names {
                    self.locals.remove(*name);
                    self.float_locals.remove(*name);
                    self.float32_locals.remove(*name);
                }

                // Back-edge: re-evaluate scrutinee and re-check pattern.
                self.instrs.push(Instr::Branch(header_label));

                self.instrs.push(Instr::Label(exit_label));
                self.loop_stack.pop();

                // Restore register watermark (see While above for rationale).
                self.next_reg = reg_mark;

                // FLS §6.15.4: "The type of a while let loop expression is the unit type ()."
                Ok(IrValue::Unit)
            }

            // FLS §6.15.1: For loop expression `for pat in start..end { body }`.
            //
            // Galvanic supports `for i in start..end` (exclusive integer range) and
            // `for i in start..=end` (inclusive integer range) as the first for-loop
            // milestone. The range iterator is desugared directly to a while-loop
            // equivalent — no library `IntoIterator` trait is involved.
            //
            // Lowering strategy:
            //   1. Lower `start` → store in a new stack slot for the loop variable.
            //   2. Lower `end` → store in a new stack slot for the end bound.
            //   3. Emit cond_label (back-edge target after increment).
            //   4. Load loop var, load end bound, compare (<  for exclusive, <= for inclusive).
            //   5. CondBranch to exit_label if condition is false (loop var >= end).
            //   6. Lower body (loop var visible in `locals`).
            //   7. emit incr_label (target for `continue`).
            //   8. Increment loop var slot by 1.
            //   9. Branch cond_label.
            //  10. Emit exit_label.
            //
            // `continue` in the body jumps to incr_label (increments then re-checks).
            // `break` in the body jumps to exit_label.
            //
            // FLS §6.15.1: "A for loop expression iterates over an iterator."
            // FLS §6.16: Range expressions produce iterators over integers.
            // FLS §6.1.2:37–45: The back-edge is a runtime branch — never elided.
            // FLS §6.15.7: `continue` advances to the next iteration (= increment step).
            //
            // Cache-line note: the loop generates ~7 instructions for the control
            // flow skeleton (load, cmp, cbz, load, add imm, str, b) — 28 bytes, fits
            // in one 32-byte half of a 64-byte instruction cache line.
            ExprKind::For { pat, iter, body, label } => {
                // FLS §6.15.1: for loops desugar via IntoIterator. Galvanic handles two
                // iterator kinds at the IR level (no runtime trait dispatch):
                //   1. Integer range (start..end / start..=end) — FLS §6.16
                //   2. Local array variable — FLS §6.8, §6.9
                //
                // Check for an array variable iterator FIRST.
                // `for x in arr` where `arr` is a local i32 array variable.
                // The loop desugars to a counted index loop: counter runs 0..arr_len,
                // binding each element to the loop variable on each iteration.
                //
                // FLS §6.15.1 AMBIGUOUS: The spec desugars `for x in arr` to
                // `IntoIterator::into_iter(arr)`, which requires trait dispatch.
                // Galvanic special-cases arrays at the IR level to avoid requiring
                // a runtime IntoIterator implementation at this milestone.
                //
                // Cache-line note: the array for loop emits ~9 instructions for the
                // control flow skeleton (load, loadimm, cmp, cbz, add+ldr, str, body,
                // load, loadimm, add, str, b) — within two 64-byte cache lines.
                let array_iter: Option<(u8, usize)> = if let ExprKind::Path(segs) = &iter.kind {
                    if segs.len() == 1 {
                        let vname = segs[0].text(self.source);
                        match self.locals.get(vname).copied() {
                            Some(base) => self.local_array_lens.get(&base).copied().map(|len| (base, len)),
                            None => None,
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some((arr_base, arr_len)) = array_iter {
                    let pat_name = pat.text(self.source);

                    // Allocate a hidden counter slot (0-based element index) and an
                    // element slot that holds the loop variable on each iteration.
                    let counter_slot = self.alloc_slot()?;
                    let elem_slot = self.alloc_slot()?;

                    // Save register watermark — same rationale as While / range For:
                    // registers allocated inside the loop body are temporaries that do not
                    // survive across iterations; restoring the watermark at loop exit lets
                    // subsequent code reuse those virtual register numbers.
                    let reg_mark = self.next_reg;

                    // Initialise counter to 0.
                    // FLS §6.15.1: The loop variable takes successive element values
                    // starting from the first.
                    let zero_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadImm(zero_reg, 0));
                    self.instrs.push(Instr::Store { src: zero_reg, slot: counter_slot });

                    // Labels: cond (condition check), incr (increment / continue target),
                    // exit (after loop / break target).
                    let cond_label = self.alloc_label();
                    let incr_label = self.alloc_label();
                    let exit_label = self.alloc_label();

                    // Push loop context for break / continue.
                    // FLS §6.15.7: `continue` in a for loop advances to the next element,
                    // i.e. increments the counter then re-checks the condition.
                    // FLS §6.15.6: `for` loops do not support break-with-value.
                    self.loop_stack.push(LoopCtx {
                        label: label.clone(),
                        header_label: incr_label,
                        exit_label,
                        break_slot: None,
                        break_ret_ty: IrTy::Unit,
                    });

                    // FLS §4.5, §4.2: Detect whether the array holds f64 or f32 elements.
                    // This drives the element load instruction and the loop variable
                    // registration (float_locals vs. locals).
                    let is_f64_arr_loop = self.local_f64_array_slots.contains(&arr_base);
                    let is_f32_arr_loop = self.local_f32_array_slots.contains(&arr_base);

                    // Bind the loop variable so the body can load it via Path.
                    // FLS §4.2: f64/f32 loop variables use float_locals so path
                    // expressions emit LoadF64Slot / LoadF32Slot instead of Load.
                    if is_f64_arr_loop {
                        self.float_locals.insert(pat_name, elem_slot);
                    } else if is_f32_arr_loop {
                        self.float32_locals.insert(pat_name, elem_slot);
                    } else {
                        self.locals.insert(pat_name, elem_slot);
                    }

                    // ── Condition: counter < arr_len ──────────────────────────────────
                    // FLS §6.15.1: The loop terminates when all elements have been visited.
                    self.instrs.push(Instr::Label(cond_label));
                    let counter_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: counter_reg, slot: counter_slot });
                    let len_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadImm(len_reg, arr_len as i32));
                    let cmp_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::BinOp { op: IrBinOp::Lt, dst: cmp_reg, lhs: counter_reg, rhs: len_reg });
                    self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: exit_label });

                    // ── Bind element: elem_slot = arr[counter] ────────────────────────
                    // FLS §6.9: Array indexing. The element at index `counter` is loaded
                    // from the consecutive stack slots of the array.
                    // FLS §4.2: Float arrays use d/s-registers for element loads.
                    let elem_reg = self.alloc_reg()?;
                    if is_f64_arr_loop {
                        // FLS §4.5: `[f64; N]` element — load into d-register, store as f64.
                        self.instrs.push(Instr::LoadIndexedF64 {
                            dst: elem_reg,
                            base_slot: arr_base,
                            index_reg: counter_reg,
                        });
                        self.instrs.push(Instr::StoreF64 { src: elem_reg, slot: elem_slot });
                    } else if is_f32_arr_loop {
                        // FLS §4.5: `[f32; N]` element — load into s-register, store as f32.
                        self.instrs.push(Instr::LoadIndexedF32 {
                            dst: elem_reg,
                            base_slot: arr_base,
                            index_reg: counter_reg,
                        });
                        self.instrs.push(Instr::StoreF32 { src: elem_reg, slot: elem_slot });
                    } else {
                        // Integer/boolean element — integer load and store.
                        self.instrs.push(Instr::LoadIndexed {
                            dst: elem_reg,
                            base_slot: arr_base,
                            index_reg: counter_reg,
                        });
                        self.instrs.push(Instr::Store { src: elem_reg, slot: elem_slot });
                    }

                    // ── Body ──────────────────────────────────────────────────────────
                    // FLS §6.15.1: "A for loop evaluates to the unit type."
                    self.lower_block_to_value(body, &IrTy::Unit)?;

                    // ── Increment: counter += 1 ───────────────────────────────────────
                    // This is the `continue` target. After incrementing, execution falls
                    // through to the back-edge Branch → cond_label.
                    // FLS §6.15.7: `continue` transfers control to the loop increment.
                    self.instrs.push(Instr::Label(incr_label));
                    let counter_reg2 = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: counter_reg2, slot: counter_slot });
                    let one_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadImm(one_reg, 1));
                    let next_counter = self.alloc_reg()?;
                    self.instrs.push(Instr::BinOp {
                        op: IrBinOp::Add,
                        dst: next_counter,
                        lhs: counter_reg2,
                        rhs: one_reg,
                    });
                    self.instrs.push(Instr::Store { src: next_counter, slot: counter_slot });

                    // Back-edge to condition.
                    self.instrs.push(Instr::Branch(cond_label));

                    // Exit label: loop done, restore register watermark.
                    self.instrs.push(Instr::Label(exit_label));
                    self.loop_stack.pop();
                    self.next_reg = reg_mark;

                    // FLS §6.15.1: "The type of a for loop expression is the unit type ()."
                    return Ok(IrValue::Unit);
                }

                // ── Range iterator (existing code) ────────────────────────────────────
                let (start_expr, end_expr, inclusive) = match iter.as_ref() {
                    Expr { kind: ExprKind::Range { start: Some(s), end: Some(e), inclusive }, .. } => {
                        (s.as_ref(), e.as_ref(), *inclusive)
                    }
                    _ => return Err(LowerError::Unsupported(
                        "for loop requires an integer range iterator (start..end or start..=end) or a local array variable".into(),
                    )),
                };

                // Allocate the loop variable slot and record the name.
                let i_slot = self.alloc_slot()?;
                let pat_name = pat.text(self.source);

                // Allocate a slot for the end bound (evaluated once before the loop).
                // FLS §6.16: The range bounds are evaluated once, not on each iteration.
                let end_slot = self.alloc_slot()?;

                // Save register watermark — same rationale as While above. Saved
                // BEFORE lowering start/end expressions so those temp registers are
                // also recyclable (they are consumed by Store and not live after it).
                let reg_mark = self.next_reg;

                // Lower and store the start bound into the loop variable slot.
                // FLS §6.16: `start` is evaluated first (left-to-right, FLS §6:3).
                let start_val = self.lower_expr(start_expr, &IrTy::I32)?;
                let start_reg = self.val_to_reg(start_val)?;
                self.instrs.push(Instr::Store { src: start_reg, slot: i_slot });

                // Lower and store the end bound.
                let end_val = self.lower_expr(end_expr, &IrTy::I32)?;
                let end_reg = self.val_to_reg(end_val)?;
                self.instrs.push(Instr::Store { src: end_reg, slot: end_slot });

                // Labels: cond (condition check / back-edge), incr (increment / continue
                // target), exit (after loop / break target).
                let cond_label = self.alloc_label();
                let incr_label = self.alloc_label();
                let exit_label = self.alloc_label();

                // Push loop context: `continue` → incr_label, `break` → exit_label.
                // FLS §6.15.7: `continue` in a for loop advances to the next iteration.
                // For a range-based for loop, the "next iteration" is the increment step.
                // `for` loops do not support break-with-value (FLS §6.15.6).
                self.loop_stack.push(LoopCtx { label: label.clone(), header_label: incr_label, exit_label, break_slot: None, break_ret_ty: IrTy::Unit });

                // Bind the loop variable name so body can load it via Path.
                self.locals.insert(pat_name, i_slot);

                // Condition check: load loop var and end bound, compare.
                self.instrs.push(Instr::Label(cond_label));
                let i_reg = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: i_reg, slot: i_slot });
                let end_reg2 = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: end_reg2, slot: end_slot });
                let cmp_reg = self.alloc_reg()?;
                // Exclusive `..`: continue while i < end (IrBinOp::Lt).
                // Inclusive `..=`: continue while i <= end (IrBinOp::Le).
                let cmp_op = if inclusive { IrBinOp::Le } else { IrBinOp::Lt };
                self.instrs.push(Instr::BinOp { op: cmp_op, dst: cmp_reg, lhs: i_reg, rhs: end_reg2 });
                // Exit if condition is false (i >= end for exclusive, i > end for inclusive).
                self.instrs.push(Instr::CondBranch { reg: cmp_reg, label: exit_label });

                // Lower body. The body result is discarded (for loops yield `()`).
                // FLS §6.15.1: "A for loop evaluates to the unit type."
                self.lower_block_to_value(body, &IrTy::Unit)?;

                // Increment step: i += 1. This is the `continue` target.
                // FLS §6.15.7: After a `continue`, the loop variable is incremented
                // before the condition is re-evaluated.
                self.instrs.push(Instr::Label(incr_label));
                let i_reg2 = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: i_reg2, slot: i_slot });
                let one_reg = self.alloc_reg()?;
                self.instrs.push(Instr::LoadImm(one_reg, 1));
                let inc_reg = self.alloc_reg()?;
                self.instrs.push(Instr::BinOp { op: IrBinOp::Add, dst: inc_reg, lhs: i_reg2, rhs: one_reg });
                self.instrs.push(Instr::Store { src: inc_reg, slot: i_slot });

                // Back-edge: jump to condition check.
                self.instrs.push(Instr::Branch(cond_label));

                // Exit label: loop done.
                self.instrs.push(Instr::Label(exit_label));

                self.loop_stack.pop();

                // Restore register watermark (see While above for rationale).
                self.next_reg = reg_mark;

                // FLS §6.15.1: "The type of a for loop expression is the unit type ()."
                Ok(IrValue::Unit)
            }

            // FLS §6.15.2: Infinite loop expression `loop { body }`.
            //
            // A loop expression repeatedly executes its body until a `break`
            // expression transfers control past the loop. Unlike `while`, there
            // is no condition — the only exit is an explicit `break`.
            //
            // Lowering strategy:
            //   1. Emit header label (back-edge target for continue / back-edge).
            //   2. Push LoopCtx so `break`/`continue` resolve to the right labels.
            //   3. Lower body block (unit type — value discarded).
            //   4. Emit unconditional back-edge Branch to header_label.
            //   5. Emit exit_label (where `break` branches to).
            //   6. Pop LoopCtx.
            //   7. Return Unit (loop without break value has type `()`).
            //
            // FLS §6.15.2: "A loop expression evaluates its block repeatedly
            // until a break expression is encountered."
            // FLS §6.15.2: "The type of a loop expression without a break value
            // is the unit type ()."
            // FLS §6.1.2:37–45: The back-edge is a runtime branch instruction
            // — it is not eliminated even if the body contains no side effects.
            //
            // Cache-line note: the header and exit labels have no instruction cost.
            // The back-edge `b .L{header}` is 4 bytes — one instruction slot.
            ExprKind::Loop { body, label } => {
                let header_label = self.alloc_label();
                let exit_label = self.alloc_label();

                // FLS §6.15.6: Only `loop` expressions support break-with-value.
                // Scan the body for `break <value>` at this loop level. If any
                // are present, allocate a stack slot to hold the result — the
                // same phi-slot pattern used for if-else expressions.
                //
                // Two sources of break-with-value:
                //   1. Unlabeled `break <value>` at this loop level (stops at
                //      nested loops — handled by `block_contains_break_with_value`).
                //   2. `break 'label <value>` targeting THIS loop's label from
                //      inside a nested loop — handled by
                //      `block_contains_labeled_break_with_value`.
                //
                // FLS §6.15.6: A labeled break exits the loop identified by the
                // label, regardless of nesting depth. We must check nested loops
                // to find such breaks.
                //
                // The break_slot is allocated BEFORE entering the loop body so
                // that `break <value>` can store into it during body lowering.
                let has_break_value = block_contains_break_with_value(body)
                    || label
                        .as_deref()
                        .is_some_and(|lbl| block_contains_labeled_break_with_value(body, lbl));
                let break_slot = if has_break_value {
                    Some(self.alloc_slot()?)
                } else {
                    None
                };

                // Save register watermark — same rationale as While above.
                // Saved AFTER break_slot allocation (a stack slot, not a register)
                // and BEFORE any register allocations inside the body.
                let reg_mark = self.next_reg;

                self.loop_stack.push(LoopCtx { label: label.clone(), header_label, exit_label, break_slot, break_ret_ty: *ret_ty });

                // Loop top: the branch-back target.
                self.instrs.push(Instr::Label(header_label));

                // Execute body. Value is discarded; only side effects matter.
                // The `break <value>` arms inside lower_expr will store to break_slot.
                self.lower_block_to_value(body, &IrTy::Unit)?;

                // Back-edge: jump unconditionally to the top of the loop.
                // FLS §6.15.2: execution continues indefinitely until `break`.
                self.instrs.push(Instr::Branch(header_label));

                // Exit: where `break` transfers control.
                self.instrs.push(Instr::Label(exit_label));

                self.loop_stack.pop();

                // Restore register watermark. Any registers used inside the loop body
                // are only live between header_label and the back-edge Branch. After
                // exit_label they are dead, so we can reuse their numbers. The result
                // register (if any) is freshly allocated below from the restored mark.
                self.next_reg = reg_mark;

                // FLS §6.15.2: "The type of a loop expression is determined by
                // its break expressions." If break-with-value was used, load the
                // result from the break slot. Otherwise the loop has type `()`.
                //
                // Cache-line note: the Load after the exit label is typically the
                // first instruction in a new cache line (the back-edge `b` and
                // the exit label occupy the end of the previous line). One Load
                // = 4 bytes.
                if let Some(slot) = break_slot {
                    let result_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: result_reg, slot });
                    Ok(IrValue::Reg(result_reg))
                } else {
                    Ok(IrValue::Unit)
                }
            }

            // FLS §6.15.6: Break expression — exit the innermost enclosing loop.
            //
            // An unqualified `break` jumps to the exit label of the innermost
            // loop. A `break value` expression would additionally store the value
            // to a break slot; break-with-value is not yet supported.
            //
            // FLS §6.15.6: "A break expression exits the innermost enclosing loop
            // expression or block expression labelled with a block label."
            // FLS §6.15.6: "The type of a break expression is the never type `!`."
            // We approximate `!` as Unit since the never type is not yet in the IR.
            //
            // FLS §6.1.2:37–45: The branch is a runtime `b` instruction — even if
            // the condition leading to `break` is statically known, the branch
            // resolves at runtime.
            //
            // Cache-line note: `b .L{exit}` is 4 bytes — one instruction slot.
            // FLS §6.15.6: Break expression — exit the innermost enclosing loop.
            //
            // Two forms:
            //   - `break` (no value): branch to exit_label.
            //   - `break <expr>` (with value): lower the value, store it to the
            //     loop's break_slot, then branch to exit_label. The slot was
            //     allocated by the enclosing `loop` lowering if any break-with-value
            //     was detected via `block_contains_break_with_value`.
            //
            // FLS §6.15.6: "Only `loop` expressions support break-with-value."
            // Attempting `break value` in `while` or `for` (which have
            // `break_slot: None`) produces an unsupported error — consistent
            // with the spec restriction.
            //
            // FLS §6.1.2:37–45: The branch is a runtime `b` instruction.
            // Cache-line note: `store + b` = 8 bytes — two instructions, fits
            // in one half of a 16-byte bundle.
            ExprKind::Break { label, value } => {
                // Resolve the exit label and optional break slot from the
                // target loop context.
                //
                // FLS §6.15.6: "A break expression exits the innermost
                // enclosing loop expression or block expression labelled
                // with a block label."
                //
                // If a label is present, search the loop_stack from top to
                // bottom for a LoopCtx whose label matches. Otherwise use the
                // innermost (top) context.
                let (exit_label, break_slot, break_ret_ty) = if let Some(lbl) = label {
                    // Labeled break: find matching loop on the stack.
                    self.loop_stack.iter().rev()
                        .find(|ctx| ctx.label.as_deref() == Some(lbl.as_str()))
                        .map(|ctx| (ctx.exit_label, ctx.break_slot, ctx.break_ret_ty))
                        .ok_or_else(|| LowerError::Unsupported(
                            format!("break label `'{lbl}` not found in enclosing loops")
                        ))?
                } else {
                    // Unlabeled break: use innermost loop.
                    self.loop_stack.last()
                        .map(|ctx| (ctx.exit_label, ctx.break_slot, ctx.break_ret_ty))
                        .ok_or_else(|| LowerError::Unsupported(
                            "break expression outside of a loop".into()
                        ))?
                };

                if let Some(val_expr) = value {
                    // break-with-value: store result to the break_slot, then jump.
                    // Use the loop's break_ret_ty (not the break statement's ret_ty,
                    // which is Unit in the body context) to lower the value correctly.
                    let slot = break_slot.ok_or_else(|| LowerError::Unsupported(
                        "break with value is only valid in a `loop` expression (FLS §6.15.6)".into(),
                    ))?;
                    let v = self.lower_expr(val_expr, &break_ret_ty)?;
                    let r = self.val_to_reg(v)?;
                    self.instrs.push(Instr::Store { src: r, slot });
                }

                // Emit the branch to the loop exit.
                self.instrs.push(Instr::Branch(exit_label));

                // FLS §6.15.6: break has type `!` (never). Approximated as Unit.
                Ok(IrValue::Unit)
            }

            // FLS §6.15.7: Continue expression — restart the target loop.
            //
            // A `continue` transfers control to the header of the innermost
            // enclosing loop (or the labeled loop if a label is given), skipping
            // any remaining statements in the body.
            //
            // FLS §6.15.7: "A continue expression advances to the next iteration
            // of the innermost enclosing loop expression."
            // FLS §6.15.7: "The type of a continue expression is the never type `!`."
            // We approximate `!` as Unit since the never type is not yet in the IR.
            //
            // Cache-line note: `b .L{header}` is 4 bytes — same cost as `break`.
            ExprKind::Continue { label } => {
                // Resolve the header label from the target loop context.
                let header_label = if let Some(lbl) = label {
                    // Labeled continue: find matching loop on the stack.
                    self.loop_stack.iter().rev()
                        .find(|ctx| ctx.label.as_deref() == Some(lbl.as_str()))
                        .map(|ctx| ctx.header_label)
                        .ok_or_else(|| LowerError::Unsupported(
                            format!("continue label `'{lbl}` not found in enclosing loops")
                        ))?
                } else {
                    self.loop_stack.last()
                        .map(|ctx| ctx.header_label)
                        .ok_or_else(|| LowerError::Unsupported(
                            "continue expression outside of a loop".into()
                        ))?
                };

                self.instrs.push(Instr::Branch(header_label));

                // FLS §6.15.7: continue has type `!` (never). Approximated as Unit.
                Ok(IrValue::Unit)
            }

            // FLS §6.19: Return expression — transfer control to the caller.
            //
            // A `return` expression exits the current function immediately,
            // yielding the given value (or unit if none) to the caller.
            //
            // Lowering strategy:
            //   1. If a value is present, lower it using the function's return
            //      type (`fn_ret_ty`), not the current expression context type.
            //      The function return type is stored in `LowerCtx::fn_ret_ty`
            //      precisely for this purpose.
            //   2. Emit `Instr::Ret` with the value.
            //   3. Return `IrValue::Unit` to the caller of `lower_expr` — any
            //      code after a `return` in the same block is unreachable but
            //      the surrounding block still lowers it. Dead instructions
            //      after `ret` are ignored by the assembler.
            //
            // FLS §6.19: "The type of a return expression is the never type `!`."
            // We approximate `!` as Unit since the never type is not yet in the IR.
            //
            // FLS §6.1.2:37–45: The `ret` is a runtime instruction — no constant
            // folding of the returned value.
            //
            // Cache-line note: a `return <literal>` emits one `LoadImm` (4 bytes)
            // + one `ret` (4 bytes) = two instructions, one half a cache line.
            ExprKind::Return(opt_val) => {
                let fn_ret_ty = self.fn_ret_ty;
                let ret_val = match opt_val {
                    Some(val) => {
                        let v = self.lower_expr(val, &fn_ret_ty)?;
                        let r = self.val_to_reg(v)?;
                        IrValue::Reg(r)
                    }
                    None => IrValue::Unit,
                };
                self.instrs.push(Instr::Ret(ret_val));
                // FLS §6.19: return has type `!` (never). Approximated as Unit.
                Ok(IrValue::Unit)
            }

            // FLS §6.5.8: Lazy boolean AND operator `&&`.
            //
            // Short-circuit semantics: the RHS is only evaluated if the LHS is true.
            // If the LHS evaluates to false (0), the RHS is not evaluated and the
            // result is false (0). If the LHS is true (1), the result is the RHS value.
            //
            // Equivalent to: if lhs { rhs } else { false }
            //
            // Lowering strategy (same phi-slot pattern as if/else):
            //   1. Lower LHS → lhs_reg.
            //   2. Allocate phi slot for the result.
            //   3. CondBranch (cbz): if lhs_reg == 0 (false), jump to .Lfalse.
            //   4. Lower RHS → rhs_reg. Store rhs_reg → phi slot. Branch to .Lend.
            //   5. .Lfalse: store 0 → phi slot.
            //   6. .Lend: load phi slot → result_reg. Return Reg(result_reg).
            //
            // FLS §6.5.8: "The right operand is only evaluated if the left operand
            // is true." The CondBranch skips the RHS evaluation entirely.
            //
            // FLS §6.1.2:37–45: Even statically-known `true && x` emits a runtime
            // `cbz` — no constant folding of the short-circuit condition.
            //
            // Cache-line note: the phi slot is one 8-byte stack entry, same as
            // the if/else phi slot. The short-circuit adds one `cbz` (4 bytes) to
            // the instruction stream.
            ExprKind::Binary { op: BinOp::And, lhs, rhs } => {
                let false_label = self.alloc_label();
                let end_label = self.alloc_label();
                // Allocate the phi slot before entering either branch.
                let phi_slot = self.alloc_slot()?;

                // Evaluate LHS. Booleans are represented as i32 (0 or 1).
                let lhs_val = self.lower_expr(lhs, &IrTy::I32)?;
                let lhs_reg = self.val_to_reg(lhs_val)?;

                // Short-circuit: if LHS is false (0), skip RHS entirely.
                self.instrs.push(Instr::CondBranch { reg: lhs_reg, label: false_label });

                // ── LHS is true: evaluate RHS and use it as the result ────────────
                let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                let rhs_reg = self.val_to_reg(rhs_val)?;
                self.instrs.push(Instr::Store { src: rhs_reg, slot: phi_slot });
                self.instrs.push(Instr::Branch(end_label));

                // ── LHS was false: result = 0 ─────────────────────────────────────
                self.instrs.push(Instr::Label(false_label));
                let zero_reg = self.alloc_reg()?;
                self.instrs.push(Instr::LoadImm(zero_reg, 0));
                self.instrs.push(Instr::Store { src: zero_reg, slot: phi_slot });

                // ── End: load and return result ───────────────────────────────────
                self.instrs.push(Instr::Label(end_label));
                let result_reg = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: result_reg, slot: phi_slot });
                Ok(IrValue::Reg(result_reg))
            }

            // FLS §6.5.8: Lazy boolean OR operator `||`.
            //
            // Short-circuit semantics: the RHS is only evaluated if the LHS is false.
            // If the LHS evaluates to true (non-zero), the RHS is not evaluated and
            // the result is the LHS value (which is 1 for a boolean). If the LHS is
            // false (0), the result is the RHS value.
            //
            // Equivalent to: if lhs { lhs } else { rhs }
            // Simplified (since lhs is 0/1): if lhs { 1 } else { rhs }
            //
            // Lowering strategy (cbz branches to .Lrhs when LHS is false):
            //   1. Lower LHS → lhs_reg.
            //   2. Allocate phi slot for the result.
            //   3. CondBranch (cbz): if lhs_reg == 0 (false), jump to .Lrhs.
            //   4. Store lhs_reg → phi slot (lhs is 1). Branch to .Lend.
            //   5. .Lrhs: Lower RHS → rhs_reg. Store rhs_reg → phi slot.
            //   6. .Lend: load phi slot → result_reg. Return Reg(result_reg).
            //
            // FLS §6.5.8: "The right operand is only evaluated if the left operand
            // is false." The CondBranch skips the RHS evaluation entirely when true.
            //
            // FLS §6.1.2:37–45: Even statically-known `false || x` emits a runtime
            // `cbz` — no constant folding of the short-circuit condition.
            //
            // Cache-line note: identical phi-slot footprint to `&&` lowering.
            ExprKind::Binary { op: BinOp::Or, lhs, rhs } => {
                let rhs_label = self.alloc_label();
                let end_label = self.alloc_label();
                // Allocate the phi slot before entering either branch.
                let phi_slot = self.alloc_slot()?;

                // Evaluate LHS. Booleans are represented as i32 (0 or 1).
                let lhs_val = self.lower_expr(lhs, &IrTy::I32)?;
                let lhs_reg = self.val_to_reg(lhs_val)?;

                // Short-circuit: if LHS is false (0), must evaluate RHS.
                self.instrs.push(Instr::CondBranch { reg: lhs_reg, label: rhs_label });

                // ── LHS is true: result = LHS (which is 1) ───────────────────────
                self.instrs.push(Instr::Store { src: lhs_reg, slot: phi_slot });
                self.instrs.push(Instr::Branch(end_label));

                // ── LHS was false: evaluate RHS and use it as result ──────────────
                self.instrs.push(Instr::Label(rhs_label));
                let rhs_val = self.lower_expr(rhs, &IrTy::I32)?;
                let rhs_reg = self.val_to_reg(rhs_val)?;
                self.instrs.push(Instr::Store { src: rhs_reg, slot: phi_slot });

                // ── End: load and return result ───────────────────────────────────
                self.instrs.push(Instr::Label(end_label));
                let result_reg = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst: result_reg, slot: phi_slot });
                Ok(IrValue::Reg(result_reg))
            }

            // FLS §6.5.4: Unary negation `-operand`.
            //
            // Three cases depending on operand type:
            //   - f64 operand: emit `fneg d{dst}, d{src}` (Instr::FNegF64)
            //   - f32 operand: emit `fneg s{dst}, s{src}` (Instr::FNegF32)
            //   - integer operand: emit `neg x{dst}, x{src}` (Instr::Neg)
            //
            // FLS §6.1.2:37–45: Even `-5` or `-2.5_f64` in a non-const context
            // must emit a runtime instruction — no compile-time folding.
            //
            // FLS §6.5.4: "The type of a negation expression is the type of the operand."
            //
            // Cache-line note: all three ARM64 instructions are 4 bytes.
            ExprKind::Unary { op: crate::ast::UnaryOp::Neg, operand } => {
                if self.is_f64_expr(operand) {
                    let val = self.lower_expr(operand, &IrTy::F64)?;
                    let src = match val {
                        IrValue::FReg(r) => r,
                        _ => return Err(LowerError::Unsupported(
                            "f64 negation: expected float register".into(),
                        )),
                    };
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::FNegF64 { dst, src });
                    Ok(IrValue::FReg(dst))
                } else if self.is_f32_expr(operand) {
                    let val = self.lower_expr(operand, &IrTy::F32)?;
                    let src = match val {
                        IrValue::F32Reg(r) => r,
                        _ => return Err(LowerError::Unsupported(
                            "f32 negation: expected f32 register".into(),
                        )),
                    };
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::FNegF32 { dst, src });
                    Ok(IrValue::F32Reg(dst))
                } else {
                    let val = self.lower_expr(operand, &IrTy::I32)?;
                    let src = self.val_to_reg(val)?;
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::Neg { dst, src });
                    Ok(IrValue::Reg(dst))
                }
            }

            // FLS §6.5.4: Negation operator `!operand` — two cases:
            //
            //   1. `!bool_value` → logical NOT: `false` (0) ↔ `true` (1).
            //      ARM64: `eor x{dst}, x{src}, #1` (XOR with 1 flips bit 0).
            //      Emits `Instr::BoolNot`. Triggered when `ret_ty == IrTy::Bool`.
            //
            //   2. `!integer_value` → bitwise NOT: flip all bits.
            //      ARM64: `mvn x{dst}, x{src}` (alias for orn xD, xzr, xS).
            //      Emits `Instr::Not`. Triggered for all other `ret_ty`.
            //
            // The distinction is necessary because `mvn` of 0 is -1 (not 1),
            // and `mvn` of 1 is -2 (not 0). Using `mvn` for booleans produces
            // non-boolean results that break downstream `cbz` conditions.
            //
            // `ret_ty` determines which case applies:
            //   - `IrTy::Bool`: the expression is expected to produce a bool → logical NOT.
            //   - `IrTy::I32` or other: integer context → bitwise NOT.
            //
            // FLS §6.5.4: "The type of a negation expression is the type of the operand."
            // FLS §6.1.2:37–45: Both variants emit runtime instructions — no folding.
            //
            // Cache-line note: both `eor` (bool) and `mvn` (int) are 4 bytes.
            ExprKind::Unary { op: crate::ast::UnaryOp::Not, operand } => {
                if *ret_ty == IrTy::Bool {
                    // Logical NOT: lower operand as bool, emit BoolNot (eor).
                    // The operand is a bool expression (0 or 1); XOR with 1 flips it.
                    let val = self.lower_expr(operand, &IrTy::Bool)?;
                    let src = self.val_to_reg(val)?;
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::BoolNot { dst, src });
                    Ok(IrValue::Reg(dst))
                } else {
                    // Bitwise NOT: lower operand as i32, emit Not (mvn).
                    // FLS §6.5.4: `!n` for integers flips all bits.
                    let val = self.lower_expr(operand, &IrTy::I32)?;
                    let src = self.val_to_reg(val)?;
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::Not { dst, src });
                    Ok(IrValue::Reg(dst))
                }
            }

            // FLS §6.5.2: Dereference expression `*expr`.
            //
            // The operand must evaluate to a reference (pointer). Lower the
            // operand to obtain the address in a register, then emit `LoadPtr`
            // to load the value at that address.
            //
            // ARM64: `ldr x{dst}, [x{src}]` — register-indirect load.
            //
            // FLS §6.5.2: "A dereference expression also called a deref
            // expression is a unary operator expression that uses the
            // dereference operator."
            //
            // FLS §6.1.2:37–45: Runtime instruction — even if the reference
            // is statically known, the load must execute at runtime.
            //
            // Cache-line note: one 4-byte instruction. The load hits the cache
            // line that contains the referent on the stack.
            ExprKind::Unary { op: crate::ast::UnaryOp::Deref, operand } => {
                // Lower operand as I32 (a pointer is a register-width integer).
                let addr_val = self.lower_expr(operand, &IrTy::I32)?;
                let src = self.val_to_reg(addr_val)?;
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::LoadPtr { dst, src });
                Ok(IrValue::Reg(dst))
            }

            // FLS §6.5.1: Borrow expression `&place` or `&mut place`.
            //
            // Computes the address of the operand's stack slot and returns it
            // as a pointer value (a 64-bit integer on ARM64).
            //
            // ARM64: `add x{dst}, sp, #{slot * 8}` — forms the stack address.
            //
            // FLS §6.5.1: "A borrow expression also called a reference
            // expression is a unary operator expression that uses the borrow
            // operator."
            //
            // Supported place expressions (FLS §6.1.4):
            //   - Simple local variables: `&x`, `&mut x`
            //   - Named struct fields:    `&p.a`, `&mut p.a`
            //   - Tuple fields:           `&t.0`, `&mut t.0`
            //   - Chained field access:   `&p.inner.x`
            //
            // Resolution delegates to `resolve_place`, which recursively
            // converts the place expression to a flat stack-slot index.
            // Since all fields occupy contiguous numbered slots in the
            // function's stack frame, `add xD, sp, #(slot * 8)` is correct
            // for any depth of field access.
            //
            // FLS §6.1.2:37–45: Runtime instruction — the address is formed
            // at runtime even if the stack layout is statically known.
            //
            // Cache-line note: one 4-byte instruction. The resulting pointer
            // occupies one 8-byte register slot — same footprint as any integer.
            ExprKind::Unary {
                op: crate::ast::UnaryOp::Ref | crate::ast::UnaryOp::RefMut,
                operand,
            } => {
                // Resolve the operand to a stack slot via resolve_place, which
                // handles simple paths, named-field access, and tuple-field access.
                // FLS §6.1.4: Place expressions denote memory locations;
                // borrowing yields a pointer to the underlying slot.
                let (slot, _) = self.resolve_place(operand)?;
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::AddrOf { dst, slot });
                Ok(IrValue::Reg(dst))
            }

            // FLS §6.5.9: Type cast expression `expr as Ty`.
            //
            // A type cast expression converts the value of `expr` to the
            // target type. All numeric integer types are supported: signed
            // types lower with IrTy::I32 context (arithmetic: asr, sdiv),
            // unsigned types with IrTy::U32 context (arithmetic: lsr, udiv).
            //
            // FLS §6.5.9: Numeric casts. On ARM64 all integer types occupy a
            // 64-bit register. Same-width casts (i32↔u32, i64↔u64) are
            // identity at the register level — no instruction emitted. Widening
            // casts (i32→i64) are also identity since the register already
            // holds 64 bits. Narrowing casts (i64→i8) should truncate the upper
            // bits; galvanic does not yet emit explicit truncation — correct for
            // values within the target range.
            //
            // FLS §6.5.9 AMBIGUOUS: The spec says narrowing integer casts
            // truncate to the target type's bit width, but does not specify the
            // mechanism. Galvanic defers explicit truncation.
            //
            // FLS §6.1.2:37–45: The operand is lowered at runtime even if its
            // value is statically known — no constant folding.
            //
            // Cache-line note: identity casts emit zero instructions. The
            // source register is reused directly by the caller.
            ExprKind::Cast { expr: inner, ty } => {
                // Determine the target type name from the type path.
                let target_name = match &ty.kind {
                    crate::ast::TyKind::Path(segments) if segments.len() == 1 => {
                        segments[0].text(self.source)
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "cast to non-path type (only named types supported)".into(),
                        ));
                    }
                };

                match target_name {
                    // FLS §6.5.9: Signed integer targets.
                    // Includes bool → i32 (0/1 → 0/1 identity), all signed
                    // integer types. Narrowing (i64→i8, i64→i16) is identity
                    // at the register level for values within the target range.
                    "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => {
                        // FLS §6.5.9: If the inner expression is f64-typed,
                        // emit FCVTZS (float-to-signed-integer, truncating toward zero).
                        // FLS §6.5.9: If the inner expression is f32-typed,
                        // emit FCVTZS w{dst}, s{src}.
                        // Otherwise, re-lower as integer (identity or narrowing cast).
                        if self.is_f64_expr(inner) {
                            let val = self.lower_expr(inner, &IrTy::F64)?;
                            let src = match val {
                                IrValue::FReg(r) => r,
                                _ => return Err(LowerError::Unsupported(
                                    "expected float register for f64 cast source".into(),
                                )),
                            };
                            let dst = self.alloc_reg()?;
                            self.instrs.push(Instr::F64ToI32 { dst, src });
                            Ok(IrValue::Reg(dst))
                        } else if self.is_f32_expr(inner) {
                            // FLS §6.5.9: `f32 as i32` truncates toward zero.
                            // ARM64: `fcvtzs w{dst}, s{src}`.
                            let val = self.lower_expr(inner, &IrTy::F32)?;
                            let src = match val {
                                IrValue::F32Reg(r) => r,
                                _ => return Err(LowerError::Unsupported(
                                    "expected f32 register for f32 cast source".into(),
                                )),
                            };
                            let dst = self.alloc_reg()?;
                            self.instrs.push(Instr::F32ToI32 { dst, src });
                            Ok(IrValue::Reg(dst))
                        } else {
                            self.lower_expr(inner, &IrTy::I32)
                        }
                    }

                    // FLS §6.5.9: Unsigned integer targets.
                    // Division uses `udiv` and right shift uses `lsr` when the
                    // result is subsequently used in arithmetic with U32 context.
                    // Narrowing casts (u64→u8, u64→u16) are identity for small
                    // values; truncation deferred (see FLS §6.5.9 AMBIGUOUS above).
                    "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => {
                        self.lower_expr(inner, &IrTy::U32)
                    }

                    // FLS §6.5.9: Cast char to integer.
                    // `char as i32` / `char as u32`: the char's Unicode code point
                    // is reinterpreted as the target integer. All valid char values
                    // (0..=0x10FFFF = 1,114,111) are non-negative and fit in both
                    // i32 and u32, so no masking or sign-extension is needed.
                    //
                    // FLS §6.5.9: "Casting between integer types is allowed."
                    // FLS §2.4.5: char values are Unicode scalar values (u32-range).
                    //
                    // The inner expression is lowered as U32 (char's IR type) and
                    // the result register is used directly — zero extra instructions.
                    "char" => {
                        self.lower_expr(inner, &IrTy::U32)
                    }

                    // FLS §6.5.9: Cast to bool: nonzero → true, zero → false.
                    // Not yet implemented — requires a comparison instruction.
                    "bool" => Err(LowerError::Unsupported(
                        "cast to bool not yet supported (FLS §6.5.9)".into(),
                    )),

                    // FLS §6.5.9: Cast to f64.
                    //
                    // Three source types:
                    //   - `f32 as f64`: exact widening. ARM64: `fcvt d{dst}, s{src}`.
                    //   - `i32 as f64`: signed integer to double. ARM64: `scvtf d{dst}, w{src}`.
                    //   - Other integer types: lower as i32 then convert.
                    "f64" => {
                        if self.is_f32_expr(inner) {
                            // FLS §6.5.9: `f32 as f64` — exact widening conversion.
                            // ARM64: `fcvt d{dst}, s{src}`.
                            let val = self.lower_expr(inner, &IrTy::F32)?;
                            let src = match val {
                                IrValue::F32Reg(r) => r,
                                _ => return Err(LowerError::Unsupported(
                                    "cast f32→f64: expected f32 register".into(),
                                )),
                            };
                            let dst = self.alloc_reg()?;
                            self.instrs.push(Instr::F32ToF64 { dst, src });
                            Ok(IrValue::FReg(dst))
                        } else {
                            let val = self.lower_expr(inner, &IrTy::I32)?;
                            let src = match val {
                                IrValue::Reg(r) => r,
                                IrValue::I32(n) => {
                                    // Materialise the constant into a register first.
                                    let r = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(r, n));
                                    r
                                }
                                _ => return Err(LowerError::Unsupported(
                                    "cast to f64: expected integer register".into(),
                                )),
                            };
                            let dst = self.alloc_reg()?;
                            self.instrs.push(Instr::I32ToF64 { dst, src });
                            Ok(IrValue::FReg(dst))
                        }
                    }

                    // FLS §6.5.9: Cast to f32.
                    //
                    // Three source types:
                    //   - `f64 as f32`: narrowing, rounds to nearest-even. ARM64: `fcvt s{dst}, d{src}`.
                    //   - `i32 as f32`: signed integer to single. ARM64: `scvtf s{dst}, w{src}`.
                    //   - Other integer types: lower as i32 then convert.
                    "f32" => {
                        if self.is_f64_expr(inner) {
                            // FLS §6.5.9: `f64 as f32` — narrowing conversion, rounds to nearest-even.
                            // ARM64: `fcvt s{dst}, d{src}`.
                            let val = self.lower_expr(inner, &IrTy::F64)?;
                            let src = match val {
                                IrValue::FReg(r) => r,
                                _ => return Err(LowerError::Unsupported(
                                    "cast f64→f32: expected f64 register".into(),
                                )),
                            };
                            let dst = self.alloc_reg()?;
                            self.instrs.push(Instr::F64ToF32 { dst, src });
                            Ok(IrValue::F32Reg(dst))
                        } else {
                            let val = self.lower_expr(inner, &IrTy::I32)?;
                            let src = match val {
                                IrValue::Reg(r) => r,
                                IrValue::I32(n) => {
                                    let r = self.alloc_reg()?;
                                    self.instrs.push(Instr::LoadImm(r, n));
                                    r
                                }
                                _ => return Err(LowerError::Unsupported(
                                    "cast to f32: expected integer register".into(),
                                )),
                            };
                            let dst = self.alloc_reg()?;
                            self.instrs.push(Instr::I32ToF32 { dst, src });
                            Ok(IrValue::F32Reg(dst))
                        }
                    }

                    other => Err(LowerError::Unsupported(format!(
                        "cast to `{other}` not yet supported (FLS §6.5.9)"
                    ))),
                }
            }

            // FLS §6.13: Field access expression `receiver.field`.
            //
            // Supported form: the receiver must be a simple path expression
            // (single-segment) naming a local variable whose type was recorded
            // in `local_struct_types` when the struct literal was stored.
            //
            // Layout: struct variable `p` of type `Point { x: i32, y: i32 }`
            // stores `x` at slot `base` and `y` at slot `base + 1`. Field
            // access loads from `base + field_index`.
            //
            // Cache-line note: field access emits one `ldr` instruction (4 bytes).
            // For a struct with N fields, the Nth field load touches slot
            // `base + N - 1`; if base is cache-line-aligned, fields 0–7 fit
            // within one 64-byte cache line (8 × 8-byte slots).
            //
            // FLS §6.13: Field access expression `receiver.field`.
            //
            // Supports chained field access (`r.b.x`) via the `resolve_place`
            // helper, which recursively resolves the receiver and computes the
            // final stack slot using per-struct field offsets.
            //
            // FLS §6.13 AMBIGUOUS: The spec does not specify whether field
            // access on a temporary (non-place) expression is well-formed.
            // Galvanic restricts to named local variables and chained field
            // access — temporary struct values are not yet supported as receivers.
            //
            // Cache-line note: field access emits one `ldr` instruction (4 bytes).
            // For a struct with N fields, the Nth field load touches slot
            // `base + field_offset[N-1]`; if base is cache-line-aligned, all
            // scalar fields fit within the same 64-byte cache lines as their slots.
            ExprKind::FieldAccess { receiver, field } => {
                let field_name = field.text(self.source);

                // FLS §6.13, §9: Field access directly on a struct-returning free
                // function call — e.g., `make(1).x` where `make` is in
                // `struct_return_free_fns`.
                //
                // `resolve_place` only handles place expressions (named variables
                // and chained field accesses). A `Call` expression is not a place,
                // so we must handle this pattern before calling `resolve_place`.
                //
                // Strategy: allocate N temporary slots, emit `CallMut` to store all
                // N return registers, then load the requested field from the
                // appropriate slot. This mirrors the `let p = make(1); p.x` path
                // without a named binding.
                //
                // FLS §6.1.2:37–45: All instructions are runtime.
                // Cache-line note: CallMut emits bl + N stores; the Load re-materialises
                // one field. For N=2: 4 instructions = 16 bytes.
                if let ExprKind::Call { callee, args } = &receiver.kind
                    && let ExprKind::Path(segs) = &callee.kind
                    && segs.len() == 1
                {
                    let fn_name = segs[0].text(self.source);
                    if let Some(struct_name) = self.struct_return_free_fns.get(fn_name).cloned() {
                        let field_names = self
                            .struct_defs
                            .get(&struct_name)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "unknown struct `{struct_name}` from free fn `{fn_name}`"
                                ))
                            })?
                            .clone();
                        let n_fields = field_names.len();
                        let field_idx = field_names
                            .iter()
                            .position(|n| n == field_name)
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "no field `{field_name}` in struct `{struct_name}`"
                                ))
                            })?;
                        // Use struct_field_offsets if available (handles nested structs).
                        let offset = self
                            .struct_field_offsets
                            .get(&struct_name)
                            .and_then(|o| o.get(field_idx))
                            .copied()
                            .unwrap_or(field_idx);

                        // Allocate N temporary slots for the struct return value.
                        let base_slot = self.alloc_slot()?;
                        for _ in 1..n_fields {
                            self.alloc_slot()?;
                        }

                        // Evaluate arguments.
                        let mut arg_regs: Vec<u8> = Vec::new();
                        for arg_expr in args.iter() {
                            let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                            let reg = self.val_to_reg(val)?;
                            arg_regs.push(reg);
                        }

                        self.has_calls = true;
                        // Emit CallMut: bl fn_name, then store x0..x{N-1} to slots.
                        self.instrs.push(Instr::CallMut {
                            name: fn_name.to_owned(),
                            args: arg_regs,
                            write_back_slot: base_slot,
                            n_fields: n_fields as u8,
                        });

                        // Load the requested field from its slot.
                        let dst = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst, slot: base_slot + offset as u8 });
                        return Ok(IrValue::Reg(dst));
                    }
                }

                // Use resolve_place to handle both simple (`p.x`) and chained
                // (`r.b.x`) field access in a uniform way.
                //
                // FLS §6.13: The result of a field access is the value stored in
                // the field's stack slot. For scalar fields (None type), emit `ldr`.
                // For struct-type fields (Some type), returning only the base slot
                // is correct for read access — the caller can further chain accesses.
                let (slot, _field_ty) = self.resolve_place(expr)?;
                // FLS §4.2: Check if this slot holds a float struct field.
                // If so, emit LoadF64Slot/LoadF32Slot to keep the value in a
                // float register, enabling downstream float arithmetic without
                // an explicit cast. (FLS §6.5.5, §6.5.9)
                match self.slot_float_ty.get(&slot).copied() {
                    Some(IrTy::F64) => {
                        let dst = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadF64Slot { dst, slot });
                        Ok(IrValue::FReg(dst))
                    }
                    Some(IrTy::F32) => {
                        let dst = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadF32Slot { dst, slot });
                        Ok(IrValue::F32Reg(dst))
                    }
                    _ => {
                        let reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: reg, slot });
                        Ok(IrValue::Reg(reg))
                    }
                }
            }

            // FLS §6.12.2 / §10.1: Method call expression `receiver.method(args)`.
            //
            // Supported form: the receiver must be a simple variable path whose
            // type was recorded in `local_struct_types`. The method is resolved
            // to a mangled function `TypeName__method_name`.
            //
            // Lowering strategy for `&self` and `self` methods: each struct field
            // is passed as an individual register argument (a value copy). This
            // matches the method's calling convention emitted by `lower_fn` when
            // `impl_type` is set. The method can read but not mutate the caller's
            // fields (correct for `&self`; sufficient for `self` at this milestone).
            //
            // ARM64 ABI: fields → x0..x{N-1}, extra args → x{N}..x{N+M-1}.
            // Return value in x0.
            //
            // FLS §6.12.2 AMBIGUOUS: The spec does not specify how many
            // auto-deref steps are legal. Galvanic restricts to zero deref steps
            // (receiver must already be the correct struct type).
            //
            // Cache-line note: loading N fields emits N `ldr` instructions (4 bytes
            // each). For a 2-field struct this is 8 bytes — fits in one cache line
            // alongside the `bl` instruction.
            ExprKind::MethodCall { receiver, method, args } => {
                // ── &str built-in method dispatch (FLS §2.4.6) ──────────────
                //
                // `&str` is not a user-defined struct/enum, so it does not go
                // through the mangled-name table.  Handle `.len()` specially
                // before the struct/enum dispatch path.
                //
                // Case A: receiver is a string literal expression → length is
                //         a compile-time constant.
                // Case B: receiver is a path to a `local_str_slots` variable →
                //         load the slot that holds the byte-length value.
                //
                // Both cases require no explicit arguments (FLS §6.12.2 —
                // `len` takes `&self` only).
                let method_name = method.text(self.source);
                if method_name == "len" && args.is_empty() {
                    // Case A: `"hello".len()` — literal receiver.
                    if let ExprKind::LitStr = &receiver.kind {
                        let text = receiver.span.text(self.source);
                        let byte_len = parse_str_byte_len(text)?;
                        let r = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(r, byte_len as i32));
                        return Ok(IrValue::Reg(r));
                    }
                    // Case B: `s.len()` where `s` is a known `&str` variable.
                    if let ExprKind::Path(segs) = &receiver.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        if let Some(&slot) = self.locals.get(var_name)
                            && self.local_str_slots.contains(&slot) {
                                let r = self.alloc_reg()?;
                                self.instrs.push(Instr::Load { dst: r, slot });
                                return Ok(IrValue::Reg(r));
                            }
                    }
                    // Case C: `[e0, e1, ...].len()` — array literal receiver.
                    //
                    // FLS §4.5: An array type `[T; N]` has a fixed element count N
                    // encoded at compile time. `.len()` on an array literal is a
                    // compile-time constant equal to the number of elements.
                    // FLS §6.12.2: Method call expressions.
                    //
                    // Cache-line note: emits one `LoadImm` — identical cost to an
                    // integer literal. No runtime memory access.
                    if let ExprKind::Array(elems) = &receiver.kind {
                        let n = elems.len();
                        let r = self.alloc_reg()?;
                        self.instrs.push(Instr::LoadImm(r, n as i32));
                        return Ok(IrValue::Reg(r));
                    }
                    // Case D: `arr.len()` where `arr` is a known array variable.
                    //
                    // FLS §4.5: The element count N is part of the array type and
                    // is recorded in `local_array_lens` when the variable is
                    // bound (let binding or function parameter). `.len()` is a
                    // compile-time constant — no runtime load required.
                    // FLS §6.12.2: Method call expressions.
                    //
                    // Cache-line note: emits one `LoadImm` — no heap or stack
                    // access, identical to reading a `const`.
                    if let ExprKind::Path(segs) = &receiver.kind
                        && segs.len() == 1
                    {
                        let var_name = segs[0].text(self.source);
                        if let Some(&slot) = self.locals.get(var_name)
                            && let Some(&arr_len) = self.local_array_lens.get(&slot)
                        {
                            let r = self.alloc_reg()?;
                            self.instrs.push(Instr::LoadImm(r, arr_len as i32));
                            return Ok(IrValue::Reg(r));
                        }
                    }
                }

                // Resolve the receiver to a struct or enum variable's base slot and type.
                //
                // FLS §6.12.2: Method call expressions — `receiver.method(args)`.
                // FLS §10.1, §11: Methods may be defined on both struct and enum types.
                let (recv_base_slot, recv_type_name) = match &receiver.kind {
                    ExprKind::Path(segs) if segs.len() == 1 => {
                        let var_name = segs[0].text(self.source);
                        let base_slot =
                            self.locals.get(var_name).copied().ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "undefined variable `{var_name}` in method call"
                                ))
                            })?;
                        // Check both struct and enum type registries.
                        // FLS §10.1, §11, §14.2: Check struct, enum, and tuple struct registries.
                        let type_name = self
                            .local_struct_types
                            .get(&base_slot)
                            .or_else(|| self.local_enum_types.get(&base_slot))
                            .or_else(|| self.local_tuple_struct_types.get(&base_slot))
                            .ok_or_else(|| {
                                LowerError::Unsupported(format!(
                                    "variable `{var_name}` is not a struct, enum, or tuple struct; \
                                     method calls on primitive types are not yet supported"
                                ))
                            })?
                            .clone();
                        (base_slot, type_name)
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "method call on non-variable receiver not yet supported".into(),
                        ));
                    }
                };

                // Build mangled function name: TypeName__method_name.
                // (`method_name` was already computed above for &str early return.)
                let mangled = format!("{recv_type_name}__{method_name}");

                // Load receiver fields into registers to pass as leading arguments.
                //
                // For struct receivers: one register per field (field 0 → x0, ...).
                // For enum receivers: discriminant in x0, then field registers x1..x{max_fields}.
                //
                // FLS §10.1: `self` is passed by value in declaration order, matching
                // the parameter-spill order in lower_fn.
                // FLS §6.1.2:37–45: All loads are runtime instructions.
                // Cache-line note: N loads = N × 4-byte `ldr` instructions.
                let mut arg_regs: Vec<u8> = Vec::with_capacity(8);
                let mut float_self_regs: Vec<u8> = Vec::new();
                let n_self_regs: usize;

                if let Some(field_names) = self.struct_defs.get(recv_type_name.as_str()).cloned() {
                    // Struct receiver: load each field.
                    // FLS §4.2, §10.1: f64/f32 fields arrive in the float register bank;
                    // integer fields arrive in x0-x7. Match the spill order in lower_fn.
                    let n_fields = field_names.len();
                    n_self_regs = n_fields;
                    for fi in 0..n_fields {
                        let slot = recv_base_slot + fi as u8;
                        match self.slot_float_ty.get(&slot).copied() {
                            Some(IrTy::F64) => {
                                let reg = self.alloc_reg()?;
                                self.instrs.push(Instr::LoadF64Slot { dst: reg, slot });
                                float_self_regs.push(reg);
                            }
                            Some(IrTy::F32) => {
                                let reg = self.alloc_reg()?;
                                self.instrs.push(Instr::LoadF32Slot { dst: reg, slot });
                                float_self_regs.push(reg);
                            }
                            _ => {
                                let reg = self.alloc_reg()?;
                                self.instrs.push(Instr::Load { dst: reg, slot });
                                arg_regs.push(reg);
                            }
                        }
                    }
                } else if let Some(variants) = self.enum_defs.get(recv_type_name.as_str()).cloned() {
                    // Enum receiver: load discriminant then field registers.
                    //
                    // FLS §15: Enum calling convention — discriminant first, fields follow.
                    // FLS §10.1: `&mut self` on enums not yet supported (value semantics
                    // for mutation would require write-back of discriminant + all fields).
                    let max_fields = variants.values().map(|(_, names)| names.len()).max().unwrap_or(0);
                    n_self_regs = 1 + max_fields;
                    // Load discriminant.
                    let disc_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::Load { dst: disc_reg, slot: recv_base_slot });
                    arg_regs.push(disc_reg);
                    // Load field registers.
                    for fi in 0..max_fields {
                        let slot = recv_base_slot + 1 + fi as u8;
                        let reg = self.alloc_reg()?;
                        self.instrs.push(Instr::Load { dst: reg, slot });
                        arg_regs.push(reg);
                    }
                } else if let Some(&n_fields) = self.tuple_struct_defs.get(recv_type_name.as_str()) {
                    // FLS §14.2, §10.1: Tuple struct receiver — load N fields from
                    // consecutive slots. f64/f32 fields go in float registers (d/s),
                    // integer fields go in integer registers (x).
                    //
                    // FLS §4.2: float fields → float register bank.
                    // FLS §6.1.2:37–45: All loads are runtime instructions.
                    // Cache-line note: N × 4-byte `ldr` instructions per method call.
                    n_self_regs = n_fields;
                    for fi in 0..n_fields {
                        let slot = recv_base_slot + fi as u8;
                        match self.slot_float_ty.get(&slot).copied() {
                            Some(IrTy::F64) => {
                                let reg = self.alloc_reg()?;
                                self.instrs.push(Instr::LoadF64Slot { dst: reg, slot });
                                float_self_regs.push(reg);
                            }
                            Some(IrTy::F32) => {
                                let reg = self.alloc_reg()?;
                                self.instrs.push(Instr::LoadF32Slot { dst: reg, slot });
                                float_self_regs.push(reg);
                            }
                            _ => {
                                let reg = self.alloc_reg()?;
                                self.instrs.push(Instr::Load { dst: reg, slot });
                                arg_regs.push(reg);
                            }
                        }
                    }
                } else {
                    return Err(LowerError::Unsupported(format!(
                        "unknown type `{recv_type_name}` in method call"
                    )));
                }

                // Lower explicit arguments (left-to-right, FLS §6.4:14).
                for arg_expr in args {
                    let val = self.lower_expr(arg_expr, &IrTy::I32)?;
                    let reg = self.val_to_reg(val)?;
                    arg_regs.push(reg);
                }

                if arg_regs.len() > 8 {
                    return Err(LowerError::Unsupported(
                        "method call with more than 8 total arguments (exceeds ARM64 register window)".into(),
                    ));
                }

                // FLS §10.1: Check whether this is a `&mut self` method.
                // For struct receivers: emit `CallMut` which writes modified fields
                // back to the caller's struct slots after the `bl`.
                // For enum receivers: `&mut self` is not yet supported.
                //
                // FLS §6.12.2: Method call expressions are dispatched to the
                // mangled function `TypeName__method_name`.
                let is_mut_self = self.method_self_kinds.get(&mangled)
                    == Some(&SelfKind::RefMut);

                self.has_calls = true;
                // Guard: struct-returning &self methods cannot be used as scalar expressions.
                // The caller needs a destination slot (use in a `let` binding).
                //
                // FLS §10.1 AMBIGUOUS: No spec-defined way to discard a struct return.
                if self.struct_return_methods.contains_key(&mangled) {
                    return Err(LowerError::Unsupported(format!(
                        "`{mangled}` returns a struct; use it in a `let` binding"
                    )));
                }

                if is_mut_self {
                    // `&mut self` call: write back x0..x{n_self_regs-1} to receiver slots.
                    // The callee emits `RetFields` (or `RetFieldsAndValue` for scalar returns),
                    // placing modified field values in x0..x{N-1} before returning.
                    // We store them back here.
                    //
                    // If the method has a scalar return type, also capture x{N} as the result.
                    //
                    // FLS §10.1: Write-back convention — callee returns modified fields;
                    // caller stores them. Scalar return in x{N} for non-unit methods.
                    //
                    // Cache-line note: N write-back stores = N × 4-byte `str`.
                    // For a 2-field struct: 8 bytes — fits in half a cache line.
                    let has_scalar_return = self.mut_self_scalar_return_fns.contains(&mangled);
                    if has_scalar_return {
                        // `&mut self` method returning a scalar value.
                        // Emit CallMutReturn: write-back fields, then capture x{N} into dst.
                        let dst = self.alloc_reg()?;
                        self.instrs.push(Instr::CallMutReturn {
                            name: mangled,
                            args: arg_regs,
                            write_back_slot: recv_base_slot,
                            n_fields: n_self_regs as u8,
                            dst,
                        });
                        Ok(IrValue::Reg(dst))
                    } else {
                        self.instrs.push(Instr::CallMut {
                            name: mangled,
                            args: arg_regs,
                            write_back_slot: recv_base_slot,
                            n_fields: n_self_regs as u8,
                        });
                        Ok(IrValue::Unit)
                    }
                } else {
                    let dst = self.alloc_reg()?;
                    // FLS §4.2: Method calls may return f64/f32; check the mangled name.
                    let float_ret = if self.f64_return_fns.contains(&mangled) {
                        Some(true)
                    } else if self.f32_return_fns.contains(&mangled) {
                        Some(false)
                    } else {
                        None
                    };
                    // FLS §4.2: Pass f64/f32 self fields in the float register bank.
                    // float_self_regs contains d-register indices for float fields.
                    self.instrs.push(Instr::Call {
                        dst,
                        name: mangled,
                        args: arg_regs,
                        float_args: float_self_regs,
                        float_ret,
                    });
                    match float_ret {
                        Some(true) => Ok(IrValue::FReg(dst)),
                        Some(false) => Ok(IrValue::F32Reg(dst)),
                        None => Ok(IrValue::Reg(dst)),
                    }
                }
            }

            // FLS §6.9: Indexing expression `base[index]`.
            //
            // Lowering strategy:
            // 1. Resolve `base` to a simple variable path and look up its stack slot.
            // 2. Lower `index` to a virtual register.
            // 3. Emit `LoadIndexed { dst, base_slot, index_reg }`.
            //
            // FLS §6.9: The base must be an array (or slice). Galvanic restricts
            // the base to a simple variable path whose type is a known array
            // (registered in `local_array_lens`).
            //
            // FLS §6.9 AMBIGUOUS: Out-of-bounds access must panic, but the panic
            // mechanism without the standard library is not specified. No bounds
            // check is emitted at this milestone.
            //
            // FLS §6.1.2:37–45: All instructions are runtime (no constant folding).
            //
            // Cache-line note: `LoadIndexed` emits two 4-byte instructions (add + ldr),
            // so an indexed read costs 8 bytes of instruction cache.
            ExprKind::Index { base, index } => {
                // FLS §6.9: 2D indexing `grid[i][j]` where `grid` is `[[T; M]; N]`.
                //
                // The outer `Index { base: Inner, index: j }` has an inner base that
                // is itself an `Index { base: Path("grid"), index: i }`. The linear
                // slot is `base_slot_of_grid + i * M + j` where M is the inner length.
                //
                // Runtime code emitted: LoadImm(M) + Mul(i*M) + Add(i*M+j) + LoadIndexed.
                // FLS §6.1.2:37–45: All instructions are runtime.
                // Cache-line note: 4 instructions (16 bytes) for a 2D index read.
                if let ExprKind::Index { base: inner_base, index: i_expr } = &base.kind
                    && let ExprKind::Path(segs) = &inner_base.kind
                    && segs.len() == 1
                {
                    let var_name = segs[0].text(self.source);
                    let base_slot = *self.locals.get(var_name).ok_or_else(|| {
                        LowerError::Unsupported(format!(
                            "undefined variable `{var_name}` in 2D index expression"
                        ))
                    })?;
                    let inner_len =
                        *self.local_array_inner_lens.get(&base_slot).ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "variable `{var_name}` is not a 2D array (nested indexing requires [[T; M]; N])"
                            ))
                        })?;
                    // Lower outer (row) index i.
                    let i_val = self.lower_expr(i_expr, &IrTy::I32)?;
                    let i_reg = self.val_to_reg(i_val)?;
                    // Lower inner (column) index j.
                    let j_val = self.lower_expr(index, &IrTy::I32)?;
                    let j_reg = self.val_to_reg(j_val)?;
                    // Compute linear index: i * inner_len + j.
                    let m_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadImm(m_reg, inner_len as i32));
                    let prod_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::BinOp {
                        op: IrBinOp::Mul,
                        dst: prod_reg,
                        lhs: i_reg,
                        rhs: m_reg,
                    });
                    let linear_reg = self.alloc_reg()?;
                    self.instrs.push(Instr::BinOp {
                        op: IrBinOp::Add,
                        dst: linear_reg,
                        lhs: prod_reg,
                        rhs: j_reg,
                    });
                    let dst = self.alloc_reg()?;
                    self.instrs.push(Instr::LoadIndexed {
                        dst,
                        base_slot,
                        index_reg: linear_reg,
                    });
                    return Ok(IrValue::Reg(dst));
                }

                // Resolve the base to an array variable's stack slot.
                let base_slot = match &base.kind {
                    ExprKind::Path(segs) if segs.len() == 1 => {
                        let var_name = segs[0].text(self.source);
                        let slot = *self.locals.get(var_name).ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "undefined variable `{var_name}` in index expression"
                            ))
                        })?;
                        // Verify this variable is a known array.
                        if !self.local_array_lens.contains_key(&slot) {
                            return Err(LowerError::Unsupported(format!(
                                "variable `{var_name}` is not an array (indexing non-arrays not yet supported)"
                            )));
                        }
                        slot
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "index expression on non-variable base not yet supported".into(),
                        ));
                    }
                };

                // Lower the index expression to a register.
                // FLS §6.9: The index is of type `usize`; galvanic uses i32 here.
                let idx_val = self.lower_expr(index, &IrTy::I32)?;
                let index_reg = self.val_to_reg(idx_val)?;

                // FLS §4.5, §4.2: Emit float-typed indexed load for f64/f32 arrays.
                // The base slot's element type was recorded during array literal lowering.
                let dst = self.alloc_reg()?;
                if self.local_f64_array_slots.contains(&base_slot) {
                    // FLS §4.5: `[f64; N]` — load element into a d-register.
                    self.instrs.push(Instr::LoadIndexedF64 { dst, base_slot, index_reg });
                    Ok(IrValue::FReg(dst))
                } else if self.local_f32_array_slots.contains(&base_slot) {
                    // FLS §4.5: `[f32; N]` — load element into an s-register.
                    self.instrs.push(Instr::LoadIndexedF32 { dst, base_slot, index_reg });
                    Ok(IrValue::F32Reg(dst))
                } else {
                    // Integer/boolean array: load into an x-register.
                    self.instrs.push(Instr::LoadIndexed { dst, base_slot, index_reg });
                    Ok(IrValue::Reg(dst))
                }
            }

            // FLS §6.10: Tuple expression as a value (not as a let initializer).
            // This path is reached when a tuple literal appears as a tail expression
            // or in a context where it's used as a value directly (rare; most tuple
            // usage goes through the `let` path above). Not yet supported — tuples
            // must be bound to a named variable first.
            ExprKind::Tuple(_) => Err(LowerError::Unsupported(
                "tuple expression must be bound to a `let` variable at this milestone".into(),
            )),

            // FLS §6.14: Closure expression (non-capturing and capturing).
            //
            // A closure `|x: i32| -> i32 { x + captured }` compiles to:
            //   1. A hidden top-level function `__closure_{fn_name}_{counter}`.
            //   2. `LoadFnAddr { dst, name }` — the closure's address as a fn pointer.
            //
            // FLS §6.22: If the closure body references free variables from the
            // enclosing scope (variables not in its own parameter list), those
            // variables are captured. Galvanic implements capture-by-copy: each
            // captured variable becomes an extra *leading* parameter of the hidden
            // function. At every call site the caller loads the captured variables
            // from its own stack slots and passes them as leading arguments before
            // the explicit arguments.
            //
            // Example: `let n = 3; let f = |x: i32| x + n; f(7)` compiles to:
            //   hidden fn `__closure_main_0(n: i32, x: i32) -> i32 { x + n }`
            //   call: `x0 = load n; x1 = load 7; blr f`
            //
            // FLS §6.14: Non-capturing closures coerce to `fn` pointer types.
            // FLS §6.22: Capturing closures in galvanic also use fn pointer ABI
            //   with hidden leading params — this matches move-closure semantics
            //   for scalar types (FLS §6.22 AMBIGUOUS: the spec does not mandate
            //   a particular capture ABI; galvanic chooses leading-parameter capture).
            // FLS §6.1.2:37–45: The function body emits runtime instructions.
            //
            // Cache-line note: the closure address fits in one 8-byte register.
            // Each captured variable adds one extra argument register per call —
            // same cost as an explicit parameter.
            ExprKind::Closure { is_move: _, params, ret_ty, body } => {
                // Generate a unique name for the closure function.
                let closure_name =
                    format!("__closure_{}_{}", self.fn_name, self.closure_counter);
                self.closure_counter += 1;

                // Determine the closure's return type.
                // FLS §6.14: The return type is either annotated or inferred.
                // Galvanic defaults to i32 when the annotation is absent.
                let closure_ret_ty = match ret_ty {
                    Some(ty) => lower_ty(ty, self.source, self.type_aliases)?,
                    None => IrTy::I32,
                };

                // ── Capture analysis (FLS §6.22) ─────────────────────────────
                // Collect the closure's own parameter names so we don't treat
                // them as captures.
                let mut closure_param_names = std::collections::HashSet::new();
                for p in params {
                    if let crate::ast::Pat::Ident(span) = &p.pat {
                        let n = span.text(self.source);
                        closure_param_names.insert(n);
                    }
                }
                // Walk the body to find free variables referencing outer locals.
                let mut captures: Vec<(&str, u8)> = Vec::new();
                find_captures(body, &self.locals, &closure_param_names, self.source, &mut captures);

                // Build a new LowerCtx for the closure body.
                // Labels continue from where the enclosing function left off so
                // that all labels in the assembly output are globally unique.
                // FLS §6.17: branch labels must be unique across the module.
                let closure_start_label = self.next_label;
                let mut closure_ctx = LowerCtx::new(
                    self.source,
                    &closure_name,
                    closure_ret_ty,
                    self.struct_defs,
                    self.tuple_struct_defs,
                    self.tuple_struct_float_field_types,
                    self.enum_defs,
                    self.enum_variant_float_field_types,
                    self.method_self_kinds,
                    self.mut_self_scalar_return_fns,
                    self.struct_return_fns,
                    self.struct_return_free_fns,
                    self.enum_return_fns,
                    self.struct_return_methods,
                    self.tuple_return_free_fns,
                    self.f64_return_fns,
                    self.f32_return_fns,
                    self.const_vals,
                    self.const_f64_vals,
                    self.const_f32_vals,
                    self.static_names,
                    self.static_f64_names,
                    self.static_f32_names,
                    &self.fn_names,
                    self.struct_field_types,
                    self.struct_field_offsets,
                    self.struct_sizes,
                    self.type_aliases,
                    self.struct_float_field_types,
                    closure_start_label,
                );

                // ── Spill captured variables (FLS §6.22) ─────────────────────
                // Each captured variable arrives as a leading argument register
                // (x0, x1, ...) and is spilled to a stack slot, then bound in
                // the closure's local scope under the same name.
                //
                // FLS §6.22: Captures precede explicit parameters in the ABI so
                // the caller can pass them without knowing the explicit-param arity.
                // FLS §6.1.2:37–45: All spills are runtime store instructions.
                let n_captures = captures.len();
                for (i, (cap_name, _outer_slot)) in captures.iter().enumerate() {
                    let slot = closure_ctx.alloc_slot()?;
                    closure_ctx.instrs.push(Instr::Store { src: i as u8, slot });
                    closure_ctx.locals.insert(cap_name, slot);
                }

                // ── Spill explicit parameters (FLS §6.14, §9) ────────────────
                // Explicit parameters arrive after the captured variables, so
                // parameter i occupies register x{n_captures + i}.
                //
                // FLS §6.14: Closure parameters follow the ARM64 calling convention;
                // the first N params arrive in x0..x{N-1} after captures.
                // FLS §9: Same spill strategy as free functions.
                // FLS §6.1.2:37–45: All spills are runtime store instructions.
                for (i, param) in params.iter().enumerate() {
                    let reg = (n_captures + i) as u8;
                    let slot = closure_ctx.alloc_slot()?;
                    closure_ctx.instrs.push(Instr::Store { src: reg, slot });
                    match &param.pat {
                        crate::ast::Pat::Ident(name_span) => {
                            let name = name_span.text(self.source);
                            if name != "_" {
                                closure_ctx.locals.insert(name, slot);
                            }
                        }
                        crate::ast::Pat::Wildcard => {}
                        other => {
                            return Err(LowerError::Unsupported(format!(
                                "only identifier and wildcard patterns are supported \
                                 in closure parameters at this milestone, found {other:?}"
                            )));
                        }
                    }
                    // Register fn-ptr params so indirect call emits `blr`.
                    if let Some(ty) = &param.ty
                        && matches!(lower_ty(ty, self.source, self.type_aliases), Ok(IrTy::FnPtr))
                    {
                        closure_ctx.local_fn_ptr_slots.insert(slot);
                    }
                }

                // Lower the closure body.
                // FLS §6.14: The body is evaluated when the closure is invoked.
                // FLS §6.1.2:37–45: Body emits runtime instructions, no constant folding.
                let body_val = closure_ctx.lower_expr(body, &closure_ret_ty)?;
                closure_ctx.instrs.push(Instr::Ret(body_val));

                // Update the enclosing function's label counter past what the closure used.
                self.next_label = closure_ctx.next_label;

                // Build the IrFn for the closure.
                let stack_slots = closure_ctx.next_slot;
                let saves_lr = closure_ctx.has_calls;
                let closure_fn = IrFn {
                    name: closure_name.clone(),
                    ret_ty: closure_ret_ty,
                    body: closure_ctx.instrs,
                    stack_slots,
                    saves_lr,
                    float_consts: closure_ctx.float_consts,
                    float32_consts: closure_ctx.float32_consts,
                };

                // Collect this closure and any closures defined inside it.
                // FLS §6.14: Nested closures compile to additional hidden functions.
                // Also propagate any trampolines generated inside the closure.
                self.pending_closures.push(closure_fn);
                self.pending_closures.extend(closure_ctx.pending_closures);
                self.pending_trampolines.extend(closure_ctx.pending_trampolines);

                // Record the captured outer-scope slots so the let-binding handler
                // can register them for the call site.
                // FLS §6.22: The enclosing `lower_stmt(Let)` drains `last_closure_captures`
                // after allocating a stack slot for the closure variable.
                // Also record the closure name and explicit parameter count for the
                // call-arg trampoline generator (FLS §6.22, §4.13).
                if !captures.is_empty() {
                    let outer_slots: Vec<u8> = captures.iter().map(|(_, s)| *s).collect();
                    self.last_closure_captures = Some(outer_slots);
                    self.last_closure_name = Some(closure_name.clone());
                    self.last_closure_n_explicit = Some(params.len());
                }

                // Materialise the closure's address as a function pointer value.
                // FLS §4.9: `LoadFnAddr` emits ADRP + ADD to load the label address.
                // FLS §6.14: The closure expression evaluates to this address.
                //
                // Cache-line note: ADRP + ADD = 8 bytes; same cost as a static load.
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::LoadFnAddr { dst, name: closure_name });
                Ok(IrValue::Reg(dst))
            }

            // Anything else: not yet supported as runtime codegen.
            _ => Err(LowerError::Unsupported(
                "expression kind in non-const context (runtime codegen not yet implemented)".into(),
            )),
        }
    }
}

