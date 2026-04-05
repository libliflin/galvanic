//! End-to-end compilation tests.
//!
//! These tests run galvanic's full pipeline (lex → parse → lower → codegen →
//! assemble → link) and execute the resulting ARM64 binary, verifying that
//! the correct exit code is produced.
//!
//! # FLS constraint compliance
//!
//! Only tests that produce correct **runtime** code are included here.
//! Tests for features that previously relied on compile-time interpretation
//! (let bindings, if/else, while, loop, function calls, break, continue,
//! return) have been removed — those features must be re-implemented with
//! proper runtime codegen (branches, stack frames, etc.) per FLS §6.1.2:37–45.
//!
//! # Prerequisites
//!
//! - `aarch64-linux-gnu-as` and `aarch64-linux-gnu-ld`
//!   (from `gcc-aarch64-linux-gnu` / `binutils-aarch64-linux-gnu`)
//! - `qemu-aarch64` for running ARM64 binaries on non-ARM64 hosts
//!   (from `qemu-user`)
//!
//! Tests are **skipped** (return early, not failed) when these tools are
//! absent. On CI (ubuntu-latest), the e2e job installs them explicitly.

use std::process::Command;

// ── Assembly inspection helper ────────────────────────────────────────────────

/// Compile source through galvanic and return the emitted ARM64 assembly text.
///
/// This helper drives the full lex → parse → lower → codegen pipeline without
/// assembling or running the output. Used to verify that the compiler emits
/// the correct instruction forms (e.g., `add` for `1 + 2`), not just that
/// the exit code is correct.
///
/// FLS §6.1.2:37–45: Non-const code must emit runtime instructions. A test
/// that only checks the exit code cannot distinguish "compiled correctly" from
/// "evaluated at compile time and emitted the constant result." Assembly
/// inspection closes that gap.
fn compile_to_asm(source: &str) -> String {
    let tokens = galvanic::lexer::tokenize(source).expect("lex failed");
    let sf = galvanic::parser::parse(&tokens, source).expect("parse failed");
    let module = galvanic::lower::lower(&sf, source).expect("lower failed");
    galvanic::codegen::emit_asm(&module).expect("codegen failed")
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return true if `tool` is present in PATH.
fn tool_available(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Returns true if all required tools (assembler, linker, QEMU) are available.
fn tools_available() -> bool {
    if std::env::consts::OS != "linux" {
        return false;
    }
    let as_ok = tool_available("aarch64-linux-gnu-as");
    let ld_ok = tool_available("aarch64-linux-gnu-ld");
    let run_ok = std::env::consts::ARCH == "aarch64"
        || tool_available("qemu-aarch64");
    as_ok && ld_ok && run_ok
}

/// Compile an inline source string through galvanic with `-o output`, run the
/// resulting binary, and return its exit code.
fn compile_and_run(source: &str) -> Option<i32> {
    if !tools_available() {
        eprintln!("e2e: skipping — aarch64 cross tools or qemu not available");
        return None;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let src_path = dir.path().join("main.rs");
    let bin_path = dir.path().join("main");

    std::fs::write(&src_path, source).expect("write fixture");

    let status = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(&src_path)
        .args(["-o", bin_path.to_str().unwrap()])
        .status()
        .expect("failed to run galvanic");

    assert!(
        status.success(),
        "galvanic failed to compile (exit {status})"
    );

    let run_status = if std::env::consts::ARCH == "aarch64" {
        Command::new(&bin_path)
            .status()
            .expect("failed to run binary natively")
    } else {
        Command::new("qemu-aarch64")
            .arg(&bin_path)
            .status()
            .expect("failed to run binary via qemu-aarch64")
    };

    Some(run_status.code().expect("binary terminated by signal"))
}

// ── Milestone 1: basic returns ───────────────────────────────────────────────

/// Milestone 1: `fn main() -> i32 { 0 }` exits with code 0.
///
/// FLS §9: fn main with i32 return type.
/// FLS §18.1: main is the program entry point.
/// FLS §2.4.4.1: 0 is an integer literal.
#[test]
fn milestone_1_main_returns_zero() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 0 }\n") else {
        return;
    };
    assert_eq!(exit_code, 0, "expected exit 0, got {exit_code}");
}

