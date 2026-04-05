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
/// If the function has local variables (`stack_slots > 0`) the prologue
/// subtracts from `sp` to reserve space, and the epilogue (emitted as part
/// of each `Ret` instruction) restores `sp` before returning.
///
/// Cache-line note: `sub sp, sp, #N` is one 4-byte instruction — the frame
/// setup occupies one slot in the first cache line of the function body.
fn emit_fn(out: &mut String, func: &crate::ir::IrFn) -> Result<(), CodegenError> {
    writeln!(out, "    // fn {} — FLS §9", func.name)?;
    writeln!(out, "    .global {}", func.name)?;
    writeln!(out, "{}:", func.name)?;

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
        emit_instr(out, instr, fsize)?;
    }

    Ok(())
}

/// Emit one instruction.
///
/// `frame_size` is passed so that `Ret` can restore `sp` before branching.
fn emit_instr(out: &mut String, instr: &Instr, frame_size: u32) -> Result<(), CodegenError> {
    match instr {
        // FLS §6.19: Return expression.
        // ARM64 ABI: return value in x0; `ret` branches to link register x30.
        // If the function has a stack frame, restore sp before returning so
        // the caller's stack is intact.
        Instr::Ret(value) => {
            emit_load_x0(out, value)?;
            if frame_size > 0 {
                writeln!(
                    out,
                    "    add     sp, sp, #{frame_size:<14} // FLS §8.1: restore stack frame"
                )?;
            }
            writeln!(out, "    ret")?;
        }

        // FLS §2.4.4.1: Load integer immediate into virtual register.
        // ARM64: `mov x{reg}, #{n}` assembles to MOVZ for 0 ≤ n ≤ 65535.
        // Negative values and values > 65535 are not yet supported.
        // Cache-line note: one MOVZ instruction = 4 bytes per LoadImm.
        Instr::LoadImm(reg, n) => {
            if *n < 0 {
                return Err(CodegenError::Unsupported(
                    "negative integer immediate (MOVN not yet implemented)".into(),
                ));
            }
            writeln!(
                out,
                "    mov     x{reg}, #{n:<19} // FLS §2.4.4.1: load imm {n}"
            )?;
        }

        // FLS §6.5.5: Integer binary arithmetic.
        // ARM64: `add`/`sub`/`mul` operate on 64-bit registers.
        // Virtual register N maps to ARM64 register xN (trivial allocation).
        // Cache-line note: one ARM64 instruction = 4 bytes per BinOp.
        Instr::BinOp { op, dst, lhs, rhs } => {
            let mnemonic = match op {
                IrBinOp::Add => "add",
                IrBinOp::Sub => "sub",
                IrBinOp::Mul => "mul",
            };
            writeln!(
                out,
                "    {mnemonic:<7} x{dst}, x{lhs}, x{rhs}          // FLS §6.5.5: {mnemonic}"
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
            if *n < 0 {
                // Negative immediates require MOVN — not yet implemented.
                return Err(CodegenError::Unsupported(
                    "negative integer return value".into(),
                ));
            }
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
