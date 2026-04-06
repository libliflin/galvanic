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

/// An arithmetic or comparison binary operation kind for the IR.
///
/// FLS §6.5.5: Arithmetic operator expressions.
/// FLS §6.5.3: Comparison operator expressions.
/// Cache-line note: fits in 1 byte as a discriminant — negligible overhead.
#[derive(Debug, Clone, Copy)]
pub enum IrBinOp {
    /// Integer addition `+`. FLS §6.5.5.
    Add,
    /// Integer subtraction `-`. FLS §6.5.5.
    Sub,
    /// Integer multiplication `*`. FLS §6.5.5.
    Mul,
    /// Signed less-than `<`. FLS §6.5.3. Result: 1 if true, 0 if false.
    Lt,
    /// Signed less-than-or-equal `<=`. FLS §6.5.3.
    Le,
    /// Signed greater-than `>`. FLS §6.5.3.
    Gt,
    /// Signed greater-than-or-equal `>=`. FLS §6.5.3.
    Ge,
    /// Equality `==`. FLS §6.5.3.
    Eq,
    /// Inequality `!=`. FLS §6.5.3.
    Ne,
    /// Signed integer division `/`. FLS §6.5.5.
    ///
    /// ARM64: `sdiv x{dst}, x{lhs}, x{rhs}`.
    /// FLS §6.23: Division by zero panics at runtime (debug mode). Galvanic
    /// does not yet insert a divide-by-zero check — this is FLS §6.23 AMBIGUOUS:
    /// the spec requires a panic but the mechanism is unspecified.
    Div,
    /// Signed integer remainder `%`. FLS §6.5.5.
    ///
    /// Computed as `lhs - (lhs / rhs) * rhs` using `sdiv` + `msub`.
    /// ARM64: two instructions — see codegen for details.
    Rem,

    /// Bitwise AND `&`. FLS §6.5.6.
    ///
    /// ARM64: `and x{dst}, x{lhs}, x{rhs}`.
    /// Cache-line note: one 4-byte instruction per BitAnd.
    BitAnd,

    /// Bitwise OR `|`. FLS §6.5.6.
    ///
    /// ARM64: `orr x{dst}, x{lhs}, x{rhs}`.
    /// Cache-line note: one 4-byte instruction per BitOr.
    BitOr,

    /// Bitwise XOR `^`. FLS §6.5.6.
    ///
    /// ARM64: `eor x{dst}, x{lhs}, x{rhs}`.
    /// Cache-line note: one 4-byte instruction per BitXor.
    BitXor,

    /// Left shift `<<`. FLS §6.5.7.
    ///
    /// ARM64: `lsl x{dst}, x{lhs}, x{rhs}` (logical shift left).
    /// FLS §6.5.7: The shift amount is taken modulo the bit width (64 on ARM64).
    /// Cache-line note: one 4-byte instruction per Shl.
    Shl,

    /// Arithmetic right shift `>>`. FLS §6.5.7.
    ///
    /// ARM64: `asr x{dst}, x{lhs}, x{rhs}` (arithmetic shift right).
    /// Signed integers use arithmetic shift (sign-extending) per FLS §6.5.7.
    /// FLS §6.5.7: The shift amount is taken modulo the bit width (64 on ARM64).
    /// Cache-line note: one 4-byte instruction per Shr.
    Shr,
}