/// Milestone 1 (variant): `fn main() -> i32 { 42 }` exits with code 42.
///
/// FLS §2.4.4.1: 42 is an integer literal.
#[test]
fn milestone_1_main_returns_nonzero() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 42 }\n") else {
        return;
    };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 1 (implicit unit): `fn main() {}` exits with code 0.
///
/// FLS §9: "If no return type is specified, the return type is `()`."
/// FLS §4.4: Unit type convention — exit code 0 for main.
#[test]
fn milestone_1_main_unit_return() {
    let Some(exit_code) = compile_and_run("fn main() {}\n") else {
        return;
    };
    assert_eq!(exit_code, 0, "expected exit 0 for unit main, got {exit_code}");
}

// ── Milestone 2: arithmetic ──────────────────────────────────────────────────
//
// These tests verify both correct exit codes AND correct runtime instruction
// emission. FLS §6.1.2:37–45: arithmetic in non-const code must emit runtime
// instructions, not be constant-folded.

/// Milestone 2: `fn main() -> i32 { 1 + 2 }` exits with code 3.
///
/// FLS §6.5.5: Addition operator `+`.
#[test]
fn milestone_2_add() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 1 + 2 }\n") else {
        return;
    };
    assert_eq!(exit_code, 3, "expected exit 3 (1+2), got {exit_code}");
}

/// Milestone 2 (variant): `fn main() -> i32 { 10 - 3 }` exits with code 7.
///
/// FLS §6.5.5: Subtraction operator `-`.
#[test]
fn milestone_2_sub() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 10 - 3 }\n") else {
        return;
    };
    assert_eq!(exit_code, 7, "expected exit 7 (10-3), got {exit_code}");
}

/// Milestone 2 (variant): `fn main() -> i32 { 3 * 4 }` exits with code 12.
///
/// FLS §6.5.5: Multiplication operator `*`.
#[test]
fn milestone_2_mul() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 3 * 4 }\n") else {
        return;
    };
    assert_eq!(exit_code, 12, "expected exit 12 (3*4), got {exit_code}");
}

/// Milestone 2 (compound): `fn main() -> i32 { 1 + 2 + 3 }` exits with code 6.
///
/// FLS §6.21: Expression precedence — `+` is left-associative.
#[test]
fn milestone_2_nested_add() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 1 + 2 + 3 }\n") else {
        return;
    };
    assert_eq!(exit_code, 6, "expected exit 6 (1+2+3), got {exit_code}");
}

// ── Milestone 3: let bindings ────────────────────────────────────────────────
//
// FLS §8.1: Let statements create local variable bindings. The initializer
// is evaluated and stored on the stack; path expressions load from the stack.
// FLS §6.1.2:37–45: All storage and loads are runtime instructions.

/// Milestone 3: `fn main() -> i32 { let x = 42; x }` exits with code 42.
///
/// FLS §8.1: Let statement with integer initializer.
/// FLS §6.3: Path expression reads the local variable.
#[test]
fn milestone_3_let_binding_literal() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { let x = 42; x }\n") else {
        return;
    };
    assert_eq!(exit_code, 42, "expected exit 42 from `let x = 42; x`, got {exit_code}");
}

/// Milestone 3 (variant): `fn main() -> i32 { let x = 1 + 2; x }` exits with 3.
///
/// FLS §8.1: Let statement initializer can be an arithmetic expression.
/// FLS §6.5.5: The arithmetic runs at runtime before being stored.
#[test]
fn milestone_3_let_binding_arithmetic() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { let x = 1 + 2; x }\n") else {
        return;
    };
    assert_eq!(exit_code, 3, "expected exit 3 from `let x = 1 + 2; x`, got {exit_code}");
}

/// Milestone 3 (two bindings): `fn main() -> i32 { let x = 10; let y = 5; x }`.
///
/// Tests that multiple let bindings allocate separate stack slots.
/// FLS §8.1: Each let statement introduces a distinct binding.
#[test]
fn milestone_3_two_let_bindings() {
    let Some(exit_code) =
        compile_and_run("fn main() -> i32 { let x = 10; let y = 5; x }\n")
    else {
        return;
    };
    assert_eq!(exit_code, 10, "expected exit 10, got {exit_code}");
}

