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

/// An IR instruction.
///
/// Milestone 1 supports only `Ret`. Each new milestone adds exactly the
/// instructions required for the next runnable program.
pub enum Instr {
    /// Return a value to the caller.
    ///
    /// FLS §6.19: Return expressions. On ARM64, the return value is
    /// placed in `x0` and `ret` branches back through the link register.
    Ret(IrValue),
}

// ── Values ────────────────────────────────────────────────────────────────────

/// An IR value — a compile-time constant or (future) virtual register.
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
