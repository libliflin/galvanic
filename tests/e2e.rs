//! End-to-end compilation tests.
//!
//! These tests run galvanic's full pipeline (lex → parse → lower → codegen →
//! assemble → link) and execute the resulting ARM64 binary, verifying that
//! the correct exit code is produced.
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

/// Milestone 3: `fn main() -> i32 { let x = 42; x }` exits with code 42.
///
/// FLS §8.1: Let statement.
/// FLS §6.3: Path expression.
#[test]
fn milestone_3_let_binding() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { let x = 42; x }\n") else {
        return;
    };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 3 (compound): multiple bindings and arithmetic.
#[test]
fn milestone_3_let_bindings_add() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { let x = 3; let y = 4; x + y }\n")
    else {
        return;
    };
    assert_eq!(exit_code, 7, "expected exit 7 (3+4), got {exit_code}");
}

// ── Milestone 4: if/else control flow ────────────────────────────────────────

/// Milestone 4: `if true { 1 } else { 0 }` exits with 1.
///
/// FLS §6.17: If expression.
/// FLS §2.4.7: Boolean literal `true`.
#[test]
fn milestone_4_if_true() {
    let Some(exit_code) =
        compile_and_run("fn main() -> i32 { if true { 1 } else { 0 } }\n")
    else {
        return;
    };
    assert_eq!(exit_code, 1, "expected exit 1 (if true), got {exit_code}");
}

/// Milestone 4 (variant): `if false { 1 } else { 0 }` exits with 0.
///
/// FLS §6.17: If expression — `false` selects else branch.
#[test]
fn milestone_4_if_false() {
    let Some(exit_code) =
        compile_and_run("fn main() -> i32 { if false { 1 } else { 0 } }\n")
    else {
        return;
    };
    assert_eq!(exit_code, 0, "expected exit 0 (if false), got {exit_code}");
}

/// Milestone 4 (compound): if/else with let bindings in branches.
#[test]
fn milestone_4_if_with_let() {
    let Some(exit_code) =
        compile_and_run("fn main() -> i32 { if true { let x = 5; x } else { 0 } }\n")
    else {
        return;
    };
    assert_eq!(exit_code, 5, "expected exit 5, got {exit_code}");
}

// ── Milestone 5: function calls ──────────────────────────────────────────────

/// Milestone 5: calling a zero-argument function.
///
/// FLS §6.12.1: Call expression.
#[test]
fn milestone_5_call_no_args() {
    let Some(exit_code) =
        compile_and_run("fn answer() -> i32 { 42 }\nfn main() -> i32 { answer() }\n")
    else {
        return;
    };
    assert_eq!(exit_code, 42, "expected exit 42 (answer()), got {exit_code}");
}

/// Milestone 5 (variant): calling a function with one argument.
///
/// FLS §6.12.1: Call expression with one argument.
/// FLS §9: Parameter binding.
#[test]
fn milestone_5_call_with_arg() {
    let Some(exit_code) = compile_and_run(
        "fn double(x: i32) -> i32 { x + x }\nfn main() -> i32 { double(21) }\n",
    ) else {
        return;
    };
    assert_eq!(exit_code, 42, "expected exit 42 (double(21)), got {exit_code}");
}

/// Milestone 5 (chain): chained calls — main calls add, add calls double.
///
/// FLS §6.12.1: Nested call expressions.
#[test]
fn milestone_5_call_chained() {
    let Some(exit_code) = compile_and_run(
        "fn double(x: i32) -> i32 { x + x }\n\
         fn add(a: i32, b: i32) -> i32 { a + b }\n\
         fn main() -> i32 { add(double(10), double(11)) }\n",
    ) else {
        return;
    };
    assert_eq!(
        exit_code, 42,
        "expected exit 42 (add(double(10), double(11))), got {exit_code}"
    );
}
