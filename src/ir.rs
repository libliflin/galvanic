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

/// The typed initializer value of a static item.
///
/// FLS §7.2: Static items. Galvanic supports integer, f64, and f32 literals
/// as static initializers (FLS §6.1.2: constant expressions).
///
/// Cache-line note: `StaticValue` is a small enum; the discriminant fits in
/// 1 byte and the payload is at most 8 bytes (f64). The enum fits in 16 bytes.
#[derive(Debug, Clone, Copy)]
pub enum StaticValue {
    /// Integer static (FLS §4.1). Stored as `.quad` (8 bytes) in `.data`.
    Int(i32),
    /// 64-bit float static (FLS §4.2). Stored as `.quad` with raw IEEE 754 bits.
    F64(f64),
    /// 32-bit float static (FLS §4.2). Stored as `.word` with raw IEEE 754 bits.
    F32(f32),
}

/// A static variable in the data section.
///
/// FLS §7.2: Static items. Each static has a fixed memory address in the
/// `.data` (mutable) or `.rodata` (immutable) section. Galvanic emits all
/// statics into `.data` at this milestone.
///
/// FLS §7.2:15: "All references to a static refer to the same memory address."
///
/// Cache-line note: each static occupies 8 bytes in the `.data` section.
/// Eight statics fit in one 64-byte data cache line. Reading a static costs
/// an ADRP + ADD + LDR sequence (12 bytes in the instruction stream),
/// whereas a `const` costs a single MOV (4 bytes) — the primary cache-line
/// tradeoff documented in galvanic's design.
pub struct StaticData {
    /// The assembly label for this static (matches the Rust name).
    pub name: String,
    /// The compile-time initializer value (FLS §6.1.2: constant expressions).
    pub value: StaticValue,
}

/// The top-level IR compilation unit.
///
/// FLS §18.1: A crate is the unit of compilation. The `Module` holds all
/// functions emitted from one source file.
pub struct Module {
    /// The functions defined in this module.
    pub fns: Vec<IrFn>,
    /// Static variables in the data section.
    ///
    /// FLS §7.2: Static items. Populated during lowering when the source
    /// file contains `static` declarations.
    pub statics: Vec<StaticData>,
    /// Trampoline functions generated when capturing closures are passed as
    /// `impl Fn` arguments.
    ///
    /// FLS §6.22, §4.13: A capturing closure has signature
    /// `(cap0, cap1, …, arg0, arg1, …) -> R`, which differs from the
    /// `impl Fn(arg0, …) -> R` signature the callee expects. A trampoline
    /// bridges the gap: it has the expected arity, reads captured values
    /// from ARM64 callee-saved registers (x27, x26, …) that the caller
    /// loaded before the `bl`, and tail-calls the actual closure function.
    ///
    /// Cache-line note: each trampoline is 3–6 instructions (12–24 bytes),
    /// fitting in less than half a 64-byte cache line.
    pub trampolines: Vec<ClosureTrampoline>,

    /// Vtable shim functions for `dyn Trait` dispatch. FLS §4.13.
    ///
    /// Each shim adapts the "fields spread across registers" calling convention
    /// used by galvanic's struct methods to the "data pointer in x0" convention
    /// expected by vtable callers. A shim for method `m` on type `T` with N
    /// fields:
    ///   1. Receives x0 = ptr to T's data on the caller's stack frame.
    ///   2. Loads fields from offsets 0, 8, 16, … in reverse order (field N-1
    ///      first so that overwriting x0 with field 0 is safe).
    ///   3. Tail-calls `T__m` with the fields in x0..x{N-1}.
    ///
    /// Cache-line note: N+1 instructions (N loads + 1 branch), ≤ 20 bytes
    /// for typical 4-field structs — fits in a single 64-byte cache line.
    pub vtable_shims: Vec<VtableShim>,

    /// Vtable data sections for `dyn Trait`. FLS §4.13.
    ///
    /// One `VtableSpec` per (trait, concrete_type) pair used in the program.
    /// Each vtable is a read-only array of function pointer addresses, one
    /// per trait method in declaration order.
    ///
    /// Cache-line note: each vtable occupies N × 8 bytes in `.rodata`.
    /// For a single-method trait, the vtable fits in one 64-byte cache line
    /// alongside 7 other vtables.
    pub vtables: Vec<VtableSpec>,
}

/// A vtable shim for `dyn Trait` dispatch. FLS §4.13.
///
/// Bridges the gap between the vtable call convention (receives a single data
/// pointer in x0) and galvanic's struct method convention (receives each field
/// in a separate register x0..x{N-1}).
///
/// ARM64 design: loads fields from the data pointer in reverse order (highest
/// index first) to avoid clobbering x0 (which holds the pointer) before field
/// 0 is loaded. Uses a tail-call (`b`, not `bl`) so the return from the
/// target method goes directly back to the vtable caller.
///
/// Cache-line note: (n_fields + 1) × 4-byte instructions. For a 1-field
/// struct: 2 instructions (8 bytes). For a 4-field struct: 5 instructions
/// (20 bytes). All fit in a single 64-byte instruction cache line.
pub struct VtableShim {
    /// Assembly label for this shim (e.g., `vtable_shim_MyTrait_MyStruct_0`).
    pub name: String,
    /// Mangled name of the target method (e.g., `MyStruct__my_method`).
    pub target: String,
    /// Number of integer fields in the concrete struct type.
    ///
    /// Each field occupies one register (x0..x{n-1}) after the shim loads
    /// them from the data pointer. Float fields are not supported at this
    /// milestone (dyn Trait dispatch for float-field structs is deferred).
    pub n_fields: usize,
}

/// A vtable data record for one (trait, concrete_type) pair. FLS §4.13.
///
/// Emitted as a read-only array of function pointer addresses in `.rodata`.
/// Indexed by method position (0 = first method in trait declaration order).
///
/// Cache-line note: N × 8 bytes in `.rodata`, 8-byte aligned.
pub struct VtableSpec {
    /// Assembly label for this vtable (e.g., `vtable_MyTrait_MyStruct`).
    pub label: String,
    /// Shim labels in trait method declaration order.
    ///
    /// `method_shim_labels[i]` is the address emitted at offset `i * 8`
    /// in the vtable. The vtable caller loads this address and calls via `blr`.
    pub method_shim_labels: Vec<String>,
}

/// A trampoline for passing a capturing closure as an `impl Fn` argument.
///
/// FLS §6.22, §4.13: When a capturing closure `move |x| x + offset` is
/// passed to a function `apply(f: impl Fn(i32) -> i32, …)`, `apply` calls
/// `f(x)` with only the explicit argument. The trampoline sits between
/// `apply` and the closure: it receives the explicit argument in x0, reads
/// the captures from callee-saved registers x27/x26/… (set by the original
/// caller before `bl apply`), and tail-calls the closure with captures first.
///
/// ARM64 callee-saved registers are preserved across function calls
/// (x19–x28), so x27 set in `main` before `bl apply` is still valid when
/// `apply` calls the trampoline via `blr x9`.
pub struct ClosureTrampoline {
    /// Unique name for this trampoline function (e.g., `__closure_main_0_trampoline`).
    pub name: String,
    /// Name of the actual closure function this trampoline forwards to.
    pub closure_name: String,
    /// Number of captured variables. Each capture occupies one callee-saved
    /// register: cap 0 → x27, cap 1 → x26, cap 2 → x25, etc.
    pub n_caps: usize,
    /// Number of explicit arguments the closure expects (its parameter count).
    pub n_explicit: usize,
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

