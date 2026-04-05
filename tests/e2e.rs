//! End-to-end tests: galvanic → ARM64 assembly → binary → run → check exit code.
//!
//! Each test:
//! 1. Writes a Rust source file to a temp directory.
//! 2. Runs `galvanic` on it, producing a `.s` assembly file.
//! 3. Assembles the `.s` with `aarch64-linux-gnu-as` (or `as` on ARM64 hosts).
//! 4. Links the `.o` with `aarch64-linux-gnu-ld` (or `ld` on ARM64 hosts).
//! 5. Runs the binary (via `qemu-aarch64` on x86_64, natively on ARM64).
//! 6. Asserts the exit code matches the expected value.
//!
//! # FLS traceability
//!
//! - FLS §9: fn main() — tested as the entry point of every program here.
//! - FLS §18.1: Program structure — main() is the program entry point.
//! - FLS §2.4.4.1: Integer literals — the return value is a literal constant.
//!
//! # Skipping
//!
//! Tests are skipped gracefully if the required tools are not present.
//! On macOS, `aarch64-linux-gnu-as` targets Linux ELF and is not available;
//! on CI (ubuntu-latest, x86_64) the tools are installed explicitly.

use std::path::Path;
use std::process::Command;

// ── Tool detection ────────────────────────────────────────────────────────────

fn is_arm64_host() -> bool {
    std::env::consts::ARCH == "aarch64"
}

fn assembler() -> &'static str {
    if is_arm64_host() { "as" } else { "aarch64-linux-gnu-as" }
}

fn linker() -> &'static str {
    if is_arm64_host() { "ld" } else { "aarch64-linux-gnu-ld" }
}

/// Returns true if all required tools (assembler, linker, QEMU) are on PATH
/// and the platform supports Linux ELF ARM64 binaries.
fn tools_available() -> bool {
    // The ARM64 Linux ELF toolchain only makes sense on Linux. macOS uses
    // Mach-O and a different system call ABI even on ARM64 hardware.
    if std::env::consts::OS != "linux" {
        return false;
    }
    let as_ok = Command::new(assembler()).arg("--version").output().is_ok();
    let ld_ok = Command::new(linker()).arg("--version").output().is_ok();
    // On an ARM64 Linux host we run natively; on x86_64 Linux we need qemu-aarch64.
    let run_ok = is_arm64_host()
        || Command::new("qemu-aarch64").arg("--version").output().is_ok();
    as_ok && ld_ok && run_ok
}

// ── Compile + run helper ──────────────────────────────────────────────────────

