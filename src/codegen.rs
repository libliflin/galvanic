//! ARM64 assembly text emission for galvanic.
//!
//! Takes an IR `Module` and writes GNU assembler (GAS) syntax suitable for
//! `aarch64-linux-gnu-as` (or the native `as` on an ARM64 host).
//!
//! # Target
//!
//! - Architecture: AArch64 (ARM64)
//! - OS ABI: Linux ELF
//! - Entry point: `_start` (bare; no libc startup)
//! - System call convention: syscall number in `x8`, args in `x0`–`x5`
//!
//! # FLS traceability
//!
//! - FLS §9: Functions — each `IrFn` emits a labeled function body.
//! - FLS §6.19: Return expressions — `Instr::Ret` emits `mov x0, #n; ret`.
//! - FLS §18.1: Program entry point — `_start` calls `main` and exits.
//!
//! # Cache-line note
//!
//! ARM64 instructions are 4 bytes each; 16 instructions fill one 64-byte
//! cache line. The `main` function for milestone 1 is exactly 2 instructions
//! (8 bytes), and `_start` is 3 instructions (12 bytes). Both fit entirely
//! within a single cache line. No explicit `.align` directives are needed
//! at this scale, but the rationale is documented here for future cycles.

use std::fmt::Write as FmtWrite;

use crate::ir::{IrBinOp, Instr, IrValue, Module};

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors that can occur during code generation.
#[derive(Debug)]
pub enum CodegenError {
    /// A language feature is not yet supported by the code generator.
    Unsupported(String),
    /// An internal string-formatting error (should not occur in practice).
    Fmt(std::fmt::Error),
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::Unsupported(msg) => write!(f, "codegen: not yet supported: {msg}"),
            CodegenError::Fmt(e) => write!(f, "codegen: format error: {e}"),
        }
    }
}