    /// Float (f64) constant pool for this function.
    ///
    /// FLS §2.4.4.2: Float literal expressions. Each entry stores the raw
    /// IEEE 754 bits of a 64-bit float constant referenced in this function's
    /// body via `Instr::LoadF64Const`. Constants are emitted in the `.rodata`
    /// section with labels `{fn_name}__fc{idx}`.
    ///
    /// Cache-line note: each constant occupies one 8-byte `.quad` — identical
    /// to a static item. Unlike statics, float constants are read-only and
    /// share the instruction-stream locality of the function that uses them.
    pub float_consts: Vec<u64>,

    /// Float (f32) constant pool for this function.
    ///
    /// FLS §2.4.4.2: Float literal expressions. Each entry stores the raw
    /// IEEE 754 bits of a 32-bit float constant referenced in this function's
    /// body via `Instr::LoadF32Const`. Constants are emitted in the `.rodata`
    /// section with labels `{fn_name}__f32c{idx}`.
    ///
    /// Cache-line note: each f32 constant occupies one 4-byte `.word` — half
    /// the footprint of an f64 constant. Two f32 constants fit in one 8-byte
    /// cache-line slot.
    pub float32_consts: Vec<u32>,
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
    /// inserts a `cbz` zero-divisor guard (Claim 4o) and a `cmn`/`cmp` signed-
    /// overflow guard for i32::MIN / -1 (Claim 4q) before every `sdiv`.
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

    /// Unsigned integer division `/`. FLS §6.5.5.
    ///
    /// Used when the operand type is unsigned (`u8`, `u16`, `u32`, `u64`, `usize`).
    /// ARM64: `udiv x{dst}, x{lhs}, x{rhs}`.
    /// FLS §4.1: Unsigned integers wrap on division by zero — galvanic does not
    /// yet insert the divide-by-zero check (FLS §6.23 AMBIGUOUS on the mechanism).
    UDiv,

    /// Logical (unsigned) right shift `>>`. FLS §6.5.7.
    ///
    /// Used when the operand type is unsigned (`u8`, `u16`, `u32`, `u64`, `usize`).
    /// ARM64: `lsr x{dst}, x{lhs}, x{rhs}` (logical shift right, zero-extending).
    /// Unsigned integers use logical shift per FLS §6.5.7.
    /// Cache-line note: one 4-byte instruction per UShr.
    UShr,
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

    /// IEEE 754 double-precision negation.
    ///
    /// `FNegF64 { dst, src }` → `fneg d{dst}, d{src}` on ARM64.
    ///
    /// FLS §6.5.4: The unary `-` applied to an `f64` value produces its
    /// arithmetic negation, flipping the IEEE 754 sign bit.
    ///
    /// FLS §6.1.2:37–45: Even `-2.5_f64` in a non-const context must emit
    /// a runtime `fneg` instruction.
    ///
    /// Cache-line note: one 4-byte ARM64 FNEG instruction.
    FNegF64 {
        /// Destination float register (ARM64 `d{dst}`).
        dst: u8,
        /// Source float register (ARM64 `d{src}`).
        src: u8,
    },

