//! Intermediate representation for galvanic.
//!
//! The IR sits between the AST and ARM64 codegen. It is intentionally
//! minimal for the first milestone: it can represent a function that
//! returns a constant integer value.
//!
//! The design will grow with each milestone program. Nothing is added
//! here before it is needed by the next runnable binary.
//!
//! # FLS traceability
//!
//! - FLS §9: Functions — each `IrFn` maps to one source-level function.
//! - FLS §6.19: Return expressions — `Instr::Ret` is the only terminator.
//! - FLS §2.4.4.1: Integer literals — `IrValue::I32` holds the constant.
//! - FLS §4.4: Unit type — `IrValue::Unit` / `IrTy::Unit`.
//! - FLS §18.1: Program structure — `Module` is the compilation unit.
//!
//! # Cache-line note
//!
//! `Instr` and `IrValue` are small enums. At this milestone they fit
//! comfortably in a single cache line per instruction. The representation
//! will be revisited when the instruction set grows.

#![allow(dead_code)]

// ── Module ────────────────────────────────────────────────────────────────────

/// The top-level IR compilation unit.
///
/// FLS §18.1: A crate is the unit of compilation. The `Module` holds all
/// functions emitted from one source file.
pub struct Module {
    /// The functions defined in this module.
    pub fns: Vec<IrFn>,
}

// ── Functions ─────────────────────────────────────────────────────────────────

/// A function in the IR.
///
/// FLS §9: Functions. Each `IrFn` corresponds to one `fn` item.
pub struct IrFn {
    /// The mangled/resolved function name (e.g., `"main"`).
    pub name: String,
    /// The return type.
    pub ret_ty: IrTy,
    /// The function body — a flat list of instructions.
    ///
    /// For milestone 1 this is always `[Instr::Ret(value)]`. Basic blocks
    /// and control flow instructions will be added in later milestones.
    pub body: Vec<Instr>,
}

// ── Instructions ──────────────────────────────────────────────────────────────

/// An arithmetic binary operation kind for the IR.
///
/// FLS §6.5.5: Arithmetic operator expressions.
/// Cache-line note: fits in 1 byte as a discriminant — negligible overhead.
#[derive(Debug, Clone, Copy)]
pub enum IrBinOp {
    /// Integer addition `+`. FLS §6.5.5.
    Add,
    /// Integer subtraction `-`. FLS §6.5.5.
    Sub,
    /// Integer multiplication `*`. FLS §6.5.5.
    Mul,
}

/// An IR instruction.
///
/// Grows by exactly the instructions needed for each new milestone program.
/// Milestone 11 adds `LoadImm` and `BinOp` to emit real arithmetic code.
pub enum Instr {
    /// Return a value to the caller.
    ///
    /// FLS §6.19: Return expressions. On ARM64, the return value is
    /// placed in `x0` and `ret` branches back through the link register.
    Ret(IrValue),

    /// Load an integer immediate into a virtual register.
    ///
    /// `LoadImm(dst, n)` → `mov x{dst}, #{n}` on ARM64.
    ///
    /// FLS §2.4.4.1: Integer literal expressions used as arithmetic operands
    /// must be materialized at runtime, not folded to a single constant.
    /// FLS §6.1.2:37–45: Non-const code must emit runtime instructions.
    ///
    /// Cache-line note: ARM64 `mov` (MOVZ) is 4 bytes — one instruction slot.
    LoadImm(u8, i32),

    /// Integer binary arithmetic: `dst = lhs op rhs`.
    ///
    /// `BinOp { Add, d, l, r }` → `add x{d}, x{l}, x{r}` on ARM64.
    ///
    /// FLS §6.5.5: Arithmetic operator expressions. The operands and result
    /// are in virtual registers; register allocation is trivially sequential
    /// at this milestone (virtual register N → ARM64 register xN).
    ///
    /// Cache-line note: one ARM64 instruction (4 bytes) per BinOp.
    BinOp {
        /// The arithmetic operation to perform.
        op: IrBinOp,
        /// Destination register (receives the result).
        dst: u8,
        /// Left-hand operand register.
        lhs: u8,
        /// Right-hand operand register.
        rhs: u8,
    },
}

// ── Values ────────────────────────────────────────────────────────────────────

/// An IR value — a compile-time constant or a virtual register reference.
#[derive(Debug, Clone, Copy)]
pub enum IrValue {
    /// A 32-bit signed integer constant.
    ///
    /// FLS §2.4.4.1: Integer literals. Narrowed from the parser's `u128`
    /// to `i32` during lowering once the type is known.
    ///
    /// ARM64 note: `i32` constants up to 16 bits emit as `mov x0, #n`
    /// (a single `MOVZ` instruction, 4 bytes, always cache-line-aligned).
    /// Larger values require `MOVZ`/`MOVK` sequences — future work.
    I32(i32),

    /// The unit value `()`.
    ///
    /// FLS §4.4: The unit type has exactly one value, also written `()`.
    /// On ARM64, a unit return leaves `x0` unspecified; by convention we
    /// emit `mov x0, #0` to produce a clean exit code for `main`.
    Unit,

    /// A value held in virtual register N.
    ///
    /// FLS §6.5: The result of an arithmetic expression is held in a virtual
    /// register. At this milestone, virtual register N maps directly to ARM64
    /// register `x{N}` (trivial register allocation — no spilling).
    ///
    /// Cache-line note: `u8` occupies 1 byte; the discriminant keeps `IrValue`
    /// small so it fits alongside other data in a cache line.
    Reg(u8),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// An IR type.
///
/// Minimal set for milestone 1. Grows with each new milestone program.
pub enum IrTy {
    /// The `i32` type. FLS §4.1.
    I32,
    /// The unit type `()`. FLS §4.4.
    Unit,
}