/// Compile `src_path` with galvanic, assemble, link, run, and return the exit
/// code. All intermediate files are placed in `work_dir`.
fn compile_and_run(src_path: &Path, work_dir: &Path) -> i32 {
    // Step 1: galvanic → {src_path}.s
    let galvanic = env!("CARGO_BIN_EXE_galvanic");
    let status = Command::new(galvanic)
        .arg(src_path)
        .status()
        .expect("failed to run galvanic");
    assert!(
        status.success(),
        "galvanic exited {} on {}",
        status,
        src_path.display()
    );

    let asm_path = src_path.with_extension("s");
    assert!(
        asm_path.exists(),
        "galvanic did not emit assembly at {}",
        asm_path.display()
    );

    // Step 2: assemble → main.o
    let obj_path = work_dir.join("main.o");
    let status = Command::new(assembler())
        .arg(&asm_path)
        .arg("-o")
        .arg(&obj_path)
        .status()
        .expect("failed to run assembler");
    assert!(status.success(), "assembler failed");

    // Step 3: link → main (bare ELF, entry = _start)
    let bin_path = work_dir.join("main");
    let status = Command::new(linker())
        .arg(&obj_path)
        .arg("-o")
        .arg(&bin_path)
        .status()
        .expect("failed to run linker");
    assert!(status.success(), "linker failed");

    // Step 4: run (natively on ARM64, via QEMU on x86_64)
    let run_status = if is_arm64_host() {
        Command::new(&bin_path)
            .status()
            .expect("failed to run binary natively")
    } else {
        Command::new("qemu-aarch64")
            .arg(&bin_path)
            .status()
            .expect("failed to run binary via qemu-aarch64")
    };

    run_status.code().expect("binary terminated by signal")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Milestone 1: `fn main() -> i32 { 0 }` exits with code 0.
///
/// FLS §9: fn main with i32 return type.
/// FLS §18.1: main is the program entry point.
/// FLS §2.4.4.1: 0 is an integer literal.
#[test]
fn milestone_1_main_returns_zero() {
    if !tools_available() {
        eprintln!(
            "milestone_1_main_returns_zero: SKIP \
             (aarch64 cross tools or qemu-aarch64 not found)"
        );
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("main.rs");

    // FLS §9 example: fn main() -> i32 returning integer literal 0.
    std::fs::write(&src, "fn main() -> i32 { 0 }\n").expect("write fixture");

    let exit_code = compile_and_run(&src, dir.path());
    assert_eq!(exit_code, 0, "expected exit 0, got {exit_code}");
}

/// Milestone 1 (variant): `fn main() -> i32 { 42 }` exits with code 42.
///
/// Verifies that the emitted binary actually propagates the literal value,
/// not just hard-codes 0.
///
/// FLS §2.4.4.1: 42 is an integer literal.
#[test]
fn milestone_1_main_returns_nonzero() {
    if !tools_available() {
        eprintln!(
            "milestone_1_main_returns_nonzero: SKIP \
             (aarch64 cross tools or qemu-aarch64 not found)"
        );
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("main.rs");

    // FLS §9 / §2.4.4.1: fn main returning integer literal 42.
    std::fs::write(&src, "fn main() -> i32 { 42 }\n").expect("write fixture");

    let exit_code = compile_and_run(&src, dir.path());
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

// ── Milestone 2: arithmetic ───────────────────────────────────────────────────

/// Milestone 2: `fn main() -> i32 { 1 + 2 }` exits with code 3.
///
/// FLS §6.5.5: Addition operator `+`.
/// FLS §2.4.4.1: Integer literals 1 and 2.
/// FLS §6.23: Arithmetic overflow — wrapping semantics documented.
#[test]
fn milestone_2_add() {
    if !tools_available() {
        eprintln!("milestone_2_add: SKIP (aarch64 cross tools or qemu-aarch64 not found)");
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("main.rs");

    // FLS §6.5.5 example: addition of two integer literals.
    std::fs::write(&src, "fn main() -> i32 { 1 + 2 }\n").expect("write fixture");

    let exit_code = compile_and_run(&src, dir.path());
    assert_eq!(exit_code, 3, "expected exit 3 (1+2), got {exit_code}");
}

/// Milestone 2 (variant): `fn main() -> i32 { 10 - 3 }` exits with code 7.
///
/// FLS §6.5.5: Subtraction operator `-`.
#[test]
fn milestone_2_sub() {
    if !tools_available() {
        eprintln!("milestone_2_sub: SKIP (aarch64 cross tools or qemu-aarch64 not found)");
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("main.rs");

    std::fs::write(&src, "fn main() -> i32 { 10 - 3 }\n").expect("write fixture");

    let exit_code = compile_and_run(&src, dir.path());
    assert_eq!(exit_code, 7, "expected exit 7 (10-3), got {exit_code}");
}

/// Milestone 2 (variant): `fn main() -> i32 { 3 * 4 }` exits with code 12.
///
/// FLS §6.5.5: Multiplication operator `*`.
#[test]
fn milestone_2_mul() {
    if !tools_available() {
        eprintln!("milestone_2_mul: SKIP (aarch64 cross tools or qemu-aarch64 not found)");
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("main.rs");

    std::fs::write(&src, "fn main() -> i32 { 3 * 4 }\n").expect("write fixture");

    let exit_code = compile_and_run(&src, dir.path());
    assert_eq!(exit_code, 12, "expected exit 12 (3*4), got {exit_code}");
}

/// Milestone 2 (compound): `fn main() -> i32 { 1 + 2 + 3 }` exits with code 6.
///
/// Verifies that nested (left-associative) binary expressions constant-fold
/// correctly through two levels of the AST.
///
/// FLS §6.21: Expression precedence — `+` is left-associative.
#[test]
fn milestone_2_nested_add() {
    if !tools_available() {
        eprintln!(
            "milestone_2_nested_add: SKIP (aarch64 cross tools or qemu-aarch64 not found)"
        );
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("main.rs");

    std::fs::write(&src, "fn main() -> i32 { 1 + 2 + 3 }\n").expect("write fixture");

    let exit_code = compile_and_run(&src, dir.path());
    assert_eq!(exit_code, 6, "expected exit 6 (1+2+3), got {exit_code}");
}

// ── Milestone 1 ───────────────────────────────────────────────────────────────

/// Milestone 1 (implicit unit): `fn main() {}` exits with code 0.
///
/// FLS §9: "If no return type is specified, the return type is `()`."
/// FLS §4.4: Unit type convention — exit code 0 for main.
#[test]
fn milestone_1_main_unit_return() {
    if !tools_available() {
        eprintln!(
            "milestone_1_main_unit_return: SKIP \
             (aarch64 cross tools or qemu-aarch64 not found)"
        );
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("main.rs");

    // FLS §9: fn main() with no return type (implicit unit return).
    std::fs::write(&src, "fn main() {}\n").expect("write fixture");

    let exit_code = compile_and_run(&src, dir.path());
    assert_eq!(exit_code, 0, "expected exit 0 for unit main, got {exit_code}");
}
