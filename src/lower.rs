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

use crate::ast::{BinOp, Block, Expr, ExprKind, ItemKind, SourceFile, StmtKind, TyKind};
use crate::ir::{IrBinOp, Instr, IrFn, IrTy, IrValue, Module};

// ── FLS citations added in this module ───────────────────────────────────────
// FLS §6.12.1: Call expressions — `lower_expr` handles `ExprKind::Call`.
// FLS §9: Functions with parameters — `lower_fn` spills x0..x{n-1} to stack.

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

    let mut ctx = LowerCtx::new(source);

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
        // Only i32 parameters are supported at this milestone.
        if !matches!(param_ty, IrTy::I32) {
            return Err(LowerError::Unsupported("parameter type other than i32".into()));
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
}

impl<'src> LowerCtx<'src> {
    fn new(source: &'src str) -> Self {
        LowerCtx {
            source,
            instrs: Vec::new(),
            next_reg: 0,
            next_slot: 0,
            next_label: 0,
            locals: HashMap::new(),
            has_calls: false,
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
            // FLS §8.1: Let statement — allocate a stack slot and store the
            // initializer value. The variable name is registered in `locals`
            // so that later path expressions can emit a Load.
            //
            // FLS §6.1.2:37–45: The store is a runtime instruction; the
            // initializer is evaluated at runtime, not compile time.
            StmtKind::Let { name, ty: _, init } => {
                let init_expr = init.as_ref().ok_or_else(|| {
                    LowerError::Unsupported("uninitialized let binding (no initializer)".into())
                })?;

                // Lower the initializer. We assume i32 for numeric expressions.
                // Type inference is future work; this is sufficient for milestone 3.
                //
                // FLS §8.1 AMBIGUOUS: the spec does not describe how type
                // inference resolves the type of the initializer in the absence
                // of a type annotation. We default to i32 for integer-producing
                // expressions.
                let val = self.lower_expr(init_expr, &IrTy::I32)?;
                let src = self.val_to_reg(val)?;

                let slot = self.alloc_slot()?;
                let var_name = name.text(self.source);
                self.locals.insert(var_name, slot);
                self.instrs.push(Instr::Store { src, slot });

                Ok(())
            }

            // Expression statements — evaluate and discard.
            // Not yet supported at this milestone.
            StmtKind::Expr(_) => {
                Err(LowerError::Unsupported(
                    "expression statements (assignment, calls, etc.) not yet implemented".into(),
                ))
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
            //
            // Both operands are lowered recursively, producing LoadImm/BinOp
            // instructions. The result is in a virtual register.
            ExprKind::Binary { op, lhs, rhs }
                if matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul) =>
            {
                match ret_ty {
                    IrTy::I32 => {
                        let lhs_val = self.lower_expr(lhs, ret_ty)?;
                        let rhs_val = self.lower_expr(rhs, ret_ty)?;

                        let lhs_reg = self.val_to_reg(lhs_val)?;
                        let rhs_reg = self.val_to_reg(rhs_val)?;

                        let dst = self.alloc_reg()?;
                        let ir_op = match op {
                            BinOp::Add => IrBinOp::Add,
                            BinOp::Sub => IrBinOp::Sub,
                            BinOp::Mul => IrBinOp::Mul,
                            _ => unreachable!("matched above"),
                        };
                        self.instrs.push(Instr::BinOp { op: ir_op, dst, lhs: lhs_reg, rhs: rhs_reg });
                        Ok(IrValue::Reg(dst))
                    }
                    _ => Err(LowerError::Unsupported("arithmetic on non-i32 type".into())),
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
            // Limitation: this implementation handles the i32-valued case. Unit-
            // returning if expressions (if without else, or with `-> ()` branches)
            // are not yet supported and will return Unsupported.
            ExprKind::If { cond, then_block, else_expr } => {
                match ret_ty {
                    IrTy::I32 => {
                        let else_label = self.alloc_label();
                        let end_label = self.alloc_label();

                        // Allocate the phi slot before entering either branch so
                        // both branches write to the same stack location.
                        // Cache-line note: the phi slot is one 8-byte stack entry;
                        // it is read exactly once after the if expression completes.
                        let phi_slot = self.alloc_slot()?;

                        // Lower condition (bool → 0 or 1 in a register).
                        // We pass IrTy::I32 since booleans are represented as integers.
                        let cond_val = self.lower_expr(cond, &IrTy::I32)?;
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
                                // where i32 is expected is a type error — unsupported.
                                return Err(LowerError::Unsupported(
                                    "if expression without else in i32 context".into(),
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
                    _ => Err(LowerError::Unsupported(
                        "if expression with non-i32 return type".into(),
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

            // Anything else: not yet supported as runtime codegen.
            _ => Err(LowerError::Unsupported(
                "expression kind in non-const context (runtime codegen not yet implemented)".into(),
            )),
        }
    }
}