impl From<std::fmt::Error> for CodegenError {
    fn from(e: std::fmt::Error) -> Self {
        CodegenError::Fmt(e)
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Emit a module as ARM64 assembly text.
///
/// Returns a `String` containing valid GAS syntax. The caller is responsible
/// for writing it to a `.s` file and invoking the assembler.
///
/// The emitted file defines a `_start` symbol that calls `main` and then
/// invokes the Linux `sys_exit` syscall with `main`'s return value.
///
/// FLS §18.1: `main` is the program entry point.
pub fn emit_asm(module: &Module) -> Result<String, CodegenError> {
    let has_main = module.fns.iter().any(|f| f.name == "main");
    if !has_main {
        return Err(CodegenError::Unsupported("no `main` function in module".into()));
    }

    let mut out = String::new();

    writeln!(out, "    .text")?;

    for func in &module.fns {
        writeln!(out)?;
        emit_fn(&mut out, func)?;
    }

    // Emit the bare _start entry point.
    writeln!(out)?;
    emit_start(&mut out)?;

    // Emit the .data section for static items.
    //
    // FLS §7.2: Static items reside in the data section.
    // FLS §7.2:15: All references to a static refer to the same memory address.
    //
    // Cache-line note: each static occupies 8 bytes (.quad). Eight statics
    // fill one 64-byte data cache line. We align each static to 8 bytes
    // (.align 3) to prevent two statics from sharing a single alignment unit
    // and to match the 64-bit LDR requirement on ARM64.
    //
    // FLS §7.2 AMBIGUOUS: The spec does not mandate a specific data section
    // alignment. Galvanic uses .align 3 (8-byte alignment) as the minimum
    // for correct 64-bit LDR addressing.
    if !module.statics.is_empty() {
        writeln!(out)?;
        writeln!(out, "    .data")?;
        for s in &module.statics {
            writeln!(out, "    .align 3")?;
            writeln!(out, "    .global {}", s.name)?;
            writeln!(out, "{}:", s.name)?;
            writeln!(out, "    .quad {}", s.value)?;
        }
    }

    Ok(out)
}

// ── Function emission ─────────────────────────────────────────────────────────

/// Compute the ARM64 stack frame size for a given number of 8-byte slots.
///
/// ARM64 ABI requires the stack pointer to be 16-byte aligned at all times.
/// We round up the raw byte count to the next multiple of 16.
///
/// Cache-line note: each 8-byte slot occupies one half of a 128-bit
/// (16-byte) alignment unit; two slots fill one aligned unit perfectly.
fn frame_size(stack_slots: u8) -> u32 {
    if stack_slots == 0 {
        return 0;
    }
    let raw = stack_slots as u32 * 8;
    // Round up to 16-byte alignment.
    (raw + 15) & !15
}

/// Emit one function.
///
/// FLS §9: Functions. Each function is a labeled sequence of instructions
/// ending with a `ret` (via `emit_instr`).
///
/// Stack layout (low address to high address, from the top of the frame):
///   [optional lr save slot]   — 16 bytes, pre-indexed push; only if saves_lr
///   [local variable slots]    — stack_slots * 8 bytes, rounded to 16; only if > 0
///
/// On entry to the function, sp points at the caller's frame boundary.
/// The prologue saves lr first (if needed), then allocates locals.
/// The epilogue restores locals first, then restores lr, then `ret`.
///
/// Cache-line note: `sub sp, sp, #N` is one 4-byte instruction — the frame
/// setup occupies one slot in the first cache line of the function body.
/// The lr save/restore pair (`str`/`ldr`) each adds one 4-byte instruction.
fn emit_fn(out: &mut String, func: &crate::ir::IrFn) -> Result<(), CodegenError> {
    writeln!(out, "    // fn {} — FLS §9", func.name)?;
    writeln!(out, "    .global {}", func.name)?;
    writeln!(out, "{}:", func.name)?;

    // FLS §6.12.1: Non-leaf functions must save the link register (x30)
    // before any `bl` instruction overwrites it. ARM64 pre-indexed store:
    //   `str x30, [sp, #-16]!` → sp -= 16 first, then store x30 at [sp].
    // This keeps sp 16-byte aligned (ARM64 ABI requirement).
    //
    // Cache-line note: the lr save is one 4-byte instruction; paired with
    // the matching `ldr` restore in the epilogue, both are in the first
    // and last cache line of the function respectively.
    if func.saves_lr {
        writeln!(
            out,
            "    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)"
        )?;
    }

    let fsize = frame_size(func.stack_slots);

    if fsize > 0 {
        // FLS §8.1: allocate stack space for local variables.
        // ARM64: SP must remain 16-byte aligned (ABI requirement).
        writeln!(
            out,
            "    sub     sp, sp, #{fsize:<14} // FLS §8.1: frame for {} slot(s)",
            func.stack_slots
        )?;
    }

    for instr in &func.body {
        emit_instr(out, instr, fsize, func.saves_lr)?;
    }

    Ok(())
}

/// Emit one instruction.
///
/// `frame_size` is passed so that `Ret` can restore `sp` before branching.
/// `saves_lr` is passed so that `Ret` can restore `x30` before `ret`.
fn emit_instr(out: &mut String, instr: &Instr, frame_size: u32, saves_lr: bool) -> Result<(), CodegenError> {
    match instr {
        // FLS §6.19: Return expression.
        // ARM64 ABI: return value in x0; `ret` branches to link register x30.
        // Epilogue order (must mirror prologue in reverse):
        //   1. restore sp for local variable frame (if any)
        //   2. restore x30 from lr save slot (if non-leaf)
        //   3. ret
        Instr::Ret(value) => {
            emit_load_x0(out, value)?;
            if frame_size > 0 {
                writeln!(
                    out,
                    "    add     sp, sp, #{frame_size:<14} // FLS §8.1: restore stack frame"
                )?;
            }
            if saves_lr {
                // ARM64 post-indexed load: load x30 from [sp], then sp += 16.
                // This undoes the prologue `str x30, [sp, #-16]!`.
                // FLS §6.12.1: restore lr so `ret` branches to the caller.
                writeln!(
                    out,
                    "    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr"
                )?;
            }
            writeln!(out, "    ret")?;
        }

        // FLS §2.4.4.1: Load integer immediate into virtual register.
        // ARM64: `mov x{reg}, #{n}` covers both positive and negative values.
        //   Positive (0 ≤ n ≤ 65535): assembler emits MOVZ.
        //   Negative (-65536 ≤ n ≤ -1): assembler emits MOVN (move NOT).
        //     Example: `mov x0, #-1` → MOVN x0, #0 (encodes as NOT(0) = -1).
        //
        // The GNU assembler (aarch64-linux-gnu-as) accepts `mov xN, #n`
        // for any i16-range immediate and selects the correct encoding.
        // Values outside the immediate range require MOVZ+MOVK sequences —
        // not yet supported; deferred to a future milestone.
        //
        // FLS §5.2: Negative literal patterns materialise their value via
        // LoadImm with a negative immediate — this is the first caller of
        // negative LoadImm.
        //
        // Cache-line note: one ARM64 instruction (MOVZ or MOVN) = 4 bytes.
        Instr::LoadImm(reg, n) => {
            writeln!(
                out,
                "    mov     x{reg}, #{n:<19} // FLS §2.4.4.1: load imm {n}"
            )?;
        }

        // FLS §7.2: Load from a static variable in the data section.
        //
        // ARM64 addressing: ADRP loads the PC-relative page address; ADD
        // applies the page offset (:lo12:) to form the full address; LDR
        // loads the 64-bit value. Three 4-byte instructions = 12 bytes.
        //
        // FLS §7.2:15: All references to a static refer to the same memory
        // address — unlike const substitution (which inlines a value via MOV),
        // every static reference goes through the data section.
        //
        // Cache-line note: three 4-byte instructions (12 bytes) occupy three
        // slots in the instruction cache line. The loaded value occupies one
        // 8-byte slot in the data cache line — one half of a 16-byte row.
        Instr::LoadStatic { dst, name } => {
            writeln!(out, "    adrp    x{dst}, {name}              // FLS §7.2: static addr (page)")?;
            writeln!(out, "    add     x{dst}, x{dst}, :lo12:{name}  // FLS §7.2: static addr (offset)")?;
            writeln!(out, "    ldr     x{dst}, [x{dst}]             // FLS §7.2: static load")?;
        }

        // FLS §6.5.5: Integer binary arithmetic.
        // ARM64: `add`/`sub`/`mul` operate on 64-bit registers.
        // Virtual register N maps to ARM64 register xN (trivial allocation).
        // Cache-line note: one ARM64 instruction = 4 bytes per arithmetic BinOp.
        //
        // FLS §6.5.3: Comparison operator expressions.
        // ARM64 comparison: `cmp x{lhs}, x{rhs}` sets condition flags;
        // `cset x{dst}, <cond>` materialises 1 or 0 based on flags.
        // Cache-line note: two 4-byte instructions (8 bytes) per comparison.
        // Signed comparison (signed integers are the only type at this milestone).
        Instr::BinOp { op, dst, lhs, rhs } => {
            match op {
                IrBinOp::Add => writeln!(
                    out,
                    "    add     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: add"
                )?,
                IrBinOp::Sub => writeln!(
                    out,
                    "    sub     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: sub"
                )?,
                IrBinOp::Mul => writeln!(
                    out,
                    "    mul     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: mul"
                )?,
                // FLS §6.5.5: Signed integer division.
                // ARM64: `sdiv x{dst}, x{lhs}, x{rhs}` — signed division.
                // FLS §6.23: Division by zero panics; no check is emitted yet.
                // FLS §6.23 AMBIGUOUS: the spec requires a panic but the mechanism
                // (how to raise it without libc) is unspecified at this milestone.
                // Cache-line note: one 4-byte instruction per division.
                IrBinOp::Div => writeln!(
                    out,
                    "    sdiv    x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: div (signed)"
                )?,
                // FLS §6.5.5: Signed integer remainder.
                // Computed as `lhs - (lhs / rhs) * rhs` using two ARM64 instructions:
                //   sdiv x{dst}, x{lhs}, x{rhs}        → x{dst} = lhs / rhs (quotient)
                //   msub x{dst}, x{dst}, x{rhs}, x{lhs} → x{dst} = lhs - dst * rhs
                // ARM64 `msub xd, xn, xm, xa` reads all sources before writing xd,
                // so reusing dst for the intermediate quotient is safe.
                // FLS §6.23: Remainder by zero panics; no check is emitted yet.
                // Cache-line note: two 4-byte instructions = 8 bytes per remainder.
                IrBinOp::Rem => {
                    writeln!(
                        out,
                        "    sdiv    x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: rem step 1: quotient"
                    )?;
                    writeln!(
                        out,
                        "    msub    x{dst}, x{dst}, x{rhs}, x{lhs}  // FLS §6.5.5: rem step 2: lhs - q*rhs"
                    )?;
                }
                // Comparison ops: signed integer comparison on ARM64.
                // `cmp xA, xB` is an alias for `subs xzr, xA, xB` — sets N, Z, C, V flags.
                // `cset xD, cond` sets xD to 1 if the condition holds, 0 otherwise.
                // The condition codes match signed integer semantics (lt, le, gt, ge, eq, ne).
                //
                // FLS §6.5.3: "The type of a comparison expression is bool."
                // ARM64 ABI: bool is represented as 0 or 1 in a 64-bit register.
                //
                // Cache-line note: the two-instruction pair (cmp + cset) is 8 bytes —
                // two adjacent slots in the same cache line. The cmp result (flags)
                // is consumed immediately by cset, so no register is written by cmp.
                IrBinOp::Lt => {
                    writeln!(out, "    cmp     x{lhs}, x{rhs}               // FLS §6.5.3: compare (signed)")?;
                    writeln!(out, "    cset    x{dst}, lt                    // FLS §6.5.3: x{dst} = (x{lhs} < x{rhs})")?;
                }
                IrBinOp::Le => {
                    writeln!(out, "    cmp     x{lhs}, x{rhs}               // FLS §6.5.3: compare (signed)")?;
                    writeln!(out, "    cset    x{dst}, le                    // FLS §6.5.3: x{dst} = (x{lhs} <= x{rhs})")?;
                }
                IrBinOp::Gt => {
                    writeln!(out, "    cmp     x{lhs}, x{rhs}               // FLS §6.5.3: compare (signed)")?;
                    writeln!(out, "    cset    x{dst}, gt                    // FLS §6.5.3: x{dst} = (x{lhs} > x{rhs})")?;
                }
                IrBinOp::Ge => {
                    writeln!(out, "    cmp     x{lhs}, x{rhs}               // FLS §6.5.3: compare (signed)")?;
                    writeln!(out, "    cset    x{dst}, ge                    // FLS §6.5.3: x{dst} = (x{lhs} >= x{rhs})")?;
                }
                IrBinOp::Eq => {
                    writeln!(out, "    cmp     x{lhs}, x{rhs}               // FLS §6.5.3: compare (signed)")?;
                    writeln!(out, "    cset    x{dst}, eq                    // FLS §6.5.3: x{dst} = (x{lhs} == x{rhs})")?;
                }
                IrBinOp::Ne => {
                    writeln!(out, "    cmp     x{lhs}, x{rhs}               // FLS §6.5.3: compare (signed)")?;
                    writeln!(out, "    cset    x{dst}, ne                    // FLS §6.5.3: x{dst} = (x{lhs} != x{rhs})")?;
                }

                // FLS §6.5.6: Bit operator expressions.
                // ARM64: `and`/`orr`/`eor` operate on 64-bit registers.
                // Cache-line note: one 4-byte instruction per bitwise op.
                IrBinOp::BitAnd => writeln!(
                    out,
                    "    and     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.6: bitwise and"
                )?,
                IrBinOp::BitOr => writeln!(
                    out,
                    "    orr     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.6: bitwise or"
                )?,
                IrBinOp::BitXor => writeln!(
                    out,
                    "    eor     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.6: bitwise xor"
                )?,

                // FLS §6.5.7: Shift operator expressions.
                // ARM64: `lsl` (logical shift left) and `asr` (arithmetic shift right).
                // Signed integers use arithmetic right shift (sign-extending) per FLS §6.5.7.
                // FLS §6.5.7 AMBIGUOUS: shift amount is taken modulo bit width; ARM64
                // variable-shift instructions use the low 6 bits of the shift register
                // for 64-bit shifts (mod 64). This matches the FLS description for i32/i64.
                // Cache-line note: one 4-byte instruction per shift.
                IrBinOp::Shl => writeln!(
                    out,
                    "    lsl     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.7: shift left"
                )?,
                IrBinOp::Shr => writeln!(
                    out,
                    "    asr     x{dst}, x{lhs}, x{rhs}          // FLS §6.5.7: arithmetic shift right (signed)"
                )?,

                // FLS §4.1: Unsigned integer division.
                // ARM64: `udiv x{dst}, x{lhs}, x{rhs}` — unsigned division.
                // Used when the operand type is unsigned (IrTy::U32).
                // FLS §6.5.5: Division by zero: galvanic does not yet insert
                // a divide-by-zero check (FLS §6.23 AMBIGUOUS on mechanism).
                // Cache-line note: one 4-byte instruction per unsigned division.
                IrBinOp::UDiv => writeln!(
                    out,
                    "    udiv    x{dst}, x{lhs}, x{rhs}          // FLS §4.1: div (unsigned)"
                )?,

                // FLS §4.1: Unsigned (logical) right shift.
                // ARM64: `lsr x{dst}, x{lhs}, x{rhs}` — logical shift right,
                // zero-extends from the right (vs `asr` which sign-extends).
                // Used when the operand type is unsigned (IrTy::U32).
                // Cache-line note: one 4-byte instruction per unsigned shift.
                IrBinOp::UShr => writeln!(
                    out,
                    "    lsr     x{dst}, x{lhs}, x{rhs}          // FLS §4.1: logical shift right (unsigned)"
                )?,
            }
        }

        // FLS §6.5.4: Arithmetic negation — two's complement negation.
        // ARM64: `neg x{dst}, x{src}` is an alias for `sub x{dst}, xzr, x{src}`.
        // The GNU assembler accepts `neg` directly; it encodes as a 4-byte instruction.
        //
        // FLS §6.1.2:37–45: Runtime instruction — no constant folding.
        // Cache-line note: one 4-byte instruction, same footprint as any other UnOp.
        Instr::Neg { dst, src } => {
            writeln!(
                out,
                "    neg     x{dst}, x{src}               // FLS §6.5.4: negate x{src}"
            )?;
        }

        // FLS §6.5.4: Bitwise NOT `!operand` — complement all bits.
        // ARM64: `mvn x{dst}, x{src}` (alias for `orn xD, xzr, xS`).
        // The GNU assembler accepts `mvn` directly; it encodes as a 4-byte instruction.
        //
        // FLS §6.1.2:37–45: Runtime instruction — no constant folding.
        // Cache-line note: one 4-byte instruction, same footprint as `neg`.
        Instr::Not { dst, src } => {
            writeln!(
                out,
                "    mvn     x{dst}, x{src}               // FLS §6.5.4: bitwise NOT x{src}"
            )?;
        }

        // FLS §6.5.4: Logical NOT `!operand` for boolean values — 0 → 1, 1 → 0.
        // ARM64: `eor x{dst}, x{src}, #1` — XOR source with immediate 1.
        // Since booleans are represented as 0 or 1, XOR with 1 flips bit 0,
        // producing the correct logical complement in a single instruction.
        //
        // Contrast with bitwise NOT (Instr::Not → `mvn`): `mvn` of 0 = -1 (not 1),
        // and `mvn` of 1 = -2 (not 0). `mvn` is wrong for booleans.
        //
        // FLS §6.1.2:37–45: Runtime instruction — no constant folding.
        // Cache-line note: ARM64 `eor` with logical immediate is 4 bytes.
        Instr::BoolNot { dst, src } => {
            writeln!(
                out,
                "    eor     x{dst}, x{src}, #1             // FLS §6.5.4: logical NOT x{src} (bool)"
            )?;
        }

        // FLS §8.1: Store a virtual register to a stack slot.
        // ARM64: `str x{src}, [sp, #{offset}]` — offset = slot * 8.
        // Cache-line note: 8-byte slots keep stores naturally aligned;
        // two slots fill one 16-byte aligned pair.
        Instr::Store { src, slot } => {
            let offset = *slot as u32 * 8;
            writeln!(
                out,
                "    str     x{src}, [sp, #{offset:<15}] // FLS §8.1: store slot {slot}"
            )?;
        }

        // FLS §8.1 / FLS §6.3: Load a stack slot into a virtual register.
        // ARM64: `ldr x{dst}, [sp, #{offset}]` — offset = slot * 8.
        // Cache-line note: naturally aligned 8-byte loads hit L1 in one cycle.
        Instr::Load { dst, slot } => {
            let offset = *slot as u32 * 8;
            writeln!(
                out,
                "    ldr     x{dst}, [sp, #{offset:<15}] // FLS §8.1: load slot {slot}"
            )?;
        }

        // FLS §6.17: Branch target label.
        // Emits `.L{n}:` — a GAS local label. No machine code is emitted;
        // the label resolves to the address of the next instruction.
        // Cache-line note: labels have zero instruction footprint.
        Instr::Label(n) => {
            writeln!(out, ".L{n}:                              // FLS §6.17: branch target")?;
        }

        // FLS §6.17: Unconditional branch.
        // ARM64: `b .L{n}` — a 4-byte PC-relative branch instruction.
        // Cache-line note: ARM64 `b` is 4 bytes — one instruction slot.
        Instr::Branch(n) => {
            writeln!(out, "    b       .L{n:<24} // FLS §6.17: branch to end")?;
        }

        // FLS §6.17: Conditional branch on zero (false condition).
        // ARM64: `cbz x{reg}, .L{label}` — branches if reg == 0 (condition is false).
        // `cbz` ("compare and branch if zero") is a single 4-byte instruction that
        // combines the compare and branch, avoiding a separate `cmp` instruction.
        // Cache-line note: ARM64 `cbz` is 4 bytes — same footprint as `b`.
        Instr::CondBranch { reg, label } => {
            writeln!(
                out,
                "    cbz     x{reg}, .L{label:<21} // FLS §6.17: branch if false"
            )?;
        }

        // FLS §6.12.1: Call expression.
        // ARM64 ABI: integer arguments 0–7 go in x0–x7; return value in x0.
        //
        // For each argument i, if `args[i] != i` we emit `mov x{i}, x{args[i]}`
        // to place the value in the correct register. If `args[i] == i` the
        // value is already in the right place (no move needed).
        //
        // After `bl {name}`, the return value is in x0. We move it to the
        // destination register `dst` (unless dst == 0, already there).
        //
        // Cache-line note: at most `args.len()` move instructions before the
        // `bl` plus one move after — fits in a few cache lines for typical
        // short argument lists.
        //
        // Limitation: this does not handle the "parallel copy" problem where
        // args form a cycle (e.g., args = [1, 0] would incorrectly overwrite).
        // For the current milestone all arguments are freshly materialized
        // immediates or loads, so arg[i] == i always holds in practice.
        Instr::Call { dst, name, args } => {
            // Move arguments to x0, x1, ... as required by the ARM64 ABI.
            for (i, &src_reg) in args.iter().enumerate() {
                if src_reg != i as u8 {
                    writeln!(
                        out,
                        "    mov     x{i}, x{src_reg:<19} // FLS §6.12.1: arg {i}"
                    )?;
                }
            }
            writeln!(out, "    bl      {name:<24} // FLS §6.12.1: call {name}")?;
            // Capture return value from x0 into the destination register.
            if *dst != 0 {
                writeln!(
                    out,
                    "    mov     x{dst}, x0              // FLS §6.12.1: return value → x{dst}"
                )?;
            }
        }

        // FLS §10.1: Return from a `&mut self` method, writing modified fields back.
        //
        // Emits N `ldr` instructions (field 0..N-1 from their stack slots into
        // x0..x{N-1}), then the standard epilogue (sp restore + lr restore + ret).
        //
        // The field loads happen BEFORE sp is restored so that [sp, #slot*8]
        // addresses are still valid (they point into the local frame).
        //
        // FLS §10.1 AMBIGUOUS: The spec does not define the calling convention
        // for &mut self. Galvanic uses a value-copy write-back convention:
        // fields passed in, modified locally, returned in x0..x{N-1}.
        //
        // Cache-line note: N loads = N × 4-byte instructions. For N=2: 8 bytes.
        Instr::RetFields { base_slot, n_fields } => {
            // Load each field from its stack slot into the corresponding return register.
            for i in 0..*n_fields {
                let slot_offset = (*base_slot as usize + i as usize) * 8;
                writeln!(
                    out,
                    "    ldr     x{i}, [sp, #{slot_offset:<16}] // FLS §10.1: write-back field {i}"
                )?;
            }
            // Standard epilogue (mirrors Instr::Ret handling).
            if frame_size > 0 {
                writeln!(
                    out,
                    "    add     sp, sp, #{frame_size:<14} // FLS §8.1: restore stack frame"
                )?;
            }
            if saves_lr {
                writeln!(
                    out,
                    "    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr"
                )?;
            }
            writeln!(out, "    ret")?;
        }

        // FLS §6.9: Indexed array load — `dst = array[index]`.
        //
        // ARM64 two-instruction sequence:
        //   add x{dst}, sp, #(base_slot * 8)         // address of element 0
        //   ldr x{dst}, [x{dst}, x{index_reg}, lsl #3] // load element at index
        //
        // `lsl #3` scales the index by 8 (one slot = 8 bytes), equivalent to
        // multiplying by `sizeof(i32)` rounded up to 8-byte alignment.
        //
        // ARM64 LDR with shifted register: `ldr xD, [xB, xI, lsl #3]` is a
        // single 4-byte instruction that reads from address `xB + xI*8`.
        // The shift amount (3) is encoded directly in the instruction encoding.
        //
        // Precondition: `dst != index_reg`. If they were the same, the `add`
        // would overwrite `index_reg` before the `ldr` can use it. The lowering
        // pass guarantees `dst` is a freshly allocated register that does not
        // alias `index_reg` (both come from `alloc_reg()` in sequence).
        //
        // FLS §6.9 AMBIGUOUS: Out-of-bounds access must panic; no check is emitted.
        //
        // Cache-line note: add + ldr = two 4-byte instructions = 8 bytes,
        // fitting in one adjacent instruction slot pair in a 64-byte cache line.
        Instr::LoadIndexed { dst, base_slot, index_reg } => {
            let base_offset = *base_slot as u32 * 8;
            writeln!(
                out,
                "    add     x{dst}, sp, #{base_offset:<15} // FLS §6.9: address of arr[0]"
            )?;
            writeln!(
                out,
                "    ldr     x{dst}, [x{dst}, x{index_reg}, lsl #3] // FLS §6.9: load arr[index]"
            )?;
        }

        // FLS §6.5.10 + §6.9: Store to an indexed array element `arr[index] = src`.
        //
        // Two-instruction sequence:
        //   add x{scratch}, sp, #{base_slot*8}  — base address of arr[0]
        //   str x{src}, [x{scratch}, x{index_reg}, lsl #3]  — store arr[index]
        //
        // The `lsl #3` scales the index by 8 (bytes per slot), matching the
        // element layout established by array literal stores.
        //
        // FLS §6.9 AMBIGUOUS: No bounds check emitted at this milestone.
        //
        // Cache-line note: add + str = two 4-byte instructions = 8 bytes,
        // mirroring LoadIndexed. The pair fits in one adjacent instruction
        // slot pair in a 64-byte cache line.
        Instr::StoreIndexed { src, base_slot, index_reg, scratch } => {
            let base_offset = *base_slot as u32 * 8;
            writeln!(
                out,
                "    add     x{scratch}, sp, #{base_offset:<15} // FLS §6.9: address of arr[0]"
            )?;
            writeln!(
                out,
                "    str     x{src}, [x{scratch}, x{index_reg}, lsl #3] // FLS §6.5.10: store arr[index]"
            )?;
        }

        // FLS §6.12.2 + §10.1: Call a `&mut self` method and write modified fields back.
        //
        // After the `bl`, x0..x{N-1} hold the method's modified field values
        // (returned via `RetFields`). Store them immediately back to the caller's
        // struct slots so subsequent field reads see the updated values.
        //
        // Cache-line note: arg moves + bl + N stores. For a 2-field struct the
        // write-back is 2 × 4-byte `str` = 8 bytes, fitting in one cache line
        // alongside the `bl`.
        Instr::CallMut { name, args, write_back_slot, n_fields } => {
            // Move arguments to x0, x1, ... (struct fields first, then extra args).
            for (i, &src_reg) in args.iter().enumerate() {
                if src_reg != i as u8 {
                    writeln!(
                        out,
                        "    mov     x{i}, x{src_reg:<19} // FLS §6.12.2: arg {i}"
                    )?;
                }
            }
            writeln!(out, "    bl      {name:<24} // FLS §6.12.2: call &mut self {name}")?;
            // Write back modified fields from x0..x{N-1} to the struct's stack slots.
            for i in 0..*n_fields {
                let slot_offset = (*write_back_slot as usize + i as usize) * 8;
                writeln!(
                    out,
                    "    str     x{i}, [sp, #{slot_offset:<16}] // FLS §10.1: write-back field {i}"
                )?;
            }
        }

        // FLS §10.1: Return modified struct fields AND a scalar value from a
        // `&mut self` method.
        //
        // Convention: fields in x0..x{N-1} (write-back for caller), scalar in x{N}.
        //
        // Emits:
        //   ldr x{i}, [sp, #{(base_slot+i)*8}]  for i in 0..n_fields  — field loads
        //   mov x{n_fields}, x{val_reg}                                — return value
        //   standard epilogue + ret
        //
        // FLS §10.1 AMBIGUOUS: The spec does not define the calling convention for
        // &mut self with a non-unit return type. This extends RetFields by placing
        // the scalar return value in x{n_fields}.
        //
        // Cache-line note: (N+1) × 4-byte loads/moves before epilogue.
        Instr::RetFieldsAndValue { base_slot, n_fields, val_reg } => {
            // Load each field from its stack slot into the corresponding write-back register.
            for i in 0..*n_fields {
                let slot_offset = (*base_slot as usize + i as usize) * 8;
                writeln!(
                    out,
                    "    ldr     x{i}, [sp, #{slot_offset:<16}] // FLS §10.1: write-back field {i}"
                )?;
            }
            // Place scalar return value in x{n_fields}.
            let ret_reg = *n_fields;
            if *val_reg != ret_reg {
                writeln!(
                    out,
                    "    mov     x{ret_reg}, x{val_reg:<18} // FLS §10.1: scalar return value"
                )?;
            }
            // Standard epilogue (mirrors Instr::RetFields handling).
            if frame_size > 0 {
                writeln!(
                    out,
                    "    add     sp, sp, #{frame_size:<14} // FLS §8.1: restore stack frame"
                )?;
            }
            if saves_lr {
                writeln!(
                    out,
                    "    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr"
                )?;
            }
            writeln!(out, "    ret")?;
        }

        // FLS §10.1: Call a `&mut self` method returning a scalar, write back fields,
        // and capture the scalar return value.
        //
        // After `bl`, x0..x{N-1} hold modified field values and x{N} holds the
        // scalar return. Write x0..x{N-1} back to the receiver's stack slots, then
        // move x{N} into `dst`.
        //
        // FLS §6.12.2: Method call expressions — dispatched to mangled name.
        // FLS §10.1: Write-back convention extended for scalar returns.
        //
        // Cache-line note: arg moves + bl + N write-back stores + 1 capture move.
        Instr::CallMutReturn { name, args, write_back_slot, n_fields, dst } => {
            // Move arguments to x0, x1, ... (struct fields first, then extra args).
            for (i, &src_reg) in args.iter().enumerate() {
                if src_reg != i as u8 {
                    writeln!(
                        out,
                        "    mov     x{i}, x{src_reg:<19} // FLS §6.12.2: arg {i}"
                    )?;
                }
            }
            writeln!(out, "    bl      {name:<24} // FLS §6.12.2: call &mut self {name}")?;
            // Write back modified fields from x0..x{N-1} to the struct's stack slots.
            for i in 0..*n_fields {
                let slot_offset = (*write_back_slot as usize + i as usize) * 8;
                writeln!(
                    out,
                    "    str     x{i}, [sp, #{slot_offset:<16}] // FLS §10.1: write-back field {i}"
                )?;
            }
            // Capture scalar return value from x{n_fields} into dst.
            let ret_reg = *n_fields;
            if *dst != ret_reg {
                writeln!(
                    out,
                    "    mov     x{dst}, x{ret_reg:<18} // FLS §10.1: capture scalar return"
                )?;
            }
        }
    }
    Ok(())
}

/// Emit instructions that place `value` into `x0` for return.
///
/// ARM64 note: `mov x0, #n` assembles to `MOVZ x0, #n` for 0 ≤ n ≤ 65535.
/// Negative values and values > 65535 require multi-instruction sequences
/// and are not yet supported.
///
/// FLS §2.4.4.1: Integer literals.
/// FLS §6.19: Return expressions — result in x0.
/// Cache-line note: each `mov` is 4 bytes — one slot in a 16-instruction
/// cache line. When the result is already in x0 (Reg(0)), no move is needed.
fn emit_load_x0(out: &mut String, value: &IrValue) -> Result<(), CodegenError> {
    match value {
        IrValue::I32(n) => {
            // ARM64: `mov x0, #n` for both positive and negative immediates.
            // Negative values encode as MOVN (GNU assembler selects automatically).
            writeln!(out, "    mov     x0, #{n}             // FLS §2.4.4.1: integer literal {n}")?;
        }
        IrValue::Unit => {
            // FLS §4.4: unit return. Convention: exit code 0 for main.
            writeln!(out, "    mov     x0, #0              // FLS §4.4: unit return")?;
        }
        IrValue::Reg(r) => {
            if *r == 0 {
                // Result already in x0 — no move needed.
                // Cache-line note: omitting the redundant mov saves 4 bytes
                // and keeps the return sequence as tight as possible.
            } else {
                // Move result from x{r} to x0 for the ARM64 return convention.
                // FLS §6.19: return value is placed in x0.
                writeln!(
                    out,
                    "    mov     x0, x{r}              // FLS §6.19: return reg {r} → x0"
                )?;
            }
        }
    }
    Ok(())
}

// ── Entry point stub ──────────────────────────────────────────────────────────

/// Emit the `_start` ELF entry point.
///
/// `_start` calls `main` and passes its return value to `sys_exit`.
///
/// FLS §18.1: The `main` function is the program entry point. On Linux ELF
/// the actual entry symbol is `_start`; calling `main` from there and
/// exiting is the standard bare-metal bootstrap pattern.
///
/// ARM64 Linux syscall ABI:
/// - syscall number in `x8`
/// - first arg in `x0`
/// - `svc #0` to invoke
/// - `__NR_exit` = 93
///
/// Cache-line note: `_start` is 3 instructions (12 bytes), fits in the
/// first quarter of a 64-byte cache line.
fn emit_start(out: &mut String) -> Result<(), CodegenError> {
    writeln!(out, "    // ELF entry point — FLS §18.1")?;
    writeln!(out, "    .global _start")?;
    writeln!(out, "_start:")?;
    writeln!(out, "    bl      main            // call fn main()")?;
    writeln!(out, "    // x0 = main()'s return value")?;
    writeln!(out, "    mov     x8, #93         // __NR_exit (ARM64 Linux)")?;
    writeln!(out, "    svc     #0              // exit(x0)")?;
    Ok(())
}