    /// IEEE 754 single-precision negation.
    ///
    /// `FNegF32 { dst, src }` → `fneg s{dst}, s{src}` on ARM64.
    ///
    /// FLS §6.5.4: The unary `-` applied to an `f32` value produces its
    /// arithmetic negation, flipping the IEEE 754 sign bit.
    ///
    /// Cache-line note: one 4-byte ARM64 FNEG instruction.
    FNegF32 {
        /// Destination single-precision float register (ARM64 `s{dst}`).
        dst: u8,
        /// Source single-precision float register (ARM64 `s{src}`).
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

    /// Truncate a register to 8 unsigned bits. Milestone 176.
    ///
    /// `TruncU8 { dst, src }` → `and w{dst}, w{src}, #255` on ARM64.
    ///
    /// Implements the FLS §6.23 wrapping semantics for `u8`: after arithmetic
    /// the result is masked to the low 8 bits so that, e.g., 200_u8 + 100_u8
    /// yields 44 (= 300 mod 256) rather than 300.
    ///
    /// Emitted at every u8 function return and explicit `return` expression.
    /// Intermediate u8 values within a function are not truncated (deferred).
    ///
    /// FLS §4.1: "The unsigned integer types have a range of [0, 2^N - 1]."
    /// FLS §6.23: Runtime wrapping semantics for non-const integer arithmetic.
    ///
    /// Cache-line note: one 4-byte ARM64 `and` instruction per truncation.
    TruncU8 {
        /// Destination register (receives low 8 bits of src).
        dst: u8,
        /// Source register (holds the value to truncate).
        src: u8,
    },

    /// Sign-extend a register from 8 signed bits. Milestone 177.
    ///
    /// `SextI8 { dst, src }` → `sxtb w{dst}, w{src}` on ARM64.
    ///
    /// Implements the FLS §6.23 wrapping semantics for `i8`: after arithmetic
    /// the result is sign-extended from 8 bits so that, e.g., 100_i8 + 50_i8
    /// yields -106 (= 150 - 256) rather than 150.
    ///
    /// Emitted at every i8 function return and explicit `return` expression.
    /// Intermediate i8 values within a function are not sign-extended (deferred).
    ///
    /// FLS §4.1: "The signed integer types have a range of [-2^(N-1), 2^(N-1)-1]."
    /// FLS §6.23: Runtime wrapping semantics for non-const integer arithmetic.
    ///
    /// Cache-line note: one 4-byte ARM64 `sxtb` instruction per sign-extension.
    SextI8 {
        /// Destination register (receives sign-extended low 8 bits of src).
        dst: u8,
        /// Source register (holds the value to sign-extend).
        src: u8,
    },

    /// Truncate a register to 16 unsigned bits. Milestone 180.
    ///
    /// `TruncU16 { dst, src }` → `and w{dst}, w{src}, #65535` on ARM64.
    ///
    /// Implements the FLS §6.5.9 narrowing cast semantics for `x as u16`:
    /// the result is the low 16 bits of the source. For example, 70000_i32 as u16
    /// yields 4464 (= 70000 mod 65536) rather than 70000.
    ///
    /// FLS §4.1: "The unsigned integer types have a range of [0, 2^N - 1]."
    /// FLS §6.5.9: Narrowing integer casts truncate to the target type's bit width.
    ///
    /// Cache-line note: one 4-byte ARM64 `and` instruction per truncation.
    TruncU16 {
        /// Destination register (receives low 16 bits of src).
        dst: u8,
        /// Source register (holds the value to truncate).
        src: u8,
    },

    /// Sign-extend a register from 16 signed bits. Milestone 180.
    ///
    /// `SextI16 { dst, src }` → `sxth x{dst}, w{src}` on ARM64.
    ///
    /// Implements the FLS §6.5.9 narrowing cast semantics for `x as i16`:
    /// the result is the low 16 bits sign-extended to 64 bits. For example,
    /// 40000_i32 as i16 yields -25536 (= 40000 - 65536) because bit 15 is set.
    ///
    /// FLS §4.1: "The signed integer types have a range of [-2^(N-1), 2^(N-1)-1]."
    /// FLS §6.5.9: Narrowing signed integer casts sign-extend from the target width.
    ///
    /// Cache-line note: one 4-byte ARM64 `sxth` instruction per sign-extension.
    SextI16 {
        /// Destination register (receives sign-extended low 16 bits of src).
        dst: u8,
        /// Source register (holds the value to sign-extend).
        src: u8,
    },

    /// Call a named function with arguments; result goes into a virtual register.
    ///
    /// `Call { dst, name, args, float_args }` emits (for each arg[i] ≠ i):
    ///   `mov x{i}, x{args[i]}`
    /// then (for each float_args[i] ≠ i):
    ///   `fmov d{i}, d{float_args[i]}`
    /// then:
    ///   `bl {name}`
    ///   `mov x{dst}, x0`  (omitted if dst == 0)
    ///
    /// FLS §6.12.1: Call expressions. The callee is a path expression resolved
    /// to a function item. Arguments are evaluated left-to-right (FLS §6.4:14)
    /// and passed in x0–x7 (integer) and d0–d7 (float) per the ARM64 ABI.
    ///
    /// ARM64 ABI: integer/pointer args 0–7 go in x0–x7; float args 0–7 go in
    /// d0–d7 (f64) or s0–s7 (f32); integer return value in x0.
    /// The link register (x30) is set to the return address by `bl`; the
    /// calling function must save x30 if it makes any calls (see `saves_lr`
    /// on `IrFn`).
    ///
    /// Cache-line note: a call with N integer + M float args emits at most
    /// N+M+2 instructions. For the common case of args already in place,
    /// moves collapse to 0.
    Call {
        /// Destination virtual register for the return value.
        dst: u8,
        /// Name of the function to call.
        name: String,
        /// Integer argument virtual registers, in left-to-right parameter order.
        /// `args[i]` holds the value to place in `x{i}` before the call.
        args: Vec<u8>,
        /// Float argument virtual registers, in left-to-right float-param order.
        /// `float_args[i]` holds the value to place in `d{i}` before the call.
        /// FLS §4.2: f64/f32 parameters use the ARM64 float register bank.
        float_args: Vec<u8>,
        /// FLS §4.2: float return type.
        /// `None` = integer/unit return (captured from x0).
        /// `Some(true)` = f64 return (captured from d0 into d{dst}).
        /// `Some(false)` = f32 return (captured from s0 into s{dst}).
        float_ret: Option<bool>,
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
        /// Number of elements in the array; used to emit a bounds check.
        /// `0` means no bounds check (e.g., for-loop accesses already guarded by loop condition).
        /// FLS §6.9: Out-of-bounds access must panic at runtime.
        len: u32,
    },

    /// Store a value into an array element: `arr[index] = src`.
    ///
    /// `StoreIndexed { src, base_slot, index_reg, scratch }` emits:
    ///   `add x{scratch}, sp, #{base_slot*8}` — base address of arr[0]
    ///   `str x{src}, [x{scratch}, x{index_reg}, lsl #3]` — store at arr[index]
    ///
    /// FLS §6.5.10: Assignment to an indexed place expression.
    /// FLS §6.9: The index operand selects an element of the array.
    ///
    /// The `lsl #3` scales the index by 8 (bytes per i64 slot), matching the
    /// layout established by array literal lowering (one slot per element).
    ///
    /// FLS §6.9 AMBIGUOUS: Out-of-bounds store must panic, but the panic
    /// mechanism without the standard library is not specified. No bounds
    /// check is emitted at this milestone.
    ///
    /// FLS §6.1.2:37–45: All instructions are runtime — no constant folding.
    ///
    /// Cache-line note: two 4-byte instructions (add + str) = 8 bytes,
    /// mirroring `LoadIndexed`. The add+str pair fits in one adjacent
    /// instruction slot pair in a 64-byte cache line.
    StoreIndexed {
        /// Register holding the value to store.
        src: u8,
        /// Stack slot index of the first array element (slot 0).
        base_slot: u8,
        /// Register holding the runtime array index (0-based).
        index_reg: u8,
        /// Scratch register for base address computation (must not alias src or index_reg).
        scratch: u8,
        /// Number of elements in the array; used to emit a bounds check.
        /// `0` means no bounds check.
        /// FLS §6.9: Out-of-bounds store must panic at runtime.
        len: u32,
    },

    /// Load an f64 element from a float array: `dst = arr[index]` where arr is `[f64; N]`.
    ///
    /// `LoadIndexedF64 { dst, base_slot, index_reg }` emits:
    ///   `add x9, sp, #{base_slot*8}`                   // base address of arr[0]
    ///   `ldr d{dst}, [x9, x{index_reg}, lsl #3]`       // load f64 arr[index]
    ///
    /// FLS §6.9: Indexing expressions. FLS §4.5: Array types. FLS §4.2: f64 in d-registers.
    /// FLS §6.1.2:37–45: All instructions are runtime.
    ///
    /// Cache-line note: add + ldr = two 4-byte instructions = 8 bytes,
    /// matching the integer `LoadIndexed` footprint.
    LoadIndexedF64 {
        /// Destination d-register for the loaded f64 element.
        dst: u8,
        /// Stack slot index of the first array element (slot 0).
        base_slot: u8,
        /// Register holding the runtime array index (0-based).
        index_reg: u8,
        /// Number of elements in the array; used to emit a bounds check.
        /// `0` means no bounds check (e.g., for-loop accesses already guarded by loop condition).
        /// FLS §6.9: Out-of-bounds access must panic at runtime.
        len: u32,
    },

    /// Load an f32 element from a float array: `dst = arr[index]` where arr is `[f32; N]`.
    ///
    /// `LoadIndexedF32 { dst, base_slot, index_reg }` emits:
    ///   `add x9, sp, #{base_slot*8}`                   // base address of arr[0]
    ///   `ldr s{dst}, [x9, x{index_reg}, lsl #3]`       // load f32 arr[index]
    ///
    /// FLS §6.9: Indexing expressions. FLS §4.5: Array types. FLS §4.2: f32 in s-registers.
    /// FLS §6.1.2:37–45: All instructions are runtime.
    ///
    /// Cache-line note: add + ldr = two 4-byte instructions = 8 bytes.
    LoadIndexedF32 {
        /// Destination s-register for the loaded f32 element.
        dst: u8,
        /// Stack slot index of the first array element (slot 0).
        base_slot: u8,
        /// Register holding the runtime array index (0-based).
        index_reg: u8,
        /// Number of elements in the array; used to emit a bounds check.
        /// `0` means no bounds check (e.g., for-loop accesses already guarded by loop condition).
        /// FLS §6.9: Out-of-bounds access must panic at runtime.
        len: u32,
    },

    /// Load from a static variable in the data section.
    ///
    /// `LoadStatic { dst, name }` emits:
    ///   `adrp x{dst}, {name}`
    ///   `add x{dst}, x{dst}, :lo12:{name}`
    ///   `ldr x{dst}, [x{dst}]`
    ///
    /// FLS §7.2: Static items. All references to a static refer to the same
    /// memory address — unlike `const` (which substitutes a value), a static
    /// reference must load from the data section at runtime.
    ///
    /// ARM64 addressing: ADRP loads the page-aligned base address into the
    /// register; the ADD applies the page offset (:lo12:) to form the full
    /// 64-bit address; LDR loads the 64-bit value from that address.
    ///
    /// Cache-line note: three 4-byte instructions (12 bytes) per static load.
    /// A `const` load is one instruction (4 bytes). The extra 8 bytes bring
    /// the static-load sequence to exactly one half of a 64-byte instruction
    /// cache line, while the value itself occupies one half of a data cache line.
    LoadStatic {
        /// Destination register for the loaded value.
        dst: u8,
        /// The assembly label of the static (same as the Rust static name).
        name: String,
    },

    /// Load from an f64 static variable in the data section into a float register.
    ///
    /// `LoadStaticF64 { dst, name }` emits:
    ///   `adrp x17, {name}`
    ///   `add  x17, x17, :lo12:{name}`
    ///   `ldr  d{dst}, [x17]`
    ///
    /// FLS §7.2: Static items. FLS §4.2: f64 type.
    /// Like `LoadStatic` but loads into the SIMD/FP register `d{dst}` instead
    /// of integer register `x{dst}`.
    ///
    /// Cache-line note: three 4-byte instructions (12 bytes); identical footprint
    /// to integer `LoadStatic`. The data value is 8 bytes in `.data`.
    LoadStaticF64 {
        /// Destination float register (d0–d15).
        dst: u8,
        /// The assembly label of the static (same as the Rust static name).
        name: String,
    },

    /// Load from an f32 static variable in the data section into a float register.
    ///
    /// `LoadStaticF32 { dst, name }` emits:
    ///   `adrp x17, {name}`
    ///   `add  x17, x17, :lo12:{name}`
    ///   `ldr  s{dst}, [x17]`
    ///
    /// FLS §7.2: Static items. FLS §4.2: f32 type.
    /// Like `LoadStaticF64` but loads into the 32-bit float register `s{dst}`.
    ///
    /// Cache-line note: three 4-byte instructions (12 bytes). The data value is
    /// 4 bytes in `.data` — half the footprint of an f64 static.
    LoadStaticF32 {
        /// Destination float register (s0–s15).
        dst: u8,
        /// The assembly label of the static (same as the Rust static name).
        name: String,
    },

    /// Load the address of a named function into a register.
    ///
    /// `LoadFnAddr { dst, name }` emits:
    ///   `adrp x{dst}, {name}`
    ///   `add  x{dst}, x{dst}, :lo12:{name}`
    ///
    /// FLS §4.9: Function pointer types. A function's address is loaded using
    /// PC-relative addressing. Unlike `bl {name}` (which transfers control),
    /// this instruction materializes the address as a data value that can be
    /// stored, passed as an argument, and later called via `blr`.
    ///
    /// ARM64 addressing: ADRP loads the page-aligned base of the function into
    /// the register; ADD applies the :lo12: offset to form the full address.
    ///
    /// Cache-line note: two 4-byte instructions (8 bytes) to load a function
    /// address — half the cost of a static load (which also dereferences the
    /// pointer).
    LoadFnAddr {
        /// Destination register for the function address.
        dst: u8,
        /// The assembly label of the function (its mangled name).
        name: String,
    },

    /// Call through a function pointer stored in a stack slot.
    ///
    /// `CallIndirect { dst, ptr_slot, args }` emits:
    ///   `mov x{i}, x{args[i]}` (for each arg not already in the right register)
    ///   `ldr x9, [sp, #{ptr_slot*8}]`  // load fn ptr into scratch register
    ///   `blr x9`                        // indirect call
    ///   `mov x{dst}, x0`               // capture return value
    ///
    /// FLS §4.9: Function pointer types. Calling through a function pointer
    /// uses the ARM64 `blr` (branch with link to register) instruction.
    ///
    /// ARM64 ABI: same register convention as direct calls — args in x0–x7,
    /// return value in x0. The link register (x30) is set by `blr`, so
    /// the caller must save x30 if it also makes direct calls.
    ///
    /// Cache-line note: N arg moves + ldr + blr + 1 result move = N+3
    /// instructions. The extra `ldr` (vs direct call) is the pointer
    /// indirection cost of using `fn(T) -> U` rather than a named function.
    CallIndirect {
        /// Destination virtual register for the return value.
        dst: u8,
        /// Stack slot holding the function pointer address.
        ptr_slot: u8,
        /// Argument virtual registers, in left-to-right parameter order.
        args: Vec<u8>,
    },

    /// Call a method through a vtable (dynamic dispatch). FLS §4.13.
    ///
    /// `CallVtable { dst, data_slot, vtable_slot, method_idx }` emits:
    ///   `ldr x9, [sp, #{vtable_slot*8}]`    // load vtable pointer
    ///   `ldr x10, [x9, #{method_idx*8}]`    // load method fn-ptr from vtable
    ///   `ldr x0, [sp, #{data_slot*8}]`      // load data pointer → shim arg
    ///   `blr x10`                             // dispatch through vtable
    ///   `mov x{dst}, x0`                     // capture return value
    ///
    /// The callee is a vtable shim that receives x0 = data pointer, expands
    /// the struct fields into registers, and tail-calls the concrete method.
    ///
    /// ARM64 scratch registers x9/x10 are caller-saved and not argument
    /// registers, so loading the vtable and method pointers there does not
    /// disturb the data pointer in x0.
    ///
    /// FLS §4.13: "Each method in the trait definition has a pointer in the
    /// vtable." Indexing: method_idx × 8 bytes gives the byte offset.
    ///
    /// Cache-line note: 4 fixed instructions (ldr, ldr, ldr, blr) + 1 result
    /// move = 5 instructions (20 bytes). Fits in one 64-byte cache line
    /// alongside 11 other instructions.
    CallVtable {
        /// Destination virtual register for the scalar return value.
        dst: u8,
        /// Stack slot holding the data pointer (first half of fat pointer).
        data_slot: u8,
        /// Stack slot holding the vtable pointer (second half of fat pointer).
        vtable_slot: u8,
        /// Index of the method within the vtable (0 = first method).
        method_idx: usize,
    },

    /// Call a function that returns a `&dyn Trait` fat pointer. FLS §4.13.
    ///
    /// `CallRetFatPtr { name, args, dst_data_slot }` emits:
    ///   `mov x{i}, x{args[i]}` (for each arg not already in the right register)
    ///   `bl {name}`
    ///   `str x0, [sp, #{dst_data_slot*8}]`      // store returned data ptr
    ///   `str x1, [sp, #{(dst_data_slot+1)*8}]`  // store returned vtable ptr
    ///
    /// After the call, x0 = data pointer, x1 = vtable pointer (the fat pointer
    /// components). Both are stored to consecutive stack slots at `dst_data_slot`
    /// and `dst_data_slot+1` so the caller can use them as a local `&dyn Trait`.
    ///
    /// FLS §4.13: `&dyn Trait` as a function return type requires returning
    /// (data_ptr, vtable_ptr) across the function boundary.
    /// FLS §4.13 AMBIGUOUS: The spec does not define the fat pointer return ABI.
    /// Galvanic uses (x0=data_ptr, x1=vtable_ptr) matching the parameter ABI.
    ///
    /// Cache-line note: N arg moves + bl + 2 stores = N+3 instructions.
    /// For a one-fat-pointer-arg call: 2 moves + bl + 2 stores = 5 instructions
    /// (20 bytes), fitting in one 64-byte cache line alongside other code.
    CallRetFatPtr {
        /// Name of the function to call.
        name: String,
        /// Integer argument registers (may include paired fat-ptr regs).
        args: Vec<u8>,
        /// Stack slot for the returned data pointer; vtable at dst_data_slot+1.
        dst_data_slot: u8,
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

    /// Return struct fields AND a scalar value from a `&mut self` method.
    ///
    /// Callee convention for `&mut self` methods with a scalar return type:
    ///   x0..x{N-1} — modified field values (write-back for caller)
    ///   x{N}       — the scalar return value
    ///
    /// Emits:
    ///   `ldr x{i}, [sp, #{(base_slot+i)*8}]`  for i in 0..n_fields  // field write-backs
    ///   `mov x{n_fields}, x{val_reg}`                                 // return value (if val_reg != n_fields)
    ///   standard epilogue + `ret`
    ///
    /// FLS §10.1: `&mut self` methods may return any type. Galvanic uses a
    /// register-packing convention: fields in x0..x{N-1}, scalar in x{N}.
    ///
    /// FLS §10.1 AMBIGUOUS: The spec does not define the calling convention for
    /// `&mut self` methods with non-unit return types. This convention is an
    /// extension of the existing `RetFields` convention.
    ///
    /// Cache-line note: N+1 loads = (N+1) × 4-byte instructions before epilogue.
    RetFieldsAndValue {
        /// Stack slot of the first self field (always 0 for methods).
        base_slot: u8,
        /// Number of struct fields.
        n_fields: u8,
        /// Register holding the scalar return value.
        val_reg: u8,
    },

    /// Call a `&mut self` method that returns a scalar value, write back fields,
    /// and capture the return value.
    ///
    /// Like `CallMut`, but after writing back x0..x{N-1} to the caller's struct
    /// slots, also reads x{N} (the callee's scalar return value) into `dst`.
    ///
    /// Emits:
    ///   `mov x{i}, x{args[i]}`  for each arg          // move args into registers
    ///   `bl {name}`                                     // call the method
    ///   `str x{i}, [sp, #{(write_back_slot+i)*8}]`  for i in 0..n_fields  // write back
    ///   `mov x{dst}, x{n_fields}`                      // capture return value
    ///
    /// FLS §10.1: Write-back convention — callee returns modified fields in
    /// x0..x{N-1}, caller stores them back. Scalar return value is in x{N}.
    ///
    /// Cache-line note: N write-back stores + 1 capture move = (N+1) × 4-byte
    /// instructions after the `bl`. Same cache footprint as `CallMut` + 1.
    CallMutReturn {
        /// Name of the `&mut self` method to call (mangled).
        name: String,
        /// Argument registers: struct fields first (x0..x{N-1}), then explicit args.
        args: Vec<u8>,
        /// Base stack slot of the receiver struct in the caller's frame.
        write_back_slot: u8,
        /// Number of struct fields to write back.
        n_fields: u8,
        /// Destination register for the scalar return value (from x{N}).
        dst: u8,
    },

    /// Compute the address of a stack slot: `dst = sp + slot * 8`.
    ///
    /// `AddrOf { dst, slot }` → `add x{dst}, sp, #{slot * 8}` on ARM64.
    ///
    /// FLS §6.5.1: Borrow expressions `&place` produce a reference (pointer)
    /// to the place's memory location. For a local variable at stack slot `s`,
    /// this is `sp + s * 8`.
    ///
    /// ARM64: `add xD, sp, #imm` computes `sp + imm` and stores the result
    /// in `xD`. The immediate is `slot * 8` (byte offset of the slot).
    ///
    /// Cache-line note: one 4-byte instruction. The resulting pointer value
    /// occupies one 8-byte register slot — identical footprint to any i32 value.
    AddrOf {
        /// Destination register — receives the address.
        dst: u8,
        /// Stack slot index; byte offset = slot * 8.
        slot: u8,
    },

    /// Compute the address of an array element: `dst = &arr[index]`.
    ///
    /// `AddrOfIndexed { dst, base_slot, index_reg, scratch }` emits:
    ///   `add x{scratch}, sp, #{base_slot*8}`                   // base address of arr[0]
    ///   `add x{dst}, x{scratch}, x{index_reg}, lsl #3`         // address of arr[index]
    ///
    /// FLS §6.5.1: Borrow expressions. FLS §6.9: Indexing expressions.
    /// FLS §4.9: `&mut [T; N]` borrows produce a mutable reference to the element.
    /// FLS §6.15.1: Used in `for x in &mut arr` to bind each element address.
    ///
    /// ARM64: Two-instruction sequence — `add` for base then `add` with `lsl #3`
    /// scales the index by 8 (bytes per slot) to reach `arr[index]`.
    ///
    /// Cache-line note: two 4-byte instructions (8 bytes). The result pointer
    /// occupies one 8-byte register slot — identical footprint to any i32 value.
    AddrOfIndexed {
        /// Destination register — receives the address of `arr[index]`.
        dst: u8,
        /// Stack slot index of the first array element (arr[0]).
        base_slot: u8,
        /// Register holding the runtime array index (0-based).
        index_reg: u8,
        /// Scratch register for base address computation (must not alias dst or index_reg).
        scratch: u8,
    },

    /// Load through a pointer: `dst = *src` (load 8 bytes at address in src).
    ///
    /// `LoadPtr { dst, src }` → `ldr x{dst}, [x{src}]` on ARM64.
    ///
    /// FLS §6.5.2: Dereference expressions `*expr` load the value at the
    /// memory address held in `expr`. The address is in a register; the
    /// load uses register-indirect addressing.
    ///
    /// ARM64: `ldr xD, [xS]` loads 8 bytes from the address in `xS`.
    ///
    /// Cache-line note: one 4-byte instruction. The load targets the same
    /// cache line as the referent value on the stack (8-byte aligned).
    LoadPtr {
        /// Destination register — receives the loaded value.
        dst: u8,
        /// Source register — holds the pointer (memory address).
        src: u8,
    },

    /// Store through a pointer: `*addr = src` (store 8 bytes at address in addr).
    ///
    /// `StorePtr { src, addr }` → `str x{src}, [x{addr}]` on ARM64.
    ///
    /// FLS §6.5.10: Assignment expression `*place = value` where the place is
    /// a dereferenced reference. The address is held in register `addr`; the
    /// value to store is in register `src`.
    ///
    /// ARM64: `str xS, [xA]` stores 8 bytes from `xS` to the address in `xA`.
    ///
    /// Cache-line note: one 4-byte instruction. The store targets the same
    /// cache line as the referent value (8-byte aligned). Symmetric with
    /// `LoadPtr` — same instruction count, same cache footprint.
    StorePtr {
        /// Source register — holds the value to store.
        src: u8,
        /// Address register — holds the pointer (memory address to write to).
        addr: u8,
    },

    /// Load a 64-bit float constant from the per-function constant pool.
    ///
    /// `LoadF64Const { dst, idx }` emits:
    ///   `adrp x17, {fn}__fc{idx}`
    ///   `add  x17, x17, :lo12:{fn}__fc{idx}`
    ///   `ldr  d{dst}, [x17]`
    ///
    /// `x17` (ARM64 `ip1`) is used as a scratch register for the address
    /// computation. `ip1` is caller-saved and reserved for intra-procedure
    /// call scratch use; using it avoids consuming a general-purpose virtual
    /// register for the address operand.
    ///
    /// The constant label `{fn}__fc{idx}` is emitted in `.rodata` by codegen.
    ///
    /// FLS §2.4.4.2: Float literal expressions. The float value is materialised
    /// at runtime via a load from read-only data, not a constant fold.
    /// FLS §6.1.2:37–45: Even a float literal emits runtime instructions.
    ///
    /// Cache-line note: three 4-byte instructions (ADRP + ADD + LDR) = 12 bytes.
    /// The constant itself is one 8-byte `.quad` in `.rodata`.
    LoadF64Const {
        /// Destination float register (ARM64 `d{dst}`).
        dst: u8,
        /// Index into the function's `float_consts` pool.
        idx: u32,
    },

    /// Store a float register to an 8-byte stack slot.
    ///
    /// `StoreF64 { src, slot }` → `str d{src}, [sp, #{slot * 8}]` on ARM64.
    ///
    /// FLS §8.1: Float let bindings require stack storage like integer bindings.
    /// The slot occupies 8 bytes — same layout as integer slots.
    ///
    /// Cache-line note: ARM64 `str` (float) is 4 bytes, same as integer `str`.
    StoreF64 {
        /// Source float register holding the value to store.
        src: u8,
        /// Destination stack slot index (byte offset = slot * 8).
        slot: u8,
    },

    /// Load an 8-byte stack slot into a float register.
    ///
    /// `LoadF64Slot { dst, slot }` → `ldr d{dst}, [sp, #{slot * 8}]` on ARM64.
    ///
    /// FLS §8.1: Reading a float local variable accesses its stack slot.
    ///
    /// Cache-line note: ARM64 `ldr` (float) is 4 bytes, same as integer `ldr`.
    LoadF64Slot {
        /// Destination float register.
        dst: u8,
        /// Source stack slot index (byte offset = slot * 8).
        slot: u8,
    },

    /// Convert a 64-bit float register to a signed 32-bit integer, truncating
    /// toward zero.
    ///
    /// `F64ToI32 { dst, src }` → `fcvtzs w{dst}, d{src}` on ARM64.
    ///
    /// FLS §6.5.9: Numeric cast `f64 as i32`. Conversion truncates toward zero
    /// (C-style truncation, same as IEEE 754 `trunc`). Out-of-range values
    /// saturate to `i32::MIN` or `i32::MAX` on ARM64 hardware.
    ///
    /// FLS §6.5.9 AMBIGUOUS: The spec requires "panics if the value is not
    /// representable" in debug mode. Galvanic does not emit a range check at
    /// this milestone — saturation behaviour differs from the spec's panic
    /// requirement.
    ///
    /// Cache-line note: one 4-byte instruction (FCVTZS).
    F64ToI32 {
        /// Destination integer register (ARM64 `w{dst}` / `x{dst}`).
        dst: u8,
        /// Source float register (ARM64 `d{src}`).
        src: u8,
    },

    /// Floating-point binary arithmetic on `f64` values.
    ///
    /// `F64BinOp { op, dst, lhs, rhs }` emits one of:
    ///   - `fadd  d{dst}, d{lhs}, d{rhs}` — addition
    ///   - `fsub  d{dst}, d{lhs}, d{rhs}` — subtraction
    ///   - `fmul  d{dst}, d{lhs}, d{rhs}` — multiplication
    ///   - `fdiv  d{dst}, d{lhs}, d{rhs}` — division
    ///
    /// FLS §6.5.5: Arithmetic expressions on floating-point types. The `+`, `-`,
    /// `*`, `/` operators on `f64` operands produce `f64` results, following
    /// IEEE 754 double-precision semantics.
    ///
    /// FLS §6.5.5 AMBIGUOUS: The spec references IEEE 754 semantics but does not
    /// specify the rounding mode. ARM64 hardware uses round-to-nearest-even by
    /// default, matching the Rust reference behaviour.
    ///
    /// Cache-line note: one 4-byte instruction per operation.
    F64BinOp {
        /// The arithmetic operation.
        op: F64BinOp,
        /// Destination float register (ARM64 `d{dst}`).
        dst: u8,
        /// Left operand float register (ARM64 `d{lhs}`).
        lhs: u8,
        /// Right operand float register (ARM64 `d{rhs}`).
        rhs: u8,
    },

    // ── f32 instructions (FLS §2.4.4.2, §4.2, §6.5.5) ───────────────────────

    /// Load a 32-bit float constant from `.rodata` into an `s`-register.
    ///
    /// `LoadF32Const { dst, idx }` emits:
    ///   adrp  x17, {fn_name}__f32c{idx}
    ///   add   x17, x17, :lo12:{fn_name}__f32c{idx}
    ///   ldr   s{dst}, [x17]
    ///
    /// FLS §2.4.4.2: Float literal with `_f32` suffix. Stored as 4-byte
    /// IEEE 754 bit pattern in `.rodata` (`.word` directive, `.align 2`).
    ///
    /// Cache-line note: same 3-instruction address materialisation as
    /// `LoadF64Const`; the constant is 4 bytes vs 8 bytes for f64.
    LoadF32Const {
        /// Destination single-precision float register (ARM64 `s{dst}`).
        dst: u8,
        /// Index into `IrFn::float32_consts`.
        idx: u32,
    },

    /// Store a single-precision float register to a stack slot.
    ///
    /// `StoreF32 { src, slot }` → `str s{src}, [sp, #{slot * 8}]`
    ///
    /// FLS §8.1: `let x: f32 = …` stores the value to the stack.
    /// The slot is 8 bytes wide; only the lower 4 bytes are written.
    ///
    /// Cache-line note: one 4-byte instruction.
    StoreF32 {
        /// Source single-precision float register (ARM64 `s{src}`).
        src: u8,
        /// Stack slot index (byte offset = slot × 8).
        slot: u8,
    },

    /// Load a single-precision float register from a stack slot.
    ///
    /// `LoadF32Slot { dst, slot }` → `ldr s{dst}, [sp, #{slot * 8}]`
    ///
    /// FLS §8.1: Reading a local variable bound as `f32`.
    ///
    /// Cache-line note: one 4-byte instruction.
    LoadF32Slot {
        /// Destination single-precision float register (ARM64 `s{dst}`).
        dst: u8,
        /// Stack slot index (byte offset = slot × 8).
        slot: u8,
    },

    /// Convert a single-precision float register to a signed 32-bit integer.
    ///
    /// `F32ToI32 { dst, src }` → `fcvtzs w{dst}, s{src}`
    ///
    /// FLS §6.5.9: `f32 as i32` truncates toward zero.
    ///
    /// FLS §6.5.9 AMBIGUOUS: ARM64 FCVTZS saturates out-of-range values;
    /// Rust requires wrapping (release) or panic (debug). Same limitation
    /// as `F64ToI32`.
    ///
    /// Cache-line note: one 4-byte instruction.
    F32ToI32 {
        /// Destination integer register (ARM64 `w{dst}` / `x{dst}`).
        dst: u8,
        /// Source single-precision float register (ARM64 `s{src}`).
        src: u8,
    },

    /// Convert a signed 32-bit integer register to a 64-bit float register.
    ///
    /// `I32ToF64 { dst, src }` → `scvtf d{dst}, w{src}` on ARM64.
    ///
    /// FLS §6.5.9: Numeric cast `i32 as f64`. Converts a signed integer to
    /// IEEE 754 double-precision float. All `i32` values are exactly
    /// representable as `f64` (which has 52-bit mantissa).
    ///
    /// Cache-line note: one 4-byte instruction (SCVTF).
    I32ToF64 {
        /// Destination float register (ARM64 `d{dst}`).
        dst: u8,
        /// Source integer register (ARM64 `w{src}`).
        src: u8,
    },

    /// Convert a signed 32-bit integer register to a 32-bit float register.
    ///
    /// `I32ToF32 { dst, src }` → `scvtf s{dst}, w{src}` on ARM64.
    ///
    /// FLS §6.5.9: Numeric cast `i32 as f32`. Converts a signed integer to
    /// IEEE 754 single-precision float. Values that cannot be exactly
    /// represented are rounded to nearest-even.
    ///
    /// Cache-line note: one 4-byte instruction (SCVTF).
    I32ToF32 {
        /// Destination single-precision float register (ARM64 `s{dst}`).
        dst: u8,
        /// Source integer register (ARM64 `w{src}`).
        src: u8,
    },

    /// Convert a 32-bit float register to a 64-bit float register.
    ///
    /// `F32ToF64 { dst, src }` → `fcvt d{dst}, s{src}` on ARM64.
    ///
    /// FLS §6.5.9: Numeric cast `f32 as f64`. Converts a single-precision
    /// float to double-precision. The conversion is exact for finite values
    /// (every f32 value is representable as f64).
    ///
    /// Cache-line note: one 4-byte instruction (FCVT).
    F32ToF64 {
        /// Destination double-precision float register (ARM64 `d{dst}`).
        dst: u8,
        /// Source single-precision float register (ARM64 `s{src}`).
        src: u8,
    },

    /// Convert a 64-bit float register to a 32-bit float register.
    ///
    /// `F64ToF32 { dst, src }` → `fcvt s{dst}, d{src}` on ARM64.
    ///
    /// FLS §6.5.9: Numeric cast `f64 as f32`. Converts a double-precision
    /// float to single-precision. Values that cannot be exactly represented
    /// are rounded to nearest-even (IEEE 754 default rounding mode).
    ///
    /// Cache-line note: one 4-byte instruction (FCVT).
    F64ToF32 {
        /// Destination single-precision float register (ARM64 `s{dst}`).
        dst: u8,
        /// Source double-precision float register (ARM64 `d{src}`).
        src: u8,
    },

    /// Floating-point binary arithmetic on `f32` values.
    ///
    /// `F32BinOp { op, dst, lhs, rhs }` emits one of:
    ///   - `fadd  s{dst}, s{lhs}, s{rhs}` — addition
    ///   - `fsub  s{dst}, s{lhs}, s{rhs}` — subtraction
    ///   - `fmul  s{dst}, s{lhs}, s{rhs}` — multiplication
    ///   - `fdiv  s{dst}, s{lhs}, s{rhs}` — division
    ///
    /// FLS §6.5.5: `+`, `-`, `*`, `/` on `f32` operands; IEEE 754
    /// single-precision semantics.
    ///
    /// Cache-line note: one 4-byte instruction per f32 binary operation.
    F32BinOp {
        /// The arithmetic operation.
        op: F32BinOp,
        /// Destination single-precision float register (ARM64 `s{dst}`).
        dst: u8,
        /// Left operand (ARM64 `s{lhs}`).
        lhs: u8,
        /// Right operand (ARM64 `s{rhs}`).
        rhs: u8,
    },

    /// IEEE 754 double-precision comparison, result in an integer register.
    ///
    /// `FCmpF64 { op, dst, lhs, rhs }` emits:
    ///   1. `fcmp  d{lhs}, d{rhs}` — sets floating-point condition flags
    ///   2. `cset  x{dst}, <cond>` — materialises 1 or 0 in `x{dst}`
    ///
    /// FLS §6.5.3: Comparison operator expressions. For `f64` operands, the
    /// operators `<`, `<=`, `>`, `>=`, `==`, `!=` produce a `bool` result.
    ///
    /// FLS §6.5.3 AMBIGUOUS: The spec does not specify NaN comparison behaviour.
    /// ARM64 FCMP with NaN input sets all flags such that `lt`, `le`, `gt`,
    /// `ge` all produce 0, and `eq` produces 0, `ne` produces 1.
    ///
    /// Cache-line note: two 4-byte instructions (fcmp + cset = 8 bytes).
    FCmpF64 {
        /// The comparison operator.
        op: FCmpOp,
        /// Destination integer register (ARM64 `x{dst}`).
        dst: u8,
        /// Left float operand (ARM64 `d{lhs}`).
        lhs: u8,
        /// Right float operand (ARM64 `d{rhs}`).
        rhs: u8,
    },

    /// IEEE 754 single-precision comparison, result in an integer register.
    ///
    /// `FCmpF32 { op, dst, lhs, rhs }` emits:
    ///   1. `fcmp  s{lhs}, s{rhs}` — sets floating-point condition flags
    ///   2. `cset  x{dst}, <cond>` — materialises 1 or 0 in `x{dst}`
    ///
    /// FLS §6.5.3: Same as `FCmpF64` but for `f32` operands.
    ///
    /// Cache-line note: two 4-byte instructions (fcmp + cset = 8 bytes).
    FCmpF32 {
        /// The comparison operator.
        op: FCmpOp,
        /// Destination integer register (ARM64 `x{dst}`).
        dst: u8,
        /// Left float operand (ARM64 `s{lhs}`).
        lhs: u8,
        /// Right float operand (ARM64 `s{rhs}`).
        rhs: u8,
    },
}

/// Comparison operator for floating-point comparison instructions.
///
/// FLS §6.5.3: Comparison operators on floating-point types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FCmpOp {
    /// `<` — ordered less than.
    Lt,
    /// `<=` — ordered less than or equal.
    Le,
    /// `>` — ordered greater than.
    Gt,
    /// `>=` — ordered greater than or equal.
    Ge,
    /// `==` — ordered equal.
    Eq,
    /// `!=` — ordered not equal.
    Ne,
}

/// Arithmetic operator for `f64` binary expressions.
///
/// FLS §6.5.5: The arithmetic operators on floating-point types.
/// Bitwise operators, remainder, and shifts are not defined for `f64` (FLS §6.5.6,
/// §6.5.7, §6.5.8 only cover integer types).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum F64BinOp {
    /// `fadd` — IEEE 754 double-precision addition.
    Add,
    /// `fsub` — IEEE 754 double-precision subtraction.
    Sub,
    /// `fmul` — IEEE 754 double-precision multiplication.
    Mul,
    /// `fdiv` — IEEE 754 double-precision division.
    Div,
}

