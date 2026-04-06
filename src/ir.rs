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
    /// Number of 8-byte stack slots allocated for local variables.
    ///
    /// FLS §8.1: Let statements introduce local variable bindings that
    /// require storage on the stack. Each slot is 8 bytes; the total frame
    /// size is rounded up to 16-byte alignment for the ARM64 ABI.
    ///
    /// Cache-line note: stack slots map to 8-byte chunks — one slot per
    /// half cache-line entry. Future register-allocation passes may
    /// eliminate some stack slots entirely.
    pub stack_slots: u8,

    /// Whether this function calls other functions (i.e., is non-leaf).
    ///
    /// If true, the function prologue must save the link register (x30)
    /// before any `bl` instruction overwrites it, and the epilogue must
    /// restore it before `ret`.
    ///
    /// ARM64 ABI: `bl` sets x30 to the return address, clobbering any
    /// previous value. A leaf function never executes `bl` so x30 remains
    /// valid for the final `ret`. A non-leaf function must save x30.
    ///
    /// FLS §6.12.1: Call expressions imply the function is non-leaf.
    /// Cache-line note: the lr save/restore pair adds 2 instructions (8 bytes)
    /// to the function prologue/epilogue — one half of a 16-byte aligned pair.
    pub saves_lr: bool,
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
/// Milestone 12 adds `Store` and `Load` for let bindings.
/// Milestone 13 adds `Label`, `Branch`, and `CondBranch` for if/else control flow.
/// Milestone 14 adds `Call` for function call expressions (FLS §6.12.1).
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

    /// Store a virtual register to a stack slot.
    ///
    /// `Store { src, slot }` → `str x{src}, [sp, #{slot * 8}]` on ARM64.
    ///
    /// FLS §8.1: Let statement initializers are stored to the local's stack
    /// slot. FLS §6.1.2:37–45: This is a runtime store instruction, not a
    /// compile-time constant.
    ///
    /// Cache-line note: ARM64 `str` is 4 bytes. Each slot is 8 bytes,
    /// so up to 8 slots fit in a single 64-byte cache line on the stack.
    Store {
        /// Source register holding the value to store.
        src: u8,
        /// Destination stack slot index (byte offset = slot * 8).
        slot: u8,
    },

    /// Load a stack slot into a virtual register.
    ///
    /// `Load { dst, slot }` → `ldr x{dst}, [sp, #{slot * 8}]` on ARM64.
    ///
    /// FLS §8.1: Reading a local variable binding accesses its stack slot.
    /// FLS §6.3: Path expressions referring to a local variable lower to Load.
    ///
    /// Cache-line note: ARM64 `ldr` is 4 bytes. Stack slots are 8 bytes each.
    Load {
        /// Destination register to receive the loaded value.
        dst: u8,
        /// Source stack slot index (byte offset = slot * 8).
        slot: u8,
    },

    /// Define a branch target label.
    ///
    /// `Label(n)` emits `.L{n}:` in the assembly output. Labels are referenced
    /// by `Branch` and `CondBranch` instructions.
    ///
    /// FLS §6.17: if expressions require forward labels for the else and end
    /// of the conditional. Labels have no runtime cost — they are assembler
    /// directives that resolve to instruction addresses.
    ///
    /// Cache-line note: labels carry no machine code; they do not consume
    /// space in the instruction stream.
    Label(u32),

    /// Unconditional branch to a label.
    ///
    /// `Branch(n)` → `b .L{n}` on ARM64.
    ///
    /// FLS §6.17: After the then-branch of an if expression, the else-branch
    /// must be skipped via an unconditional branch to the end label.
    ///
    /// Cache-line note: ARM64 `b` is a 4-byte instruction.
    Branch(u32),

    /// Conditional branch: jump to `label` if `reg` is zero (false).
    ///
    /// `CondBranch { reg, label }` → `cbz x{reg}, .L{label}` on ARM64.
    ///
    /// ARM64 `cbz` ("compare and branch if zero") combines a compare-with-zero
    /// and a branch in a single 4-byte instruction, avoiding the need for a
    /// separate `cmp` instruction for boolean conditions.
    ///
    /// FLS §6.17: The condition expression of an if is a boolean value. The
    /// branch jumps to the else block (or past the if body) when the condition
    /// evaluates to `false` (0). FLS §2.4.7: Boolean literals — `false` = 0,
    /// `true` = 1.
    ///
    /// Cache-line note: ARM64 `cbz` is 4 bytes — same footprint as `b`.
    CondBranch {
        /// The virtual register holding the boolean condition (0 = false).
        reg: u8,
        /// The label to branch to when `reg` is zero (condition is false).
        label: u32,
    },

    /// Call a named function with arguments; result goes into a virtual register.
    ///
    /// `Call { dst, name, args }` emits (for each arg[i] ≠ i):
    ///   `mov x{i}, x{args[i]}`
    /// then:
    ///   `bl {name}`
    ///   `mov x{dst}, x0`  (omitted if dst == 0)
    ///
    /// FLS §6.12.1: Call expressions. The callee is a path expression resolved
    /// to a function item. Arguments are evaluated left-to-right (FLS §6.4:14)
    /// and passed in x0–x7 per the ARM64 procedure call standard.
    ///
    /// ARM64 ABI: integer/pointer args 0–7 go in x0–x7; return value in x0.
    /// The link register (x30) is set to the return address by `bl`; the
    /// calling function must save x30 if it makes any calls (see `saves_lr`
    /// on `IrFn`).
    ///
    /// Cache-line note: a call with N args emits at most N+2 instructions
    /// (N moves + bl + mov for result). For the common case of 1–2 args
    /// already in x0–x1, N moves collapse to 0.
    Call {
        /// Destination virtual register for the return value.
        dst: u8,
        /// Name of the function to call.
        name: String,
        /// Argument virtual registers, in left-to-right parameter order.
        /// `args[i]` holds the value to place in `x{i}` before the call.
        args: Vec<u8>,
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
