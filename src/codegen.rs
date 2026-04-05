//! ARM64 (AArch64) assembly code generation for galvanic.
//!
//! Milestone 1: emit a bare Linux `_start` entry point that loads the main
//! function's return value into `x0` and invokes the exit(2) syscall.
//!
//! # Target
//!
//! Architecture: AArch64 (ARM64)
//! OS ABI:       Linux ELF (bare syscall, no libc dependency)
//! Assembler:    GNU as (`aarch64-linux-gnu-as`)
//!
//! # Bootstrap strategy
//!
//! Emitting assembly text (`.s` files) and shelling out to GNU `as` and `ld`
//! is the bootstrapping approach for the first milestones. A built-in
//! instruction encoder will be added when the set of emitted instruction
//! forms grows enough to justify it.
//!
//! # Cache-line notes (milestone 1)
//!
//! The `_start` stub for milestone 1 is three instructions (12 bytes), well
//! within one 64-byte cache line. Explicit `.p2align` directives and
//! hot/cold function separation will be added as the emitted code grows.

use crate::ir::{IrFn, IrInst, Program};

/// Emit ARM64 GNU assembly text for the given IR program.
///
/// Returns the complete contents of a `.s` file suitable for passing to
/// `aarch64-linux-gnu-as`.
///
/// FLS §9: Each `IrFn` in the `Program` corresponds to one function.
/// FLS §18.1: The `main` function is treated as the program entry point;
/// its body is inlined into the `_start` symbol.
pub fn emit_asm(program: &Program) -> String {
    let mut out = String::with_capacity(256);

    // Section and global directives.
    //
    // Cache-line note: `.text` on AArch64 defaults to 4-byte (single
    // instruction) alignment. We will add explicit `.p2align 6` (64-byte
    // cache-line alignment) for hot functions once we have multiple functions
    // and profiling data.
    out.push_str("    .text\n");
    out.push_str("    .global _start\n");
    out.push_str("_start:\n");

    if let Some(main_fn) = program.fns.iter().find(|f| f.name == "main") {
        // Inline main's body directly into _start.
        // Milestone 1: main has exactly one instruction.
        emit_fn_body(&mut out, main_fn);
    } else {
        // No main: emit exit(0) as a diagnostic fallback.
        // FLS §18.1 NOTE: a crate without a main function is not a valid
        // Rust program. This path should only be reached in error cases.
        emit_exit(&mut out, 0);
    }

    out
}

/// Emit the body of a function as inline ARM64 instructions.
fn emit_fn_body(out: &mut String, f: &IrFn) {
    for inst in &f.body {
        match inst {
            IrInst::ReturnInt(n) => emit_exit(out, *n),
        }
    }
}

/// Emit the AArch64 Linux `exit` syscall with the given exit code.
///
/// AArch64 Linux syscall convention (AAPCS64 + Linux ABI):
/// - `x0`: first argument (exit status)
/// - `x8`: syscall number (`__NR_exit` = 93)
/// - `svc #0`: enter the kernel
///
/// FLS §6.19: Return expressions map to this stub at the code-gen level.
///
/// # Cache-line note
///
/// Three instructions = 12 bytes. The entire `_start` stub for milestone 1
/// fits in one 64-byte cache line, with 52 bytes to spare. No alignment
/// padding is needed at this scale.
///
/// # Immediate encoding
///
/// `mov x0, #N` is valid for `N` in 0..=65535. For exit codes (0–255)
/// this is always sufficient. Values ≥ 65536 would require `movz`/`movk`
/// — that is noted here as future work for milestone 2+.
fn emit_exit(out: &mut String, code: i64) {
    // Clamp to [0, 255]: the Linux kernel masks the exit status to 8 bits
    // (WEXITSTATUS uses bits 8–15 of the wait status word). Values outside
    // this range from an i32 main are implementation-defined in practice.
    // We clamp here so `mov x0, #N` always uses a value ≤ 255.
    //
    // FLS §6.23 AMBIGUOUS: The spec does not specify what happens when a
    // main function returns an integer outside the range representable as a
    // process exit code. This clamping is a pragmatic choice, not an FLS
    // requirement.
    let clamped = code.clamp(0, 255);
    out.push_str(&format!("    mov x0, #{clamped}\n"));
    out.push_str("    mov x8, #93\n");
    out.push_str("    svc #0\n");
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{IrFn, IrInst, Program};

    fn program_with_main_returning(n: i64) -> Program {
        Program {
            fns: vec![IrFn {
                name: "main".to_string(),
                body: vec![IrInst::ReturnInt(n)],
            }],
        }
    }

    /// FLS §9: emit_asm produces well-structured assembly with _start symbol.
    #[test]
    fn emit_asm_has_start_symbol() {
        let asm = emit_asm(&program_with_main_returning(0));
        assert!(asm.contains("_start:"), "missing _start label");
        assert!(asm.contains(".global _start"), "missing .global directive");
        assert!(asm.contains(".text"), "missing .text section");
    }

    /// FLS §6.19 + §2.4.4.1: return value 0 → `mov x0, #0`.
    #[test]
    fn return_0_emits_correct_exit_sequence() {
        let asm = emit_asm(&program_with_main_returning(0));
        assert!(asm.contains("mov x0, #0"), "missing exit code 0");
        assert!(asm.contains("mov x8, #93"), "missing __NR_exit");
        assert!(asm.contains("svc #0"), "missing svc instruction");
    }

    /// Exit code 42 is emitted correctly.
    #[test]
    fn return_42_emits_correct_exit_code() {
        let asm = emit_asm(&program_with_main_returning(42));
        assert!(asm.contains("mov x0, #42"));
    }

    /// No-main program emits exit(0) as fallback.
    #[test]
    fn no_main_emits_fallback_exit_0() {
        let p = Program { fns: vec![] };
        let asm = emit_asm(&p);
        assert!(asm.contains("mov x0, #0"));
        assert!(asm.contains("mov x8, #93"));
    }
}