/// Arithmetic operator for `f32` binary expressions.
///
/// FLS §6.5.5: The arithmetic operators on floating-point types.
/// Bitwise operators, remainder, and shifts are not defined for `f32`
/// (FLS §6.5.6, §6.5.7, §6.5.8 only cover integer types).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum F32BinOp {
    /// `fadd` — IEEE 754 single-precision addition.
    Add,
    /// `fsub` — IEEE 754 single-precision subtraction.
    Sub,
    /// `fmul` — IEEE 754 single-precision multiplication.
    Mul,
    /// `fdiv` — IEEE 754 single-precision division.
    Div,
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
    /// Larger values use `MOVZ`+`MOVK` sequences (8 bytes, 2 instruction slots).
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

    /// A value held in float register N (ARM64 `d{N}`, 64-bit / f64).
    ///
    /// FLS §4.2: The `f64` type is a 64-bit IEEE 754 floating-point number.
    /// ARM64 uses a separate 128-bit SIMD/FP register bank (`v0`–`v31`); the
    /// 64-bit alias is `d{N}`. Float and integer register banks are independent:
    /// `x0` and `d0` are physically separate.
    ///
    /// Cache-line note: `u8` occupies 1 byte — same size as `Reg(u8)`.
    FReg(u8),

    /// A value held in single-precision float register N (ARM64 `s{N}`, 32-bit / f32).
    ///
    /// FLS §4.2: The `f32` type is a 32-bit IEEE 754 floating-point number.
    /// ARM64: `s{N}` is the 32-bit view of SIMD/FP register `v{N}`.
    ///
    /// Kept separate from `FReg` (f64) so lowering and codegen can choose the
    /// correct register width (`s` vs `d`) without inspecting instruction context.
    ///
    /// Cache-line note: `u8` occupies 1 byte — same size as `FReg(u8)`.
    F32Reg(u8),
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

    /// Unsigned integer type — covers `u16`, `u32`, `u64`, and `usize`.
    ///
    /// FLS §4.1: Unsigned integer types. All map to a 64-bit ARM64 register
    /// (AArch64 is 64-bit; narrower types use the low bits). Key difference
    /// from `IrTy::I32`: division uses `udiv` (unsigned) and right shift uses
    /// `lsr` (logical, zero-extending) rather than `asr` (arithmetic,
    /// sign-extending).
    ///
    /// Cache-line note: same register width as `IrTy::I32`.
    U32,

    /// The 8-bit unsigned integer type `u8`. FLS §4.1.
    ///
    /// Arithmetic uses the same instructions as `IrTy::U32` (unsigned division,
    /// logical shift). The key difference: at function return and on explicit
    /// `return` expressions, a `TruncU8` instruction masks the result to the
    /// low 8 bits (`and w{r}, w{r}, #255`), implementing the FLS §6.23
    /// wrapping semantics for u8.
    ///
    /// FLS §4.1: "The unsigned integer types ... have the same operations as
    /// the signed integer types."
    /// FLS §6.23: At runtime without overflow checks, integer overflow wraps
    /// in two's complement. For u8, the modulus is 256.
    ///
    /// Cache-line note: same register width as `IrTy::U32`; one extra
    /// `and` instruction at return boundaries.
    U8,

    /// The 8-bit signed integer type `i8`. FLS §4.1. Milestone 177.
    ///
    /// Arithmetic uses the same instructions as `IrTy::I32` (signed division,
    /// arithmetic shift right). The key difference: at function return and on
    /// explicit `return` expressions, a `SextI8` instruction sign-extends the
    /// result from 8 bits (`sxtb w{r}, w{r}`), implementing the FLS §6.23
    /// wrapping semantics for i8.
    ///
    /// FLS §4.1: "The signed integer types have a range of [-2^(N-1), 2^(N-1)-1]."
    /// FLS §6.23: At runtime without overflow checks, integer arithmetic wraps
    /// in two's complement. For i8, the range is -128..=127.
    ///
    /// Cache-line note: same register width as `IrTy::I32`; one extra
    /// `sxtb` instruction at return boundaries.
    I8,

    /// The 16-bit unsigned integer type `u16`. FLS §4.1. Milestone 181.
    ///
    /// Arithmetic uses the same instructions as `IrTy::U32` (unsigned division,
    /// logical shift right). The key difference: at function return and on
    /// explicit `return` expressions, a `TruncU16` instruction masks the result
    /// to the low 16 bits (`and w{r}, w{r}, #65535`), implementing the FLS §6.23
    /// wrapping semantics for u16.
    ///
    /// FLS §4.1: The unsigned integer types have a range of [0, 2^N-1]. For u16: 0..=65535.
    /// FLS §6.23: At runtime without overflow checks, integer arithmetic wraps
    /// in two's complement. For u16, the modulus is 65536.
    ///
    /// Cache-line note: same register width as `IrTy::U32`; one extra
    /// `and` instruction at return boundaries.
    U16,

    /// The 16-bit signed integer type `i16`. FLS §4.1. Milestone 181.
    ///
    /// Arithmetic uses the same instructions as `IrTy::I32` (signed division,
    /// arithmetic shift right). The key difference: at function return and on
    /// explicit `return` expressions, a `SextI16` instruction sign-extends the
    /// result from 16 bits (`sxth x{r}, w{r}`), implementing the FLS §6.23
    /// wrapping semantics for i16.
    ///
    /// FLS §4.1: The signed integer types have a range of [-2^(N-1), 2^(N-1)-1]. For i16: -32768..=32767.
    /// FLS §6.23: At runtime without overflow checks, integer arithmetic wraps
    /// in two's complement. For i16, the range is -32768..=32767.
    ///
    /// Cache-line note: same register width as `IrTy::I32`; one extra
    /// `sxth` instruction at return boundaries.
    I16,

    /// A function pointer type `fn(T1, ...) -> R`. FLS §4.9.
    ///
    /// Function pointers are 64-bit addresses — one ARM64 register, identical
    /// in layout to any scalar value. They are passed as a single integer
    /// argument register and stored in a single stack slot.
    ///
    /// Cache-line note: same register/slot footprint as `IrTy::I32`.
    FnPtr,

    /// The 64-bit floating-point type `f64`. FLS §4.2.
    ///
    /// Values live in ARM64 float registers `d{N}`. Stack slots are 8 bytes —
    /// the same size as integer slots — so mixed integer/float functions share
    /// the same slot-allocation logic. Float and integer registers are distinct
    /// physical banks on ARM64, so no moves between them occur at this level;
    /// explicit `F64ToI32` / `I32ToF64` conversions are required (FLS §6.5.9).
    ///
    /// Cache-line note: same 8-byte slot footprint as `IrTy::I32`.
    F64,

    /// The 32-bit floating-point type `f32`. FLS §4.2.
    ///
    /// Values live in ARM64 single-precision float registers `s{N}`. Stack
    /// slots are 8 bytes (same as all other types); only the lower 4 bytes
    /// are used. Explicit `F32ToI32` conversion required for integer contexts.
    ///
    /// Cache-line note: same 8-byte slot footprint as `IrTy::F64`; slightly
    /// smaller constant pool entries (`.word` vs `.quad`).
    F32,
}
