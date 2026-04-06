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

use crate::ast::{BinOp, Block, Expr, ExprKind, ItemKind, Pat, SourceFile, Stmt, StmtKind, TyKind};
use crate::ir::{IrBinOp, Instr, IrFn, IrTy, IrValue, Module};

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
        ExprKind::Call { .. } => true,
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
        ExprKind::While { cond, body } => {
            expr_contains_call(cond) || block_contains_call(body)
        }
        ExprKind::Loop(body) => block_contains_call(body),
        ExprKind::Break(opt_val) => opt_val.as_ref().is_some_and(|e| expr_contains_call(e)),
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
    }
}

/// Return `true` if the expression contains a `break <value>` at the current
/// loop level. Does **not** recurse into nested loop bodies, because `break`
/// statements inside nested loops belong to those loops, not the outer one.
fn expr_contains_break_with_value(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Break(Some(_)) => true,
        // Recurse into non-loop control-flow.
        ExprKind::If { cond, then_block, else_expr } => {
            expr_contains_break_with_value(cond)
                || block_contains_break_with_value(then_block)
                || else_expr.as_ref().is_some_and(|e| expr_contains_break_with_value(e))
        }
        ExprKind::Block(b) => block_contains_break_with_value(b),
        ExprKind::Match { scrutinee, arms } => {
            expr_contains_break_with_value(scrutinee)
                || arms.iter().any(|a| expr_contains_break_with_value(&a.body))
        }
        // Do NOT recurse into nested loops — their `break` belongs to them.
        ExprKind::Loop(_) | ExprKind::While { .. } | ExprKind::For { .. } => false,
        _ => false,
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
/// FLS §6.12.1: Functions with parameters receive arguments in x0–x{n-1}
/// per the ARM64 ABI. We spill each parameter to a stack slot so that
/// path expressions can reference them via `Load` — reusing the same
/// infrastructure as let-binding locals.
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
        Some(block) => block,
    };

    let mut ctx = LowerCtx::new(source, ret_ty);

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
    for (i, param) in fn_def.params.iter().enumerate() {
        if i >= 8 {
            return Err(LowerError::Unsupported(
                "functions with more than 8 parameters (exceeds ARM64 register window)".into(),
            ));
        }
        let param_ty = lower_ty(&param.ty, source)?;
        // Only i32 and bool parameters are supported (both use integer registers).
        // FLS §4.3: bool is passed as a 32-bit integer register on ARM64.
        // FLS §4.1: i32 parameters occupy one 64-bit register (x0–x7).
        if !matches!(param_ty, IrTy::I32 | IrTy::Bool) {
            return Err(LowerError::Unsupported("parameter type other than i32/bool".into()));
        }
        let slot = ctx.alloc_slot()?;
        let param_name = param.name.text(source);
        ctx.locals.insert(param_name, slot);
        // Spill parameter register i (arm64 x{i}) to its stack slot.
        // `src: i as u8` directly names the incoming register — this is
        // safe because the body hasn't allocated any virtual registers yet.
        ctx.instrs.push(Instr::Store { src: i as u8, slot });
    }

    ctx.lower_block(body, &ret_ty)?;

    let body_instrs = ctx.instrs;
    let stack_slots = ctx.next_slot;
    let saves_lr = ctx.has_calls;
    Ok(IrFn { name, ret_ty, body: body_instrs, stack_slots, saves_lr })
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
fn lower_ty(ty: &crate::ast::Ty, source: &str) -> Result<IrTy, LowerError> {
    match &ty.kind {
        TyKind::Unit => Ok(IrTy::Unit),
        TyKind::Path(segments) if segments.len() == 1 => {
            match segments[0].text(source) {
                "i32" => Ok(IrTy::I32),
                // FLS §4.3: bool is a distinct type in the IR so that `!` can
                // emit logical NOT (eor, XOR with 1) rather than bitwise NOT (mvn).
                // On ARM64, bool and i32 share the same register layout (0/1 as i64),
                // but the semantics of `!` differ.
                "bool" => Ok(IrTy::Bool),
                name => Err(LowerError::Unsupported(format!("type `{name}`"))),
            }
        }
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
}

impl<'src> LowerCtx<'src> {
    fn new(source: &'src str, fn_ret_ty: IrTy) -> Self {
        LowerCtx {
            source,
            instrs: Vec::new(),
            next_reg: 0,
            next_slot: 0,
            next_label: 0,
            locals: HashMap::new(),
            has_calls: false,
            loop_stack: Vec::new(),
            fn_ret_ty,
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

    /// Allocate the next unique label ID.
    ///
    /// FLS §6.17: Each if expression needs two labels (else and end).
    /// Labels are function-scoped and monotonically increasing.
    fn alloc_label(&mut self) -> u32 {
        let id = self.next_label;
        self.next_label += 1;
        id
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
            StmtKind::Let { name, ty: _, init } => {
                let slot = self.alloc_slot()?;
                let var_name = name.text(self.source);
                self.locals.insert(var_name, slot);

                if let Some(init_expr) = init.as_ref() {
                    // Lower the initializer. We assume i32 for numeric
                    // expressions. Type inference is future work.
                    //
                    // FLS §8.1 AMBIGUOUS: the spec does not describe how type
                    // inference resolves the type of the initializer in the
                    // absence of a type annotation. We default to i32 for
                    // integer-producing expressions.
                    let val = self.lower_expr(init_expr, &IrTy::I32)?;
                    let src = self.val_to_reg(val)?;
                    self.instrs.push(Instr::Store { src, slot });
                }
                // If no initializer: slot is allocated and registered but no
                // Store is emitted. FLS §8.1 (uninitialized let binding).

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
                    _ => Err(LowerError::Unsupported("integer literal with non-i32 type".into())),
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

            // FLS §4.4: Unit literal `()`.
            ExprKind::Unit => Ok(IrValue::Unit),

            // FLS §6.4: Block expression — lower statements then the tail value.
            //
            // A block expression `{ stmt; ...; tail }` evaluates each statement
            // in order and produces the tail expression's value.
            ExprKind::Block(block) => {
                self.lower_block_to_value(block, ret_ty)
            }

            // FLS §6.3: Path expression — a reference to a local variable.
            //
            // A single-segment path is a local variable reference. Emits a
            // `Load` instruction to read the value from its stack slot at runtime.
            //
            // FLS §6.1.2:37–45: The load is a runtime instruction — even if
            // the variable holds a statically-known value, we must load it.
            ExprKind::Path(segments) if segments.len() == 1 => {
                let var_name = segments[0].text(self.source);
                let slot = self.locals.get(var_name).copied().ok_or_else(|| {
                    LowerError::Unsupported(format!("undefined variable `{var_name}`"))
                })?;
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::Load { dst, slot });
                Ok(IrValue::Reg(dst))
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
                    _ => Err(LowerError::Unsupported("bitwise/arithmetic on non-i32 type".into())),
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
                    IrTy::I32 | IrTy::Bool => {
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
                let scrut_val = self.lower_expr(scrutinee, &scrut_ty)?;
                let scrut_reg = self.val_to_reg(scrut_val)?;
                let scrut_slot = self.alloc_slot()?;
                self.instrs.push(Instr::Store { src: scrut_reg, slot: scrut_slot });

                // Split into checked arms (all but the last) and the default arm.
                // The default arm is emitted unconditionally (exhaustiveness deferred).
                let (checked_arms, default_arm) = arms.split_at(arms.len() - 1);

                let exit_label = self.alloc_label();

                match ret_ty {
                    IrTy::I32 | IrTy::Bool => {
                        let phi_slot = self.alloc_slot()?;

                        for arm in checked_arms {
                            let next_label = self.alloc_label();

                            match &arm.pat {
                                Pat::Wildcard => {
                                    // Wildcard in non-last position — treat as unconditional.
                                    // Lower body, store to phi, branch to exit.
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
                                // FLS §6.1.2:37–45: The ldr/str pair emits at runtime.
                                // Cache-line note: 2 instructions (ldr + str = 8 bytes).
                                Pat::Ident(span) => {
                                    let name = span.text(self.source);
                                    let bind_slot = self.alloc_slot()?;
                                    let bind_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                                    self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                                    self.locals.insert(name, bind_slot);
                                    let body_val = self.lower_expr(&arm.body, ret_ty)?;
                                    self.locals.remove(name);
                                    let body_reg = self.val_to_reg(body_val)?;
                                    self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });
                                    self.instrs.push(Instr::Branch(exit_label));
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
                                                let alt_imm = match alt {
                                                    Pat::LitInt(n) => *n as i32,
                                                    Pat::NegLitInt(n) => -(*n as i32),
                                                    Pat::LitBool(b) => *b as i32,
                                                    _ => unreachable!(),
                                                };
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
                                        _ => unreachable!(),
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

                            // Arm body (reached when any pattern check matched).
                            let body_val = self.lower_expr(&arm.body, ret_ty)?;
                            let body_reg = self.val_to_reg(body_val)?;
                            self.instrs.push(Instr::Store { src: body_reg, slot: phi_slot });
                            self.instrs.push(Instr::Branch(exit_label));

                            self.instrs.push(Instr::Label(next_label));
                        }

                        // Default arm — unconditional.
                        // FLS §5.1.4: If the default arm has an identifier pattern,
                        // bind the scrutinee to the name before lowering the body.
                        let default_binding = match &default_arm[0].pat {
                            Pat::Ident(span) => {
                                let name = span.text(self.source);
                                let bind_slot = self.alloc_slot()?;
                                let bind_reg = self.alloc_reg()?;
                                self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                                self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                                self.locals.insert(name, bind_slot);
                                Some(name)
                            }
                            _ => None,
                        };
                        let body_val = self.lower_expr(&default_arm[0].body, ret_ty)?;
                        if let Some(name) = default_binding {
                            self.locals.remove(name);
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
                                    self.lower_expr(&arm.body, &IrTy::Unit)?;
                                    self.instrs.push(Instr::Branch(exit_label));
                                    self.instrs.push(Instr::Label(next_label));
                                    continue;
                                }
                                // FLS §5.1.4: Identifier pattern — always matches, binds name.
                                Pat::Ident(span) => {
                                    let name = span.text(self.source);
                                    let bind_slot = self.alloc_slot()?;
                                    let bind_reg = self.alloc_reg()?;
                                    self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                                    self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                                    self.locals.insert(name, bind_slot);
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
                                                let alt_imm = match alt {
                                                    Pat::LitInt(n) => *n as i32,
                                                    Pat::NegLitInt(n) => -(*n as i32),
                                                    Pat::LitBool(b) => *b as i32,
                                                    _ => unreachable!(),
                                                };
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
                                _ => {
                                    let s_reg = self.alloc_reg()?;
                                    self.instrs
                                        .push(Instr::Load { dst: s_reg, slot: scrut_slot });
                                    let pat_imm = match &arm.pat {
                                        Pat::LitInt(n) => *n as i32,
                                        Pat::NegLitInt(n) => -(*n as i32),
                                        Pat::LitBool(b) => *b as i32,
                                        _ => unreachable!(),
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

                            self.lower_expr(&arm.body, &IrTy::Unit)?;
                            self.instrs.push(Instr::Branch(exit_label));
                            self.instrs.push(Instr::Label(next_label));
                        }

                        // Default arm — unconditional.
                        // FLS §5.1.4: If the default arm has an identifier pattern, bind.
                        let default_binding_unit = match &default_arm[0].pat {
                            Pat::Ident(span) => {
                                let name = span.text(self.source);
                                let bind_slot = self.alloc_slot()?;
                                let bind_reg = self.alloc_reg()?;
                                self.instrs.push(Instr::Load { dst: bind_reg, slot: scrut_slot });
                                self.instrs.push(Instr::Store { src: bind_reg, slot: bind_slot });
                                self.locals.insert(name, bind_slot);
                                Some(name)
                            }
                            _ => None,
                        };
                        self.lower_expr(&default_arm[0].body, &IrTy::Unit)?;
                        if let Some(name) = default_binding_unit {
                            self.locals.remove(name);
                        }
                        self.instrs.push(Instr::Label(exit_label));
                        Ok(IrValue::Unit)
                    }
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
                // Resolve the callee to a function name.
                let fn_name = match &callee.kind {
                    ExprKind::Path(segments) if segments.len() == 1 => {
                        segments[0].text(self.source).to_owned()
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "call expression with non-path callee (function pointers not yet supported)".into(),
                        ));
                    }
                };

                if args.len() > 8 {
                    return Err(LowerError::Unsupported(
                        "call with more than 8 arguments (exceeds ARM64 register window)".into(),
                    ));
                }

                // Lower each argument to a virtual register, left-to-right.
                // We assume i32 arguments at this milestone — type inference
                // is future work.
                let mut arg_regs = Vec::with_capacity(args.len());
                for arg in args {
                    let val = self.lower_expr(arg, &IrTy::I32)?;
                    let reg = self.val_to_reg(val)?;
                    arg_regs.push(reg);
                }

                // Allocate the destination register for the return value.
                let dst = self.alloc_reg()?;

                self.has_calls = true;
                self.instrs.push(Instr::Call { dst, name: fn_name, args: arg_regs });

                Ok(IrValue::Reg(dst))
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
                // Resolve the LHS to a stack slot (must be a declared local).
                let slot = match &lhs.kind {
                    ExprKind::Path(segments) if segments.len() == 1 => {
                        let var_name = segments[0].text(self.source);
                        self.locals.get(var_name).copied().ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "assignment to undefined variable `{var_name}`"
                            ))
                        })?
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "assignment to non-variable place expression not yet supported".into(),
                        ));
                    }
                };

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
                // Resolve target to a stack slot.
                let slot = match &target.kind {
                    ExprKind::Path(segments) if segments.len() == 1 => {
                        let var_name = segments[0].text(self.source);
                        self.locals.get(var_name).copied().ok_or_else(|| {
                            LowerError::Unsupported(format!(
                                "compound assignment to undefined variable `{var_name}`"
                            ))
                        })?
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
                // Both operands must be i32 at this milestone.
                // FLS §6.5.3 AMBIGUOUS: the spec does not separately describe
                // the type-checking rules for comparisons in the absence of
                // type inference. We assume both sides are i32.
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
            ExprKind::While { cond, body } => {
                let header_label = self.alloc_label();
                let exit_label = self.alloc_label();

                // Push a loop context so that `break`/`continue` inside the body
                // can resolve to the correct labels.
                // FLS §6.15.3, §6.15.6, §6.15.7.
                // `while` loops do not support break-with-value (FLS §6.15.6).
                self.loop_stack.push(LoopCtx { header_label, exit_label, break_slot: None, break_ret_ty: IrTy::Unit });

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

                // FLS §6.15.3: "The type of a while expression is the unit type ()."
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
            ExprKind::For { pat, iter, body } => {
                // Only integer range iterators are supported at this milestone.
                let (start_expr, end_expr, inclusive) = match iter.as_ref() {
                    Expr { kind: ExprKind::Range { start: Some(s), end: Some(e), inclusive }, .. } => {
                        (s.as_ref(), e.as_ref(), *inclusive)
                    }
                    _ => return Err(LowerError::Unsupported(
                        "for loop requires an integer range iterator (start..end or start..=end)".into(),
                    )),
                };

                // Allocate the loop variable slot and record the name.
                let i_slot = self.alloc_slot()?;
                let pat_name = pat.text(self.source);

                // Allocate a slot for the end bound (evaluated once before the loop).
                // FLS §6.16: The range bounds are evaluated once, not on each iteration.
                let end_slot = self.alloc_slot()?;

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
                self.loop_stack.push(LoopCtx { header_label: incr_label, exit_label, break_slot: None, break_ret_ty: IrTy::Unit });

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
            ExprKind::Loop(body) => {
                let header_label = self.alloc_label();
                let exit_label = self.alloc_label();

                // FLS §6.15.6: Only `loop` expressions support break-with-value.
                // Scan the body for `break <value>` at this loop level. If any
                // are present, allocate a stack slot to hold the result — the
                // same phi-slot pattern used for if-else expressions.
                //
                // The break_slot is allocated BEFORE entering the loop body so
                // that `break <value>` can store into it during body lowering.
                let break_slot = if block_contains_break_with_value(body) {
                    Some(self.alloc_slot()?)
                } else {
                    None
                };

                self.loop_stack.push(LoopCtx { header_label, exit_label, break_slot, break_ret_ty: *ret_ty });

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
            ExprKind::Break(value) => {
                // Resolve the exit label and optional break slot from the
                // innermost loop context.
                let (exit_label, break_slot, break_ret_ty) = self.loop_stack.last()
                    .map(|ctx| (ctx.exit_label, ctx.break_slot, ctx.break_ret_ty))
                    .ok_or_else(|| LowerError::Unsupported(
                        "break expression outside of a loop".into()
                    ))?;

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

            // FLS §6.15.7: Continue expression — restart the innermost loop.
            //
            // A `continue` transfers control to the header of the innermost
            // enclosing loop, skipping any remaining statements in the body.
            //
            // FLS §6.15.7: "A continue expression advances to the next iteration
            // of the innermost enclosing loop expression."
            // FLS §6.15.7: "The type of a continue expression is the never type `!`."
            // We approximate `!` as Unit since the never type is not yet in the IR.
            //
            // Cache-line note: `b .L{header}` is 4 bytes — same cost as `break`.
            ExprKind::Continue => {
                // Resolve the header label from the innermost loop context.
                let header_label = self.loop_stack.last()
                    .map(|ctx| ctx.header_label)
                    .ok_or_else(|| LowerError::Unsupported(
                        "continue expression outside of a loop".into()
                    ))?;

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

            // FLS §6.5.4: Unary negation `-operand` — arithmetic two's complement negation.
            //
            // Lowering:
            //   1. Lower the operand to a register.
            //   2. Emit `Instr::Neg { dst, src }` → `neg x{dst}, x{src}` on ARM64.
            //
            // FLS §6.1.2:37–45: Even `-5` in a non-const context emits a runtime `neg`
            // instruction — no compile-time folding to a negative immediate.
            //
            // FLS §6.5.4: "The type of a negation expression is the type of the operand."
            //
            // Cache-line note: `neg` is 4 bytes (alias for `sub xD, xzr, xS`).
            ExprKind::Unary { op: crate::ast::UnaryOp::Neg, operand } => {
                let val = self.lower_expr(operand, &IrTy::I32)?;
                let src = self.val_to_reg(val)?;
                let dst = self.alloc_reg()?;
                self.instrs.push(Instr::Neg { dst, src });
                Ok(IrValue::Reg(dst))
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

            // FLS §6.5.9: Type cast expression `expr as Ty`.
            //
            // A type cast expression converts the value of `expr` to the
            // target type. At this milestone, only casts to `i32` and `bool`
            // are supported — both are represented identically as 32-bit
            // integer values in the IR (bool: 0 = false, 1 = true), so the
            // cast is a no-op at the instruction level: lower the operand as
            // i32 and return the result register unchanged.
            //
            // FLS §6.5.9: Supported numeric casts (at this milestone):
            //   i32 as i32  → identity (no instruction emitted)
            //   bool as i32 → identity (bool is already 0/1 in IR)
            //
            // FLS §6.1.2:37–45: The operand is lowered at runtime even if its
            // value is statically known — no constant folding.
            //
            // FLS §6.5.9 AMBIGUOUS: The spec enumerates valid cast combinations
            // in terms of "permitted coercions" but does not specify the exact
            // set of allowed source→target pairs. The Rust reference lists them;
            // galvanic follows the reference. Unsupported casts return an error.
            //
            // Cache-line note: no new instruction is emitted for identity casts
            // (i32→i32, bool→i32). The source value's register is reused directly.
            ExprKind::Cast { expr: inner, ty } => {
                // Determine the target type name from the type path.
                let target_name = match &ty.kind {
                    crate::ast::TyKind::Path(segments) if segments.len() == 1 => {
                        segments[0].text(self.source)
                    }
                    _ => {
                        return Err(LowerError::Unsupported(
                            "cast to non-path type (only named types like i32 supported)".into(),
                        ));
                    }
                };

                match target_name {
                    // FLS §6.5.9: Numeric casts to i32.
                    // Includes bool → i32 (0/1 → 0/1), i32 → i32 (identity).
                    // Both source types are already represented as 32-bit
                    // integers in the IR, so no instruction is emitted.
                    "i32" => {
                        // Lower the operand using i32 context. Boolean values
                        // (FLS §2.4.7) are already 0/1 integers in the IR, so
                        // this handles `bool as i32` correctly without any
                        // additional instruction.
                        self.lower_expr(inner, &IrTy::I32)
                    }

                    other => Err(LowerError::Unsupported(format!(
                        "cast to `{other}` (only `i32` target supported at this milestone)"
                    ))),
                }
            }

            // Anything else: not yet supported as runtime codegen.
            _ => Err(LowerError::Unsupported(
                "expression kind in non-const context (runtime codegen not yet implemented)".into(),
            )),
        }
    }
}
