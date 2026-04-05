//! End-to-end compilation tests.
//!
//! These tests run galvanic's full pipeline (lex → parse → IR → codegen →
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

/// Compile `fixture` through galvanic with `-o output`.
///
/// Returns `Some(())` on success, or `None` if the required ARM64 toolchain
/// is not available (skipping the test gracefully).
fn compile_fixture(fixture: &str, output: &str) -> Option<()> {
    if !tool_available("aarch64-linux-gnu-as") || !tool_available("aarch64-linux-gnu-ld") {
        eprintln!(
            "e2e: skipping — aarch64 cross tools not available \
             (install gcc-aarch64-linux-gnu)"
        );
        return None;
    }

    let fixture_path = format!(
        "{}/tests/fixtures/{fixture}",
        env!("CARGO_MANIFEST_DIR")
    );

    let status = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(&fixture_path)
        .args(["-o", output])
        .status()
        .expect("failed to run galvanic");

    assert!(
        status.success(),
        "galvanic failed to compile {fixture} (exit {status})"
    );

    Some(())
}

/// Run the binary at `path` and return its exit code.
///
/// Uses `qemu-aarch64` on non-native hosts. Returns `None` if the binary
/// cannot be executed in this environment (test is skipped).
fn run_binary(path: &str) -> Option<i32> {
    let result = if cfg!(target_arch = "aarch64") {
        // Native ARM64: run directly.
        Command::new(path).status()
    } else if tool_available("qemu-aarch64") {
        // Cross-host: run under QEMU user-mode emulation.
        Command::new("qemu-aarch64").arg(path).status()
    } else {
        eprintln!(
            "e2e: skipping binary execution — not on ARM64 and \
             qemu-aarch64 not available (install qemu-user)"
        );
        return None;
    };

    let status = result.expect("failed to run binary");
    Some(status.code().unwrap_or(-1))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Milestone 1: `fn main() -> i32 { 0 }` exits with code 0.
///
/// FLS §9: Functions — main is the entry point.
/// FLS §2.4.4.1: Integer literals — `0` is a valid integer literal.
/// FLS §6.2: Literal expressions.
/// FLS §18.1: Crate entry point.
#[test]
fn milestone_1_main_returns_0() {
    let output = "/tmp/galvanic_e2e_milestone_1";

    let Some(()) = compile_fixture("milestone_1.rs", output) else {
        return; // toolchain not available — skip
    };

    let Some(exit_code) = run_binary(output) else {
        let _ = std::fs::remove_file(output);
        return; // cannot run on this host — skip
    };

    let _ = std::fs::remove_file(output);

    assert_eq!(
        exit_code, 0,
        "milestone_1.rs: expected exit code 0, got {exit_code}"
    );
}