/// An IR instruction.
///
/// Grows by exactly the instructions needed for each new milestone program.
/// Milestone 11 adds `LoadImm` and `BinOp` to emit real arithmetic code.
/// Milestone 12 adds `Store` and `Load` for let bindings.
/// Milestone 13 adds `Label`, `Branch`, and `CondBranch` for if/else control flow.
/// Milestone 14 adds `Call` for function call expressions (FLS §6.12.1).
/// Milestone 16 adds comparison ops to `IrBinOp` and while loop lowering.
/// Milestone 21 adds bitwise and shift ops to `IrBinOp` (FLS §6.5.6, §6.5.7).
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

    /// Arithmetic negation: `dst = -src` (two's complement).
    ///
    /// `Neg { dst, src }` → `neg x{dst}, x{src}` on ARM64.
    ///
    /// FLS §6.5.4: Negation operator expressions. The unary `-` applied to
    /// a numeric value produces its arithmetic negation. For `i32`, this is
    /// two's complement negation: `neg xD, xS` ≡ `sub xD, xzr, xS`.
    ///
    /// FLS §6.1.2:37–45: Even `-literal` in a non-const context must emit a
    /// runtime instruction, not fold to a negative immediate.
    ///
    /// Cache-line note: ARM64 `neg` is a 4-byte instruction (alias for sub xD, xzr, xS).
    Neg {
        /// Destination register (receives the negated value).
        dst: u8,
        /// Source register (holds the value to negate).
        src: u8,
    },

    /// Bitwise NOT: `dst = !src` (complement all bits).
    ///
    /// `Not { dst, src }` → `mvn x{dst}, x{src}` on ARM64.
    ///
    /// FLS §6.5.4: Negation operator expressions. The unary `!` applied to
    /// an integer value produces its bitwise complement. For `i32`, this flips
    /// all 32 bits: `!0` = `-1`, `!5` = `-6`.
    ///
    /// FLS §6.5.4: "The type of a negation expression is the type of the operand."
    ///
    /// FLS §6.1.2:37–45: Even `!literal` in a non-const context must emit a
    /// runtime instruction, not fold to a complemented immediate.
    ///
    /// Cache-line note: ARM64 `mvn` is a 4-byte instruction (alias for orn xD, xzr, xS).
    Not {
        /// Destination register (receives the bitwise complement).
        dst: u8,
        /// Source register (holds the value to complement).
        src: u8,
    },

    /// Logical NOT: `dst = !src` (0 → 1, 1 → 0), for boolean operands.
    ///
    /// `BoolNot { dst, src }` → `eor x{dst}, x{src}, #1` on ARM64.
    ///
    /// FLS §6.5.4: Negation operator expressions. The unary `!` applied to
    /// a `bool` value produces its logical complement: `!true` = `false` (0),
    /// `!false` = `true` (1).
    ///
    /// ARM64: `eor xD, xS, #1` XORs the source with the immediate 1,
    /// flipping only the least-significant bit. Since booleans are represented
    /// as 0 or 1, this produces the correct logical complement in a single
    /// 4-byte instruction — more efficient than the two-instruction alternative
    /// of `LoadImm(1)` + `BinOp(BitXor)`.
    ///
    /// FLS §6.1.2:37–45: Runtime instruction — `!b` always emits `eor`,
    /// even when `b` is statically known.
    ///
    /// Cache-line note: ARM64 `eor` with logical immediate is 4 bytes —
    /// same footprint as `mvn` (bitwise NOT for integers).
    BoolNot {
        /// Destination register (receives the logical complement 0 or 1).
        dst: u8,
        /// Source register (holds the bool value 0 or 1 to negate).
        src: u8,
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

    /// Return from a `&mut self` method, writing modified fields back to the caller.
    ///
    /// `RetFields { base_slot, n_fields }` emits:
    ///   `ldr x0, [sp, #{base_slot*8}]`
    ///   `ldr x1, [sp, #{(base_slot+1)*8}]`
    ///   ... (one per field)
    ///   epilogue (sp restore if frame_size > 0, lr restore if saves_lr)
    ///   `ret`
    ///
    /// This instruction is emitted by `lower_fn` for `&mut self` methods with
    /// unit return type. The caller uses `CallMut` to read x0..x{N-1} back
    /// into the struct's stack slots after the `bl`.
    ///
    /// FLS §10.1: `&mut self` methods must propagate mutations to the caller.
    /// ARM64 ABI: small aggregates returned in x0..x{N-1}.
    ///
    /// Limitation: early `return` inside a `&mut self` method body bypasses
    /// this write-back. Only methods that terminate via their tail expression
    /// are fully supported at this milestone.
    ///
    /// Cache-line note: N field loads emit N × 4-byte `ldr` instructions.
    /// For a 1-field struct this is 4 bytes (fits in any cache line slot);
    /// for an 8-field struct this is 32 bytes (half a cache line).
    RetFields {
        /// Stack slot of the first self field. Always 0 for methods, since
        /// self fields are spilled first in `lower_fn`.
        base_slot: u8,
        /// Number of struct fields to write back. 0 for unit structs (acts
        /// like a plain `Ret(Unit)`).
        n_fields: u8,
    },

    /// Load from a stack-allocated array at a runtime-computed index.
    ///
    /// `LoadIndexed { dst, base_slot, index_reg }` emits:
    ///   `add x{dst}, sp, #(base_slot * 8)`       // address of arr[0]
    ///   `ldr x{dst}, [x{dst}, x{index_reg}, lsl #3]` // load arr[index]
    ///
    /// The `lsl #3` scales the index by 8 (the size of one i32 stack slot).
    ///
    /// FLS §6.9: Indexing expressions. The base is a stack-allocated array
    /// (consecutive 8-byte slots); the index selects which slot to load.
    ///
    /// FLS §6.9 AMBIGUOUS: The spec requires bounds checking (panic on
    /// out-of-bounds access), but does not specify the panic mechanism.
    /// Galvanic does not emit bounds checks at this milestone.
    ///
    /// ARM64 LDR addressing: `[xB, xI, lsl #3]` reads from address `xB + xI*8`.
    /// The `lsl #3` extension is encoded in the instruction and has zero extra cost.
    ///
    /// Cache-line note: two 4-byte instructions (8 bytes) per indexed load.
    /// The add+ldr pair fits in the same pair of instruction slots in a
    /// 64-byte cache line, so the base address computation and load are
    /// fetched together.
    LoadIndexed {
        /// Destination register for the loaded element.
        dst: u8,
        /// Stack slot index of the first array element (slot 0).
        base_slot: u8,
        /// Register holding the runtime array index (0-based).
        index_reg: u8,
    },

    /// Call a `&mut self` method and write modified fields back to the caller's struct.
    ///
    /// `CallMut { name, args, write_back_slot, n_fields }` emits:
    ///   `mov x{i}, x{args[i]}` (for each arg not already in the right register)
    ///   `bl {name}`
    ///   `str x0, [sp, #{write_back_slot*8}]`
    ///   `str x1, [sp, #{(write_back_slot+1)*8}]`
    ///   ... (one per field, since the callee returned them in x0..x{N-1})
    ///
    /// After the `bl`, x0..x{N-1} hold the method's modified field values
    /// (returned via `RetFields`). They are immediately written back to the
    /// caller's struct stack slots so that subsequent field reads see the
    /// updated values.
    ///
    /// FLS §6.12.2: Method call expressions.
    /// FLS §10.1: `&mut self` mutation must be visible to the caller.
    ///
    /// Cache-line note: N field stores emit N × 4-byte `str` instructions
    /// after the `bl`. Paired with the `ldr` sequence in `RetFields` on the
    /// callee side, each mutation costs 2N + 1 extra instructions (N loads,
    /// N stores, 1 bl) beyond a value-copy call.
    CallMut {
        /// Name of the `&mut self` method to call (mangled).
        name: String,
        /// Argument registers in call order: struct fields first (x0..x{N-1}),
        /// then any explicit arguments.
        args: Vec<u8>,
        /// Base stack slot of the receiver struct in the caller's frame.
        write_back_slot: u8,
        /// Number of struct fields to write back (same as struct field count).
        n_fields: u8,
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
///
/// `Clone` and `Copy` are derived so that `LowerCtx` can store the function
/// return type and pass it to `return` expression lowering without borrow
/// conflicts (FLS §6.19).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IrTy {
    /// The `i32` type. FLS §4.1.
    I32,
    /// The unit type `()`. FLS §4.4.
    Unit,
    /// The boolean type `bool`. FLS §4.3.
    ///
    /// Represented as a 0/1 integer in a 64-bit register, but kept distinct
    /// from `IrTy::I32` so that `!` can emit logical NOT (`eor reg, #1`)
    /// rather than bitwise NOT (`mvn`). On ARM64, `bool` uses the same
    /// register layout as `i32` — no extra cost.
    ///
    /// FLS §4.3: "The boolean type bool has two values: true and false."
    /// FLS §6.5.4: `!` on bool is logical NOT; `!` on integer is bitwise NOT.
    ///
    /// Cache-line note: same register width as `IrTy::I32`.
    Bool,
}