// ── Assembly inspection: runtime instruction verification ────────────────────
//
// FLS §6.1.2:37–45: Non-const code must emit runtime instructions.
// An exit-code test alone cannot prove compliance: both `mov x0, #3; ret`
// (interpreter) and `mov x0, #1; mov x1, #2; add x2, x0, x1; ...` (compiler)
// produce exit code 3 for `1 + 2`. Assembly inspection is required.

/// `fn main() -> i32 { 1 + 2 }` must emit an `add` instruction at runtime.
///
/// FLS §6.5.5: Addition operator `+`.
/// FLS §6.1.2:37–45: Non-const arithmetic must emit runtime instructions.
#[test]
fn runtime_add_emits_add_instruction() {
    let asm = compile_to_asm("fn main() -> i32 { 1 + 2 }\n");
    assert!(
        asm.contains("add"),
        "expected `add` instruction in assembly for `1 + 2`, got:\n{asm}"
    );
    assert!(
        !asm.contains("mov     x0, #3"),
        "assembly must not fold `1 + 2` to constant #3:\n{asm}"
    );
}

/// `fn main() -> i32 { 10 - 3 }` must emit a `sub` instruction at runtime.
///
/// FLS §6.5.5: Subtraction operator `-`.
/// FLS §6.1.2:37–45: Non-const arithmetic must emit runtime instructions.
#[test]
fn runtime_sub_emits_sub_instruction() {
    let asm = compile_to_asm("fn main() -> i32 { 10 - 3 }\n");
    assert!(
        asm.contains("sub"),
        "expected `sub` instruction in assembly for `10 - 3`, got:\n{asm}"
    );
}

/// `fn main() -> i32 { 3 * 4 }` must emit a `mul` instruction at runtime.
///
/// FLS §6.5.5: Multiplication operator `*`.
/// FLS §6.1.2:37–45: Non-const arithmetic must emit runtime instructions.
#[test]
fn runtime_mul_emits_mul_instruction() {
    let asm = compile_to_asm("fn main() -> i32 { 3 * 4 }\n");
    assert!(
        asm.contains("mul"),
        "expected `mul` instruction in assembly for `3 * 4`, got:\n{asm}"
    );
}

/// Nested arithmetic `1 + 2 + 3` emits multiple `add` instructions.
///
/// FLS §6.21: Expression precedence — `+` is left-associative, so
/// `1 + 2 + 3` parses as `(1 + 2) + 3` and requires two `add` instructions.
/// FLS §6.1.2:37–45: Both additions must execute at runtime.
#[test]
fn runtime_nested_add_emits_multiple_add_instructions() {
    let asm = compile_to_asm("fn main() -> i32 { 1 + 2 + 3 }\n");
    let add_count = asm.matches("add").count();
    assert!(
        add_count >= 2,
        "expected at least 2 `add` instructions for `1 + 2 + 3`, found {add_count}:\n{asm}"
    );
}

/// `fn main() -> i32 { let x = 42; x }` must emit `str` and `ldr`.
///
/// FLS §8.1: The let statement stores 42 to a stack slot at runtime.
/// FLS §6.3: The path expression `x` loads from that stack slot at runtime.
/// FLS §6.1.2:37–45: Neither the store nor the load may be elided.
#[test]
fn runtime_let_binding_emits_str_and_ldr() {
    let asm = compile_to_asm("fn main() -> i32 { let x = 42; x }\n");
    assert!(
        asm.contains("str"),
        "expected `str` instruction for let binding, got:\n{asm}"
    );
    assert!(
        asm.contains("ldr"),
        "expected `ldr` instruction for path expression, got:\n{asm}"
    );
    // The frame setup must subtract from sp.
    assert!(
        asm.contains("sub     sp"),
        "expected `sub sp` for stack frame, got:\n{asm}"
    );
    // The frame restore must add back to sp before ret.
    assert!(
        asm.contains("add     sp"),
        "expected `add sp` for stack restore before ret, got:\n{asm}"
    );
}
