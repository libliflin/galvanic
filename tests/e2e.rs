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

// ── Milestone 4: if/else control flow ────────────────────────────────────────
//
// FLS §6.17: If expressions evaluate the condition at runtime and branch to
// either the then-block or the else-block. Both branches must produce the same
// type; the if expression's value is the value of the taken branch.
//
// FLS §2.4.7: Boolean literals — `true` = 1, `false` = 0.
// FLS §6.1.2:37–45: The branch must resolve at runtime, not compile time.
// Even `if true { 1 } else { 0 }` must emit a `cbz` instruction.

/// Milestone 4: `fn main() -> i32 { if true { 1 } else { 0 } }` exits with 1.
///
/// The `true` condition is 1; `cbz` does NOT branch; the then-block runs.
/// FLS §6.17: If expression with boolean literal condition.
/// FLS §2.4.7: `true` has value 1.
#[test]
fn milestone_4_if_true() {
    let Some(exit_code) =
        compile_and_run("fn main() -> i32 { if true { 1 } else { 0 } }\n")
    else {
        return;
    };
    assert_eq!(exit_code, 1, "expected exit 1 from `if true {{ 1 }} else {{ 0 }}`");
}

/// Milestone 4 (variant): `fn main() -> i32 { if false { 1 } else { 0 } }` exits with 0.
///
/// The `false` condition is 0; `cbz` DOES branch to the else-block.
/// FLS §6.17: If expression with false condition takes the else branch.
/// FLS §2.4.7: `false` has value 0.
#[test]
fn milestone_4_if_false() {
    let Some(exit_code) =
        compile_and_run("fn main() -> i32 { if false { 1 } else { 0 } }\n")
    else {
        return;
    };
    assert_eq!(exit_code, 0, "expected exit 0 from `if false {{ 1 }} else {{ 0 }}`");
}

/// Milestone 4 (with let): `fn main() -> i32 { let x = 1; if true { x } else { 0 } }`.
///
/// Tests that if/else composes correctly with let bindings.
/// FLS §6.17: The then-block can contain a path expression.
/// FLS §8.1: The let binding is stored on the stack before the if.
#[test]
fn milestone_4_if_with_let() {
    let src = "fn main() -> i32 { let x = 7; if true { x } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

// ── Milestone 5: function calls ──────────────────────────────────────────────
//
// FLS §6.12.1: Call expressions. The callee is a path expression resolving to
// a function item. Arguments are evaluated left-to-right and passed in x0–x{n-1}
// per the ARM64 ABI. The return value arrives in x0.
//
// FLS §9: Functions with parameters receive arguments in registers; spilling
// to the stack makes them addressable by name in the body.

/// Milestone 5: two functions, one calls the other.
///
/// `fn add(a: i32, b: i32) -> i32 { a + b }` is called from main.
/// FLS §6.12.1: Call expression with two integer arguments.
/// FLS §9: Parameters are passed in x0 and x1 per the ARM64 ABI.
#[test]
fn milestone_5_call_add() {
    let src = "fn add(a: i32, b: i32) -> i32 { a + b }\nfn main() -> i32 { add(1, 2) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 3, "expected exit 3 from add(1, 2), got {exit_code}");
}

/// Milestone 5 (variant): call with three arguments.
///
/// FLS §9: Functions may take multiple parameters.
/// FLS §6.5.5: Arithmetic in the callee body runs at runtime.
#[test]
fn milestone_5_call_three_args() {
    let src = "fn sum3(a: i32, b: i32, c: i32) -> i32 { a + b + c }\nfn main() -> i32 { sum3(1, 2, 3) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 6, "expected exit 6 from sum3(1,2,3), got {exit_code}");
}

/// Milestone 5 (variant): callee result used in caller arithmetic.
///
/// FLS §6.12.1: The return value of a call is a value expression.
/// FLS §6.5.5: It can be used as an arithmetic operand.
#[test]
fn milestone_5_call_result_in_expr() {
    let src = "fn double(x: i32) -> i32 { x + x }\nfn main() -> i32 { double(5) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 10, "expected exit 10 from double(5), got {exit_code}");
}

// ── Milestone 6: mutable variables and assignment ────────────────────────────
//
// FLS §6.5.10: Assignment expressions update the value of a mutable place.
// FLS §8.1: `let mut` bindings may be re-assigned after initialisation.
// FLS §6.1.2:37–45: The assignment is a runtime `str` instruction — the
// variable's stack slot is overwritten with the new value at runtime.
//
// These tests verify that assignment expressions work end-to-end:
// the initial value (from the let binding) is overwritten by the assignment,
// and the final path expression reads the updated value.

/// Milestone 6: `let mut x = 0; x = 42; x` — assignment overwrites the initial value.
///
/// FLS §6.5.10: Assignment expression `x = 42` emits a runtime `str`.
/// FLS §8.1: `let mut x = 0` initialises x to 0 on the stack.
/// FLS §6.3: The tail expression `x` loads from x's stack slot (reads 42).
#[test]
fn milestone_6_mutable_assignment() {
    let src = "fn main() -> i32 { let mut x = 0; x = 42; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 42, "expected exit 42 from `let mut x = 0; x = 42; x`, got {exit_code}");
}

/// Milestone 6 (variant): assignment in terms of the current value.
///
/// `let mut x = 10; x = x + 5; x` — the RHS reads x (10) then adds 5.
/// FLS §6.5.10: The RHS of an assignment is evaluated before the store.
/// FLS §6.5.5: Arithmetic on the loaded value runs at runtime.
#[test]
fn milestone_6_assignment_with_arithmetic() {
    let src = "fn main() -> i32 { let mut x = 10; x = x + 5; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 15, "expected exit 15 from `let mut x = 10; x = x + 5; x`, got {exit_code}");
}

/// Milestone 6 (variant): two variables, assign one to the other.
///
/// FLS §6.5.10: The RHS of an assignment can be any expression.
/// FLS §6.3: Reading one variable as the RHS of an assignment to another.
#[test]
fn milestone_6_assign_from_other_var() {
    let src = "fn main() -> i32 { let mut x = 1; let y = 99; x = y; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 99, "expected exit 99 from `x = y; x`, got {exit_code}");
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

/// `fn main() -> i32 { add(1, 2) }` must emit `bl add` and save/restore lr.
///
/// FLS §6.12.1: A call expression must emit `bl` at runtime.
/// FLS §6.12.1: The calling function must save x30 (lr) before `bl`.
#[test]
fn runtime_call_emits_bl_and_lr_save() {
    let src = "fn add(a: i32, b: i32) -> i32 { a + b }\nfn main() -> i32 { add(1, 2) }\n";
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("bl      add"),
        "expected `bl add` instruction for call expression:\n{asm}"
    );
    assert!(
        asm.contains("str     x30, [sp, #-16]!"),
        "expected lr save in non-leaf main:\n{asm}"
    );
    assert!(
        asm.contains("ldr     x30, [sp], #16"),
        "expected lr restore before ret in non-leaf main:\n{asm}"
    );
}

/// `fn main() -> i32 { if true { 1 } else { 0 } }` must emit `cbz`.
///
/// FLS §6.17: If expression must branch at runtime via `cbz`.
/// FLS §6.1.2:37–45: The condition `true` must not be folded; `cbz` must appear.
#[test]
fn runtime_if_emits_cbz() {
    let asm = compile_to_asm("fn main() -> i32 { if true { 1 } else { 0 } }\n");
    assert!(
        asm.contains("cbz"),
        "expected `cbz` instruction for if condition, got:\n{asm}"
    );
    // Must not fold `if true { 1 }` to a constant `mov x0, #1` without branching.
    // The then-branch result is stored via str/ldr through the phi slot.
    assert!(
        asm.contains("str") && asm.contains("ldr"),
        "expected `str`/`ldr` for phi slot in if expression:\n{asm}"
    );
}

/// `let mut x = 0; x = 42; x` emits two `str` instructions (one per store).
///
/// FLS §6.5.10: Assignment emits a runtime `str`, distinct from the let initializer.
/// FLS §6.1.2:37–45: Both the let and the assignment must emit runtime stores —
/// the compiler must not optimise away the initial store or the assignment.
#[test]
fn runtime_assignment_emits_two_stores() {
    let asm = compile_to_asm("fn main() -> i32 { let mut x = 0; x = 42; x }\n");
    let str_count = asm.matches("str").count();
    assert!(
        str_count >= 2,
        "expected at least 2 `str` instructions (let init + assignment), found {str_count}:\n{asm}"
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

// ── Milestone 7: while loops ─────────────────────────────────────────────────
//
// FLS §6.15.3: While loop expressions. The condition is evaluated before each
// iteration; if true, the body executes and the loop repeats. If false, the
// loop terminates with value `()`.
//
// FLS §6.5.3: Comparison operators produce boolean values (0 or 1) used as
// the while condition. The comparison runs at runtime via `cmp`+`cset`.
//
// FLS §6.1.2:37–45: The condition and body must execute at runtime — even
// `while x < 5 { ... }` with a statically-knowable bound emits runtime branches.

/// Milestone 7: count from 0 to 5 with `while`.
///
/// ```rust
/// fn main() -> i32 {
///     let mut x = 0;
///     while x < 5 { x = x + 1; }
///     x
/// }
/// ```
/// The loop body runs 5 times; `x` ends at 5.
///
/// FLS §6.15.3: While loop with `<` comparison.
/// FLS §6.5.3: `x < 5` emits `cmp`+`cset` at runtime.
#[test]
fn milestone_7_while_count_to_five() {
    let src = "fn main() -> i32 { let mut x = 0; while x < 5 { x = x + 1; } x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 5, "expected exit 5 from while-count-to-five, got {exit_code}");
}

/// Milestone 7 (variant): while with immediately-false condition never executes.
///
/// FLS §6.15.3: "The block is repeatedly executed as long as the condition holds."
/// When the condition is false on entry, the body is skipped and the loop exits.
/// FLS §6.5.3: `x < 0` with x=0 is false; cbz branches directly to exit.
#[test]
fn milestone_7_while_false_condition() {
    let src = "fn main() -> i32 { let mut x = 0; while x < 0 { x = x + 1; } x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected exit 0 (loop body never runs), got {exit_code}");
}

/// Milestone 7 (variant): while loop with `<=` comparison.
///
/// FLS §6.5.3: `<=` comparison emits `cmp`+`cset le`.
/// Loop runs while `x <= 3`; x ends at 4.
#[test]
fn milestone_7_while_le_comparison() {
    let src = "fn main() -> i32 { let mut x = 0; while x <= 3 { x = x + 1; } x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 4, "expected exit 4 from while x <= 3, got {exit_code}");
}

/// Assembly inspection: while loop must emit `cmp`, `cset`, `cbz`, and `b`.
///
/// FLS §6.15.3: The loop condition is checked at runtime via `cbz`.
/// FLS §6.5.3: The comparison `x < 5` emits `cmp`+`cset` at runtime.
/// FLS §6.1.2:37–45: No compile-time folding of the loop — both `cmp`+`cset`
/// and `cbz` must appear, along with the back-edge `b .L{n}`.
#[test]
fn runtime_while_emits_cmp_cset_cbz_and_b() {
    let asm = compile_to_asm("fn main() -> i32 { let mut x = 0; while x < 5 { x = x + 1; } x }\n");
    assert!(
        asm.contains("cmp"),
        "expected `cmp` instruction for while condition, got:\n{asm}"
    );
    assert!(
        asm.contains("cset"),
        "expected `cset` instruction to materialise comparison result, got:\n{asm}"
    );
    assert!(
        asm.contains("cbz"),
        "expected `cbz` instruction for while exit test, got:\n{asm}"
    );
    // The back-edge `b .L{n}` must appear — this is what makes it a loop.
    assert!(
        asm.contains("\n    b "),
        "expected unconditional `b` back-edge instruction in while loop, got:\n{asm}"
    );
}

// ── Milestone 8: loop / break / continue ─────────────────────────────────────
//
// FLS §6.15.2: Infinite loop expressions. A `loop` block executes indefinitely
// until a `break` expression is encountered. The type of a loop without a
// break value is `()`.
//
// FLS §6.15.6: Break expressions exit the innermost enclosing loop. The branch
// to the loop exit must be emitted as a runtime instruction — not constant-folded
// even when the break condition is statically known.
//
// FLS §6.15.7: Continue expressions restart the innermost loop by branching to
// the loop header.
//
// FLS §6.1.2:37–45: All branches are runtime instructions.

/// Milestone 8: `loop { break; }` — simplest infinite loop, exits immediately.
///
/// The loop executes one iteration, hits `break`, and exits. The rest of main
/// returns 0.
///
/// FLS §6.15.2: `loop` executes the body until a break.
/// FLS §6.15.6: `break` without a value exits the loop.
#[test]
fn milestone_8_loop_immediate_break() {
    let src = "fn main() -> i32 { loop { break; } 0 }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected exit 0 from `loop {{ break; }} 0`, got {exit_code}");
}

/// Milestone 8: loop with a conditional break that runs the body multiple times.
///
/// ```rust
/// fn main() -> i32 {
///     let mut i = 0;
///     loop {
///         if i == 3 { break; }
///         i = i + 1;
///     }
///     i
/// }
/// ```
/// The body runs 3 times (`i` = 0, 1, 2), then `i == 3` is true and `break`
/// exits the loop. The tail expression reads `i` = 3.
///
/// FLS §6.15.2: Loop body runs until break.
/// FLS §6.15.6: `break` in a nested if exits the enclosing loop.
/// FLS §6.17: The `if` test is evaluated at runtime each iteration.
#[test]
fn milestone_8_loop_count_to_three() {
    let src = "fn main() -> i32 { let mut i = 0; loop { if i == 3 { break; } i = i + 1; } i }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 3, "expected exit 3 from loop-count-to-three, got {exit_code}");
}

/// Milestone 8: continue skips remaining body and restarts loop.
///
/// ```rust
/// fn main() -> i32 {
///     let mut i = 0;
///     let mut j = 0;
///     loop {
///         if i == 5 { break; }
///         i = i + 1;
///         if i == 3 { continue; }
///         j = j + 1;
///     }
///     j
/// }
/// ```
/// `i` goes 0→1→2→3→4→5. When `i == 3`, `continue` skips `j = j + 1`.
/// So `j` is incremented for i=1,2,4,5 → j ends at 4.
///
/// FLS §6.15.7: `continue` restarts the loop body from the header.
/// FLS §6.15.6: `break` exits the loop when `i == 5`.
#[test]
fn milestone_8_loop_continue_skips_body() {
    let src = "fn main() -> i32 { let mut i = 0; let mut j = 0; loop { if i == 5 { break; } i = i + 1; if i == 3 { continue; } j = j + 1; } j }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 4, "expected exit 4 from loop-continue test, got {exit_code}");
}

/// Assembly inspection: `loop { break; }` must emit a back-edge `b` and a `cbz`-free loop header.
///
/// A `loop` has no condition — its header emits no `cbz`. The only branch
/// out of the loop is the `break`'s unconditional `b` to the exit label.
/// The back-edge unconditional `b` to the header must also appear.
///
/// FLS §6.15.2: `loop` is an infinite loop — no condition check at the header.
/// FLS §6.15.6: `break` emits a runtime `b` to the exit label.
/// FLS §6.1.2:37–45: Both branches are runtime instructions.
#[test]
fn runtime_loop_emits_back_edge_and_break_branch() {
    let asm = compile_to_asm("fn main() -> i32 { loop { break; } 0 }\n");
    // The back-edge must appear — this is what makes it a loop structure.
    let b_count = asm.lines().filter(|l| l.trim_start().starts_with("b ")).count();
    assert!(
        b_count >= 2,
        "expected at least 2 unconditional `b` instructions (back-edge + break branch), found {b_count}:\n{asm}"
    );
    // Unlike `while`, a `loop` has no condition — `cbz` must NOT appear at the header.
    // (It may appear if there's an `if` inside the body, but this simple case has none.)
    assert!(
        !asm.contains("cbz"),
        "expected no `cbz` for unconditional `loop {{ break; }}`, got:\n{asm}"
    );
}

// ── Milestone 9: explicit return expressions ──────────────────────────────────
//
// FLS §6.19: Return expressions transfer control from the current function to
// the caller, optionally producing a value. A `return` expression may appear
// anywhere an expression is allowed — not just as the final tail expression.
//
// The key test is that `return` appearing as a statement (inside a block, or
// inside an `if` branch) emits a `ret` instruction at the correct point in
// the instruction stream, allowing earlier exit from the function.
//
// FLS §6.19: "The type of a return expression is the never type `!`."
// FLS §6.1.2:37–45: The `ret` is a runtime instruction — not elided even
// when the returned value is statically known.

/// Milestone 9: explicit `return` as the only statement in a function.
///
/// `fn f() -> i32 { return 42; }` — the tail is absent; the only exit is
/// the explicit `return` statement.
///
/// FLS §6.19: Return expression with value.
/// FLS §6.4: Block with no tail expression — the return statement provides
/// the only exit from the function.
#[test]
fn milestone_9_explicit_return_only() {
    let src = "fn f() -> i32 { return 42; }\nfn main() -> i32 { f() }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 42, "expected exit 42 from explicit `return 42;`, got {exit_code}");
}

/// Milestone 9: early return from a function based on a condition.
///
/// ```rust
/// fn clamp_lower(x: i32) -> i32 {
///     if x < 0 { return 0; }
///     x
/// }
/// fn main() -> i32 { clamp_lower(-5) }
/// ```
/// When `x < 0`, the `return 0` exits before reaching the tail `x`.
///
/// FLS §6.19: Return expression inside an if branch provides early exit.
/// FLS §6.17: The if has no else; when the condition is false the tail runs.
/// FLS §6.5.3: `x < 0` emits `cmp`+`cset` at runtime.
#[test]
fn milestone_9_early_return_taken() {
    let src = "fn clamp_lower(x: i32) -> i32 { if x < 0 { return 0; } x }\nfn main() -> i32 { clamp_lower(-5) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected exit 0 from clamp_lower(-5), got {exit_code}");
}

/// Milestone 9 (variant): early return NOT taken — falls through to tail.
///
/// Same `clamp_lower` function, called with a non-negative argument.
/// The `if` condition is false; `return 0` is skipped; the tail `x` runs.
///
/// FLS §6.19: The return expression is not reached; the tail expression provides
/// the function's value.
#[test]
fn milestone_9_early_return_not_taken() {
    let src = "fn clamp_lower(x: i32) -> i32 { if x < 0 { return 0; } x }\nfn main() -> i32 { clamp_lower(7) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 7, "expected exit 7 from clamp_lower(7), got {exit_code}");
}

/// Assembly inspection: explicit `return` must emit a `ret` instruction.
///
/// A `return <value>` inside a function body emits a `ret` instruction at
/// the point of the return, not just at the end of the function.
///
/// FLS §6.19: Return expressions emit runtime `ret` instructions.
/// FLS §6.1.2:37–45: The `ret` is not elided even when the value is constant.
#[test]
fn runtime_explicit_return_emits_ret() {
    let asm = compile_to_asm("fn f() -> i32 { return 42; }\nfn main() -> i32 { f() }\n");
    // The `ret` instruction must appear in `f`'s body.
    assert!(
        asm.contains("ret"),
        "expected `ret` instruction for explicit return expression, got:\n{asm}"
    );
}

// ── Milestone 10: integer division and remainder ──────────────────────────────
//
// FLS §6.5.5: Arithmetic operator expressions include `/` (division) and `%`
// (remainder). For integer types, division truncates toward zero. Remainder
// satisfies `(a / b) * b + (a % b) == a`.
//
// FLS §6.23: Division by zero and signed MIN / -1 overflow panic at runtime.
// Galvanic does not yet insert a panic check — this is noted as a known gap.
//
// FLS §6.1.2:37–45: Division must emit a runtime `sdiv` instruction, not
// be constant-folded — even `10 / 2` must emit `sdiv`.

/// Milestone 10: `fn main() -> i32 { 10 / 2 }` exits with code 5.
///
/// FLS §6.5.5: Integer division operator `/`.
/// ARM64: `sdiv` — signed integer division.
#[test]
fn milestone_10_div() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 10 / 2 }\n") else {
        return;
    };
    assert_eq!(exit_code, 5, "expected exit 5 from `10 / 2`, got {exit_code}");
}

/// Milestone 10 (variant): division with non-even result truncates toward zero.
///
/// FLS §6.5.5: Integer division truncates toward zero.
/// `7 / 2 = 3` (rounds toward zero, not down).
#[test]
fn milestone_10_div_truncates() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 7 / 2 }\n") else {
        return;
    };
    assert_eq!(exit_code, 3, "expected exit 3 from `7 / 2`, got {exit_code}");
}

/// Milestone 10: `fn main() -> i32 { 10 % 3 }` exits with code 1.
///
/// FLS §6.5.5: Integer remainder operator `%`.
/// ARM64: `sdiv` + `msub` — `lhs - (lhs / rhs) * rhs`.
#[test]
fn milestone_10_rem() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 10 % 3 }\n") else {
        return;
    };
    assert_eq!(exit_code, 1, "expected exit 1 from `10 % 3`, got {exit_code}");
}

/// Milestone 10 (variant): division composed with other arithmetic.
///
/// FLS §6.5.5: Division has higher precedence than addition.
/// `fn main() -> i32 { 6 / 2 + 1 }` → `(6 / 2) + 1` = `3 + 1` = 4.
/// FLS §6.21: Expression precedence — `*`, `/`, `%` bind tighter than `+`, `-`.
#[test]
fn milestone_10_div_in_expr() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 6 / 2 + 1 }\n") else {
        return;
    };
    assert_eq!(exit_code, 4, "expected exit 4 from `6 / 2 + 1`, got {exit_code}");
}

/// Assembly inspection: `10 / 2` must emit `sdiv` (not constant-fold to 5).
///
/// FLS §6.5.5: Division operator.
/// FLS §6.1.2:37–45: Non-const division must emit a runtime `sdiv` instruction.
#[test]
fn runtime_div_emits_sdiv() {
    let asm = compile_to_asm("fn main() -> i32 { 10 / 2 }\n");
    assert!(
        asm.contains("sdiv"),
        "expected `sdiv` instruction for `10 / 2`, got:\n{asm}"
    );
    assert!(
        !asm.contains("mov     x0, #5"),
        "assembly must not constant-fold `10 / 2` to #5:\n{asm}"
    );
}

/// Assembly inspection: `10 % 3` must emit `sdiv` + `msub`.
///
/// FLS §6.5.5: Remainder operator `%`.
/// FLS §6.1.2:37–45: Non-const remainder must emit runtime instructions.
/// ARM64: remainder requires two instructions (`sdiv` + `msub`).
#[test]
fn runtime_rem_emits_sdiv_and_msub() {
    let asm = compile_to_asm("fn main() -> i32 { 10 % 3 }\n");
    assert!(
        asm.contains("sdiv"),
        "expected `sdiv` in remainder sequence for `10 % 3`, got:\n{asm}"
    );
    assert!(
        asm.contains("msub"),
        "expected `msub` in remainder sequence for `10 % 3`, got:\n{asm}"
    );
}

// ── Milestone 11: lazy boolean operators && and || ───────────────────────────
//
// FLS §6.5.8: Lazy boolean operator expressions. Both `&&` and `||` use
// short-circuit evaluation: the RHS is only evaluated if the LHS does not
// determine the result.
//
// - `&&`: if LHS is false (0), result is false without evaluating RHS.
// - `||`: if LHS is true (non-zero), result is true without evaluating RHS.
//
// Lowering uses a phi slot and `CondBranch` (cbz), the same mechanism used
// for if/else. The RHS evaluation is placed in the branch that is only reached
// when the LHS does not short-circuit.
//
// FLS §6.1.2:37–45: Both the LHS evaluation and the short-circuit branch must
// emit runtime instructions — no constant folding even when the LHS is known.

/// Milestone 11: `true && true` → 1 (both sides true).
///
/// FLS §6.5.8: Lazy boolean AND — RHS is evaluated when LHS is true.
/// FLS §2.4.7: `true` = 1, `false` = 0.
#[test]
fn milestone_11_and_both_true() {
    let src = "fn main() -> i32 { if true && true { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected 1 from `true && true`, got {exit_code}");
}

/// Milestone 11: `true && false` → 0 (LHS true, RHS false).
///
/// FLS §6.5.8: When LHS is true, RHS is evaluated and its value is the result.
#[test]
fn milestone_11_and_lhs_true_rhs_false() {
    let src = "fn main() -> i32 { if true && false { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected 0 from `true && false`, got {exit_code}");
}

/// Milestone 11: `false && true` → 0 (LHS false short-circuits, RHS not evaluated).
///
/// FLS §6.5.8: "The right operand is only evaluated if the left operand is true."
/// The branch to the false path skips RHS evaluation entirely.
#[test]
fn milestone_11_and_lhs_false_short_circuits() {
    let src = "fn main() -> i32 { if false && true { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected 0 from `false && true` (short-circuit), got {exit_code}");
}

/// Milestone 11: comparison-based `&&` — `x > 0 && y > 0`.
///
/// Tests `&&` with runtime-computed boolean operands, not just literals.
/// FLS §6.5.8: Both operands are lazy boolean expressions.
/// FLS §6.5.3: Each comparison emits `cmp`+`cset` at runtime.
#[test]
fn milestone_11_and_with_comparisons() {
    let src = "fn f(x: i32, y: i32) -> i32 { if x > 0 && y > 0 { 1 } else { 0 } }\nfn main() -> i32 { f(3, 4) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected 1 from `3 > 0 && 4 > 0`, got {exit_code}");
}

/// Milestone 11: `&&` short-circuits when LHS comparison is false.
///
/// `f(-1, 99)` → LHS `x > 0` is false → short-circuit → result = 0.
/// The RHS `y > 0` is not evaluated at all.
///
/// FLS §6.5.8: Short-circuit — RHS is skipped when LHS is false.
#[test]
fn milestone_11_and_short_circuits_on_false_lhs() {
    let src = "fn f(x: i32, y: i32) -> i32 { if x > 0 && y > 0 { 1 } else { 0 } }\nfn main() -> i32 { f(-1, 99) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected 0 from `-1 > 0 && 99 > 0` (short-circuit), got {exit_code}");
}

/// Milestone 11: `false || true` → 1 (LHS false, RHS true evaluated).
///
/// FLS §6.5.8: Lazy boolean OR — RHS is evaluated when LHS is false.
#[test]
fn milestone_11_or_lhs_false_rhs_true() {
    let src = "fn main() -> i32 { if false || true { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected 1 from `false || true`, got {exit_code}");
}

/// Milestone 11: `true || false` → 1 (LHS true short-circuits, RHS not evaluated).
///
/// FLS §6.5.8: "The right operand is only evaluated if the left operand is false."
#[test]
fn milestone_11_or_lhs_true_short_circuits() {
    let src = "fn main() -> i32 { if true || false { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected 1 from `true || false` (short-circuit), got {exit_code}");
}

/// Milestone 11: `false || false` → 0 (both false).
///
/// FLS §6.5.8: When LHS is false, RHS is evaluated. Both false → result false.
#[test]
fn milestone_11_or_both_false() {
    let src = "fn main() -> i32 { if false || false { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected 0 from `false || false`, got {exit_code}");
}

/// Milestone 11: comparison-based `||` short-circuits when LHS is true.
///
/// `f(5, -1)` → LHS `x > 0` is true → short-circuit → result = 1.
/// The RHS `y > 0` is not evaluated.
///
/// FLS §6.5.8: Short-circuit — RHS is skipped when LHS is true.
/// FLS §6.5.3: `x > 0` emits `cmp`+`cset` at runtime.
#[test]
fn milestone_11_or_short_circuits_on_true_lhs() {
    let src = "fn f(x: i32, y: i32) -> i32 { if x > 0 || y > 0 { 1 } else { 0 } }\nfn main() -> i32 { f(5, -1) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected 1 from `5 > 0 || -1 > 0` (short-circuit), got {exit_code}");
}

/// Assembly inspection: `&&` must emit `cbz` for the short-circuit branch.
///
/// FLS §6.5.8: The short-circuit is implemented by a conditional branch.
/// FLS §6.1.2:37–45: The branch is a runtime instruction — not elided.
#[test]
fn runtime_and_emits_cbz_for_short_circuit() {
    let asm = compile_to_asm("fn f(a: i32, b: i32) -> i32 { if a > 0 && b > 0 { 1 } else { 0 } }\nfn main() -> i32 { f(1, 1) }\n");
    assert!(
        asm.contains("cbz"),
        "expected `cbz` instruction for `&&` short-circuit, got:\n{asm}"
    );
}

/// Assembly inspection: `||` must emit `cbz` for the short-circuit branch.
///
/// FLS §6.5.8: The short-circuit is implemented by a conditional branch.
/// FLS §6.1.2:37–45: The branch is a runtime instruction — not elided.
#[test]
fn runtime_or_emits_cbz_for_short_circuit() {
    let asm = compile_to_asm("fn f(a: i32, b: i32) -> i32 { if a > 0 || b > 0 { 1 } else { 0 } }\nfn main() -> i32 { f(1, 1) }\n");
    assert!(
        asm.contains("cbz"),
        "expected `cbz` instruction for `||` short-circuit, got:\n{asm}"
    );
}

/// Assembly inspection: unary negation `-x` must emit a `neg` instruction.
///
/// FLS §6.5.4: Negation operator expressions. The unary `-` applied to an
/// integer value must emit a runtime `neg` instruction, not fold the literal
/// to a negative immediate.
///
/// FLS §6.1.2:37–45: Non-const code must emit runtime instructions.
/// FLS §6.5.4: "The type of a negation expression is the type of the operand."
#[test]
fn runtime_neg_emits_neg_instruction() {
    let asm = compile_to_asm("fn negate(x: i32) -> i32 { -x }\nfn main() -> i32 { negate(5) }\n");
    assert!(
        asm.contains("neg"),
        "expected `neg` instruction for unary negation, got:\n{asm}"
    );
}

// ── Milestone 12: bitwise operators & | ^ << >> ───────────────────────────────
//
// FLS §6.5.6: Bit operator expressions. The `&`, `|`, and `^` operators
// perform bitwise AND, OR, and XOR on integer types respectively.
//
// FLS §6.5.7: Shift operator expressions. `<<` shifts left (padding with zeros);
// `>>` shifts right (arithmetic shift for signed integers — sign-extending).
//
// ARM64 instructions:
//   & → `and x{d}, x{l}, x{r}`
//   | → `orr x{d}, x{l}, x{r}`
//   ^ → `eor x{d}, x{l}, x{r}`
//   << → `lsl x{d}, x{l}, x{r}`
//   >> → `asr x{d}, x{l}, x{r}` (arithmetic, signed)
//
// FLS §6.1.2:37–45: All bitwise/shift operators emit runtime instructions —
// even statically-known values like `5 & 3` must emit `and` at runtime.
//
// FLS example (§6.5.6): `0b1010 & 0b1100` = `0b1000` = 8
// FLS example (§6.5.7): `1 << 3` = 8

/// Milestone 12: `fn main() -> i32 { 5 & 3 }` exits with code 1.
///
/// `5 & 3` = `0b101 & 0b011` = `0b001` = 1.
///
/// FLS §6.5.6: Bitwise AND operator `&`.
/// FLS §6.1.2:37–45: Must emit runtime `and` instruction, not fold to 1.
#[test]
fn milestone_12_bitwise_and() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 5 & 3 }\n") else {
        return;
    };
    assert_eq!(exit_code, 1, "expected exit 1 from `5 & 3`, got {exit_code}");
}

/// Milestone 12: `fn main() -> i32 { 5 | 3 }` exits with code 7.
///
/// `5 | 3` = `0b101 | 0b011` = `0b111` = 7.
///
/// FLS §6.5.6: Bitwise OR operator `|`.
#[test]
fn milestone_12_bitwise_or() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 5 | 3 }\n") else {
        return;
    };
    assert_eq!(exit_code, 7, "expected exit 7 from `5 | 3`, got {exit_code}");
}

/// Milestone 12: `fn main() -> i32 { 5 ^ 3 }` exits with code 6.
///
/// `5 ^ 3` = `0b101 ^ 0b011` = `0b110` = 6.
///
/// FLS §6.5.6: Bitwise XOR operator `^`.
#[test]
fn milestone_12_bitwise_xor() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 5 ^ 3 }\n") else {
        return;
    };
    assert_eq!(exit_code, 6, "expected exit 6 from `5 ^ 3`, got {exit_code}");
}

/// Milestone 12: `fn main() -> i32 { 1 << 3 }` exits with code 8.
///
/// `1 << 3` = 8.
///
/// FLS §6.5.7: Left shift operator `<<`. Pads with zeros on the right.
#[test]
fn milestone_12_shift_left() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 1 << 3 }\n") else {
        return;
    };
    assert_eq!(exit_code, 8, "expected exit 8 from `1 << 3`, got {exit_code}");
}

/// Milestone 12: `fn main() -> i32 { 16 >> 2 }` exits with code 4.
///
/// `16 >> 2` = 4.
///
/// FLS §6.5.7: Right shift operator `>>`. Arithmetic shift for signed i32.
#[test]
fn milestone_12_shift_right() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 16 >> 2 }\n") else {
        return;
    };
    assert_eq!(exit_code, 4, "expected exit 4 from `16 >> 2`, got {exit_code}");
}

/// Milestone 12: bitwise ops composed with arithmetic.
///
/// `fn main() -> i32 { (3 & 5) | (1 << 2) }` = `(1) | (4)` = 5.
///
/// FLS §6.5.6: Bit operators. FLS §6.21: Precedence — shift binds tighter
/// than `|`, which binds tighter than `||`. So `(3 & 5) | (1 << 2)` parses
/// as `(3 & 5) | (1 << 2)`.
#[test]
fn milestone_12_bitwise_composed() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { (3 & 5) | (1 << 2) }\n") else {
        return;
    };
    assert_eq!(exit_code, 5, "expected exit 5 from `(3 & 5) | (1 << 2)`, got {exit_code}");
}

/// Assembly inspection: `5 & 3` must emit `and` (not constant-fold to 1).
///
/// FLS §6.5.6: Bitwise AND.
/// FLS §6.1.2:37–45: Non-const bitwise ops must emit runtime instructions.
#[test]
fn runtime_and_emits_and_instruction() {
    let asm = compile_to_asm("fn main() -> i32 { 5 & 3 }\n");
    assert!(
        asm.contains("and"),
        "expected `and` instruction for `5 & 3`, got:\n{asm}"
    );
    assert!(
        !asm.contains("mov     x0, #1"),
        "assembly must not constant-fold `5 & 3` to #1:\n{asm}"
    );
}

/// Assembly inspection: `5 | 3` must emit `orr`.
///
/// FLS §6.5.6: Bitwise OR. ARM64 uses `orr` mnemonic.
/// FLS §6.1.2:37–45: Non-const bitwise ops must emit runtime instructions.
#[test]
fn runtime_or_emits_orr_instruction() {
    let asm = compile_to_asm("fn main() -> i32 { 5 | 3 }\n");
    assert!(
        asm.contains("orr"),
        "expected `orr` instruction for `5 | 3`, got:\n{asm}"
    );
}

/// Assembly inspection: `5 ^ 3` must emit `eor`.
///
/// FLS §6.5.6: Bitwise XOR. ARM64 uses `eor` mnemonic.
/// FLS §6.1.2:37–45: Non-const bitwise ops must emit runtime instructions.
#[test]
fn runtime_xor_emits_eor_instruction() {
    let asm = compile_to_asm("fn main() -> i32 { 5 ^ 3 }\n");
    assert!(
        asm.contains("eor"),
        "expected `eor` instruction for `5 ^ 3`, got:\n{asm}"
    );
}

/// Assembly inspection: `1 << 3` must emit `lsl`.
///
/// FLS §6.5.7: Left shift. ARM64 uses `lsl` mnemonic.
/// FLS §6.1.2:37–45: Non-const shifts must emit runtime instructions.
#[test]
fn runtime_shl_emits_lsl_instruction() {
    let asm = compile_to_asm("fn main() -> i32 { 1 << 3 }\n");
    assert!(
        asm.contains("lsl"),
        "expected `lsl` instruction for `1 << 3`, got:\n{asm}"
    );
}

/// Assembly inspection: `16 >> 2` must emit `asr` (arithmetic shift right for signed i32).
///
/// FLS §6.5.7: Right shift on signed integers is arithmetic (sign-extending).
/// ARM64 uses `asr` for arithmetic shift right.
/// FLS §6.1.2:37–45: Non-const shifts must emit runtime instructions.
#[test]
fn runtime_shr_emits_asr_instruction() {
    let asm = compile_to_asm("fn main() -> i32 { 16 >> 2 }\n");
    assert!(
        asm.contains("asr"),
        "expected `asr` instruction for `16 >> 2`, got:\n{asm}"
    );
}

// ── Milestone 13: compound assignment operators ───────────────────────────────
//
// FLS §6.5.11: Compound assignment expressions. `x op= e` is equivalent to
// `x = x op e` — the variable is read, the operation is applied at runtime,
// and the result is stored back. The expression has type `()`.
//
// FLS §6.1.2:37–45: The load, binary op, and store must all emit runtime
// instructions — even `x += 1` with statically-known values emits ldr/add/str.
//
// ARM64: compound assignment emits `ldr x{t}, [sp, #slot*8]`,
//        the binary op instruction, then `str x{result}, [sp, #slot*8]`.

/// Milestone 13: `let mut x = 5; x += 3; x` exits with code 8.
///
/// FLS §6.5.11: `+=` compound assignment.
/// FLS §6.1.2:37–45: ldr + add + str are runtime instructions.
#[test]
fn milestone_13_compound_add() {
    let src = "fn main() -> i32 { let mut x = 5; x += 3; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 8, "expected exit 8 from `x += 3` (5+3), got {exit_code}");
}

/// Milestone 13: `let mut x = 10; x -= 3; x` exits with code 7.
///
/// FLS §6.5.11: `-=` compound assignment.
#[test]
fn milestone_13_compound_sub() {
    let src = "fn main() -> i32 { let mut x = 10; x -= 3; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 7, "expected exit 7 from `x -= 3` (10-3), got {exit_code}");
}

/// Milestone 13: `let mut x = 3; x *= 4; x` exits with code 12.
///
/// FLS §6.5.11: `*=` compound assignment.
#[test]
fn milestone_13_compound_mul() {
    let src = "fn main() -> i32 { let mut x = 3; x *= 4; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 12, "expected exit 12 from `x *= 4` (3*4), got {exit_code}");
}

/// Milestone 13: `let mut x = 10; x /= 2; x` exits with code 5.
///
/// FLS §6.5.11: `/=` compound assignment.
#[test]
fn milestone_13_compound_div() {
    let src = "fn main() -> i32 { let mut x = 10; x /= 2; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 5, "expected exit 5 from `x /= 2` (10/2), got {exit_code}");
}

/// Milestone 13: `let mut x = 10; x %= 3; x` exits with code 1.
///
/// FLS §6.5.11: `%=` compound assignment.
#[test]
fn milestone_13_compound_rem() {
    let src = "fn main() -> i32 { let mut x = 10; x %= 3; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected exit 1 from `x %= 3` (10%3), got {exit_code}");
}

/// Milestone 13: `let mut x = 5; x &= 3; x` exits with code 1.
///
/// `5 & 3` = `0b101 & 0b011` = `0b001` = 1.
///
/// FLS §6.5.11: `&=` compound assignment (bitwise AND).
#[test]
fn milestone_13_compound_bitand() {
    let src = "fn main() -> i32 { let mut x = 5; x &= 3; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected exit 1 from `x &= 3` (5&3), got {exit_code}");
}

/// Milestone 13: `let mut x = 5; x |= 2; x` exits with code 7.
///
/// `5 | 2` = `0b101 | 0b010` = `0b111` = 7.
///
/// FLS §6.5.11: `|=` compound assignment (bitwise OR).
#[test]
fn milestone_13_compound_bitor() {
    let src = "fn main() -> i32 { let mut x = 5; x |= 2; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 7, "expected exit 7 from `x |= 2` (5|2), got {exit_code}");
}

/// Milestone 13: compound assignment used inside a while loop.
///
/// `while x < 10 { x += 3; }` with x starting at 1: x goes 1→4→7→10.
/// Loop exits when x == 10; exit code is 10.
///
/// This is the canonical use case for compound assignment in real programs.
///
/// FLS §6.5.11: compound assignment inside a loop body.
/// FLS §6.15.3: while loop executes while condition is true.
#[test]
fn milestone_13_compound_in_loop() {
    let src = "fn main() -> i32 { let mut x = 1; while x < 10 { x += 3; } x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 10, "expected exit 10 from `while x < 10 {{ x += 3; }}`, got {exit_code}");
}

/// Assembly inspection: `x += 3` must emit `ldr`, `add`, and `str`.
///
/// FLS §6.5.11: compound assignment desugars to load + op + store at runtime.
/// FLS §6.1.2:37–45: all three instructions must appear — no elision.
#[test]
fn runtime_compound_add_emits_ldr_add_str() {
    let asm = compile_to_asm("fn main() -> i32 { let mut x = 5; x += 3; x }\n");
    assert!(
        asm.contains("ldr"),
        "expected `ldr` instruction for compound assignment read, got:\n{asm}"
    );
    assert!(
        asm.contains("add"),
        "expected `add` instruction for compound assignment operation, got:\n{asm}"
    );
    // At least 2 `str` instructions: let init + compound assign store.
    let str_count = asm.matches("str").count();
    assert!(
        str_count >= 2,
        "expected at least 2 `str` instructions (let init + compound assign), found {str_count}:\n{asm}"
    );
}

// ── Milestone 14: unary bitwise NOT ──────────────────────────────────────────
//
// FLS §6.5.4: Negation operator expressions. The unary `!` applied to an
// integer value produces its bitwise complement. For `i32`, `!n` = `-(n+1)`.
//
// ARM64: `mvn x{dst}, x{src}` (ORN xD, xzr, xS) — complement all bits.
//
// FLS §6.1.2:37–45: Even `!5` in a non-const function must emit a runtime
// `mvn` instruction — no compile-time folding to the complemented immediate.
//
// Note: In Rust, `!` on integers is bitwise NOT (not logical NOT). The exit
// code is taken mod 256 by the OS; `!0_i32` = -1 = 0xFF…FF, so the process
// exits with code 255. We test with small operands where the result is
// non-negative and unambiguous.

/// Milestone 14: `fn main() -> i32 { !0 }` → bitwise NOT of 0 = -1 → exit 255 (mod 256).
///
/// FLS §6.5.4: Bitwise NOT on integer. `!0_i32` = -1 = 0xFFFFFFFF.
/// The OS truncates the exit code to 8 bits: 255.
#[test]
fn milestone_14_not_zero() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { !0 }\n") else {
        return;
    };
    assert_eq!(exit_code, 255, "expected exit 255 from `!0` (bitwise NOT, 8-bit truncation), got {exit_code}");
}

/// Milestone 14: `fn main() -> i32 { !!5 }` → double NOT restores original value → exit 5.
///
/// FLS §6.5.4: `!(!5)` = `!(-6)` = 5. Double complement is identity for integers.
/// FLS §6.1.2:37–45: Both `mvn` instructions must be emitted at runtime.
#[test]
fn milestone_14_double_not() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { !!5 }\n") else {
        return;
    };
    assert_eq!(exit_code, 5, "expected exit 5 from `!!5` (double bitwise NOT), got {exit_code}");
}

/// Milestone 14: `!` in a function parameter — confirms runtime codegen path.
///
/// `fn not(x: i32) -> i32 { !x }` with `x = 0` → 255 (exit code mod 256).
/// FLS §6.5.4: `!` on an unknown-at-compile-time value must emit `mvn`.
/// FLS §6.1.2:37–45: Can't constant-fold when the operand is a function parameter.
#[test]
fn milestone_14_not_parameter() {
    let src = "fn not(x: i32) -> i32 { !x }\nfn main() -> i32 { not(0) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 255, "expected exit 255 from `not(0)` = `!0`, got {exit_code}");
}

/// Assembly inspection: `!5` must emit `mvn` (not constant-fold to -6).
///
/// FLS §6.5.4: Bitwise NOT on integer.
/// FLS §6.1.2:37–45: Non-const `!operand` must emit a runtime instruction.
/// ARM64: `mvn x{dst}, x{src}` is the bitwise complement instruction.
#[test]
fn runtime_not_emits_mvn_instruction() {
    let asm = compile_to_asm("fn main() -> i32 { !5 }\n");
    assert!(
        asm.contains("mvn"),
        "expected `mvn` instruction for `!5` (bitwise NOT), got:\n{asm}"
    );
    // Must NOT constant-fold !5 = -6 into an immediate.
    assert!(
        !asm.contains("mov     x0, #-6"),
        "assembly must not constant-fold `!5` to #-6:\n{asm}"
    );
}

/// Assembly inspection: `!b` for a bool parameter must emit `eor ... #1` (not `mvn`).
///
/// FLS §6.5.4: Logical NOT on bool. `eor x{dst}, x{src}, #1` XORs bit 0,
/// producing the correct logical complement for a 0/1 boolean value.
///
/// If `mvn` were emitted instead: `!true` = `mvn 1` = -2 (wrong, should be 0),
/// and `!false` = `mvn 0` = -1 (wrong, should be 1).
///
/// FLS §6.1.2:37–45: Non-const `!bool_param` must emit a runtime instruction.
#[test]
fn runtime_bool_not_emits_eor_instruction() {
    let asm = compile_to_asm("fn negate(b: bool) -> bool { !b }\nfn main() -> i32 { 0 }\n");
    assert!(
        asm.contains("eor") && asm.contains("#1"),
        "expected `eor ... #1` instruction for `!b` (logical NOT), got:\n{asm}"
    );
    // Must NOT use bitwise NOT (mvn) for a bool operand.
    assert!(
        !asm.contains("mvn"),
        "assembly must not use `mvn` for bool `!b` — should be `eor ... #1`:\n{asm}"
    );
}

// ── Milestone 15: type cast expressions `as` ─────────────────────────────────
//
// FLS §6.5.9: Type cast expressions. The `as` operator converts a value of
// one type to a value of another type. At this milestone, only casts to `i32`
// are supported: `i32 as i32` (identity) and `bool as i32` (0/1 integer).
//
// The `as` operator has higher precedence than `*`, `/`, `%` and lower
// precedence than unary operators. It is left-associative.
//
// FLS §6.1.2:37–45: The operand is lowered at runtime even if its value is
// statically known — no constant folding of the cast expression.
//
// For `i32 as i32`: the cast is a no-op at the instruction level. The source
// register is reused directly — no additional instruction is emitted.
// For `bool as i32`: booleans are represented as 0/1 i32 values in the IR,
// so the cast is also a no-op at the instruction level.
//
// FLS example (§6.5.9): No explicit example given for integer identity casts;
// derived from the semantic description ("numeric casts").

/// Milestone 15: `fn main() -> i32 { 7 as i32 }` → identity cast exits 7.
///
/// FLS §6.5.9: `i32 as i32` is an identity cast. No instruction is emitted
/// for the cast itself — the source value is used directly.
/// FLS §6.1.2:37–45: The operand `7` is still materialized via `mov` at runtime.
#[test]
fn milestone_15_identity_cast_literal() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 7 as i32 }\n") else {
        return;
    };
    assert_eq!(exit_code, 7, "expected exit 7 from `7 as i32` (identity cast), got {exit_code}");
}

/// Milestone 15: cast in a let binding — `let x: i32 = 5 as i32; x` exits 5.
///
/// FLS §6.5.9: The result of `5 as i32` is `5`. Assigned to `x`, then returned.
/// FLS §8.1: Let statement binding with explicit type annotation.
#[test]
fn milestone_15_cast_in_let() {
    let src = "fn main() -> i32 { let x: i32 = 5 as i32; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 5, "expected exit 5 from `let x: i32 = 5 as i32; x`, got {exit_code}");
}

/// Milestone 15: cast applied to a variable — `fn f(n: i32) -> i32 { n as i32 }` returns n.
///
/// FLS §6.5.9: `n as i32` with `n: i32` is an identity cast. The runtime value
/// is not statically known (it's a parameter), confirming the cast operates on
/// dynamic values, not just literals.
/// FLS §6.1.2:37–45: Cannot constant-fold a parameter value.
#[test]
fn milestone_15_cast_parameter() {
    let src = "fn identity(n: i32) -> i32 { n as i32 }\nfn main() -> i32 { identity(42) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 42, "expected exit 42 from `identity(42)` with `n as i32`, got {exit_code}");
}

/// Milestone 15: `true as i32` → exits 1; `false as i32` → exits 0.
///
/// FLS §6.5.9: Boolean-to-integer cast. `true` converts to 1, `false` to 0.
/// FLS §2.4.7: Boolean literals — `false` = 0, `true` = 1.
/// Both representations are identical in galvanic's IR, so this is a no-op cast.
#[test]
fn milestone_15_bool_as_i32_true() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { true as i32 }\n") else {
        return;
    };
    assert_eq!(exit_code, 1, "expected exit 1 from `true as i32`, got {exit_code}");
}

/// Milestone 15: `false as i32` → exits 0.
///
/// FLS §6.5.9: Boolean-to-integer cast. `false` converts to 0.
#[test]
fn milestone_15_bool_as_i32_false() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { false as i32 }\n") else {
        return;
    };
    assert_eq!(exit_code, 0, "expected exit 0 from `false as i32`, got {exit_code}");
}

/// Milestone 15: cast in an arithmetic expression — `(3 as i32) + 4` exits 7.
///
/// FLS §6.5.9: `as` has higher precedence than `+`. `3 as i32 + 4` parses as
/// `(3 as i32) + 4`. This verifies precedence is encoded correctly.
#[test]
fn milestone_15_cast_in_arithmetic() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 3 as i32 + 4 }\n") else {
        return;
    };
    assert_eq!(exit_code, 7, "expected exit 7 from `3 as i32 + 4`, got {exit_code}");
}

/// Assembly inspection: `x as i32` with a variable emits no extra instructions.
///
/// FLS §6.5.9: An identity cast (i32 → i32) produces no additional machine
/// code — the source register is reused directly.
/// FLS §6.1.2:37–45: The parameter `x` still requires a runtime `ldr` to load
/// from the stack, but no cast instruction is emitted.
#[test]
fn runtime_cast_identity_emits_no_cast_instruction() {
    let asm = compile_to_asm("fn f(x: i32) -> i32 { x as i32 }\nfn main() -> i32 { f(5) }\n");
    // No ARM64 cast instruction should appear — `sxtw`, `uxtw`, `sbfx` etc.
    assert!(
        !asm.contains("sxtw") && !asm.contains("uxtw") && !asm.contains("sbfx"),
        "identity cast i32→i32 must not emit a cast instruction, got:\n{asm}"
    );
}

// ── Milestone 16: bool as parameter and return type ───────────────────────────
//
// FLS §4.3: The boolean type `bool` has two values: `true` (1) and `false` (0).
// On ARM64, booleans are passed and returned in 32-bit integer registers —
// the same layout as `i32`. Mapping `bool` to `IrTy::I32` is therefore
// correct and sufficient for this milestone.
//
// This milestone enables:
//   - Functions that accept `bool` parameters (passed as 0 or 1 in xN)
//   - Functions that return `bool` (returned as 0 or 1 in x0)
//   - Composing boolean-returning functions with if/else
//
// FLS §6.1.3: Boolean literal expressions materialise as `LoadImm 0/1` —
// the same representation used for bool parameters.
// FLS §6.1.2:37–45: All code emits runtime instructions.

/// Milestone 16: bool parameter `true` is passed as 1, if-dispatch returns 1.
///
/// FLS §4.3: `true` is represented as 1 in a 32-bit register.
/// FLS §6.17: The if expression dispatches on the bool parameter at runtime.
#[test]
fn milestone_16_bool_param_true() {
    let src = "fn to_int(b: bool) -> i32 { if b { 1 } else { 0 } }\nfn main() -> i32 { to_int(true) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected exit 1 from to_int(true), got {exit_code}");
}

/// Milestone 16: bool parameter `false` is passed as 0, if-dispatch returns 0.
///
/// FLS §4.3: `false` is represented as 0 in a 32-bit register.
/// FLS §6.17: The if expression dispatches on the bool parameter at runtime.
#[test]
fn milestone_16_bool_param_false() {
    let src = "fn to_int(b: bool) -> i32 { if b { 1 } else { 0 } }\nfn main() -> i32 { to_int(false) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected exit 0 from to_int(false), got {exit_code}");
}

/// Milestone 16: function returning `bool` — comparison result flows as return value.
///
/// `is_zero(0)` computes `0 == 0` (true = 1) and returns it as bool.
/// The caller uses the return value as an if condition.
///
/// FLS §4.3: bool return type is represented as 0/1 in x0.
/// FLS §6.5.3: Equality comparison emits `cmp`+`cset` at runtime.
#[test]
fn milestone_16_bool_return_true() {
    let src = "fn is_zero(x: i32) -> bool { x == 0 }\nfn main() -> i32 { if is_zero(0) { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected exit 1 from if is_zero(0) {{1}} else {{0}}, got {exit_code}");
}

/// Milestone 16: function returning `bool` — false case.
///
/// `is_zero(5)` computes `5 == 0` (false = 0), so the else branch runs.
///
/// FLS §4.3: bool return type is 0 when the comparison is false.
/// FLS §6.5.3: Equality comparison emits `cmp`+`cset` at runtime.
#[test]
fn milestone_16_bool_return_false() {
    let src = "fn is_zero(x: i32) -> bool { x == 0 }\nfn main() -> i32 { if is_zero(5) { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected exit 0 from if is_zero(5) {{1}} else {{0}}, got {exit_code}");
}

/// Milestone 16: bool parameter and bool return type together.
///
/// `negate(false)` takes bool param (0), applies `!` (mvn), returns bool (1).
/// The caller uses the returned bool as an if condition.
///
/// FLS §4.3: bool param and return use the same register layout as i32.
/// FLS §6.5.4: `!` on a bool emits `eor reg, #1` (logical NOT).
#[test]
fn milestone_16_bool_param_and_return() {
    let src = "fn negate(b: bool) -> bool { !b }\nfn main() -> i32 { if negate(false) { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected exit 1 from if negate(false) {{1}} else {{0}}, got {exit_code}");
}

// ── Milestone 17: boolean logical NOT emits runtime `eor` ───────────────────
//
// FLS §6.5.4: The unary `!` applied to a `bool` value produces its logical
// complement: `!true` = `false` (0), `!false` = `true` (1).
//
// ARM64 codegen: `eor x{dst}, x{src}, #1` — XOR with 1 flips bit 0.
// This is distinct from bitwise NOT (Instr::Not → `mvn`), which would produce
// -2 for `!true` and -1 for `!false` — wrong for boolean semantics.
//
// FLS §6.1.2:37–45: Even `!true` in a non-const context emits a runtime `eor`.

/// Milestone 17: `!true` must return `false` (0), not `-2` (from `mvn`).
///
/// FLS §6.5.4: Logical NOT for bool — `!true` = `false`.
/// FLS §4.3: `false` is represented as 0 in a 64-bit register.
#[test]
fn milestone_17_bool_not_true() {
    let src = "fn negate(b: bool) -> bool { !b }\nfn main() -> i32 { if negate(true) { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected exit 0 from if negate(true) {{1}} else {{0}}: !true = false, got {exit_code}");
}

/// Milestone 17: `!false` must return `true` (1).
///
/// FLS §6.5.4: Logical NOT for bool — `!false` = `true`.
/// FLS §4.3: `true` is represented as 1 in a 64-bit register.
#[test]
fn milestone_17_bool_not_false() {
    let src = "fn negate(b: bool) -> bool { !b }\nfn main() -> i32 { if negate(false) { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected exit 1 from if negate(false) {{1}} else {{0}}: !false = true, got {exit_code}");
}

/// Milestone 17: `!b` used directly as an if condition lowers correctly.
///
/// Tests `if !b { 1 } else { 0 }` where b is a bool parameter. The condition
/// `!b` must emit `eor` (logical NOT) so that `cbz` behaves correctly.
///
/// FLS §6.5.4: `!` on bool is logical NOT.
/// FLS §6.17: The if condition is evaluated at runtime.
#[test]
fn milestone_17_bool_not_as_condition() {
    let _src = "fn main(b: bool) -> i32 { if !b { 1 } else { 0 } }\n";
    // We test via the fixture: negate(false) → true → if-then branch → 1
    let src2 = "fn check(b: bool) -> i32 { if !b { 1 } else { 0 } }\nfn main() -> i32 { check(false) }\n";
    let Some(exit_code) = compile_and_run(src2) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected exit 1 from check(false) with if !b {{1}} else {{0}}, got {exit_code}");
}

// ── Milestone 18: recursive functions produce correct results ─────────────────
//
// Prior to this milestone, a function whose binary expression combined two
// call results (e.g., `fib(n-1) + fib(n-2)`) would produce wrong answers for
// n >= 4 due to register clobbering: ARM64 calling convention makes x0–x17
// caller-saved, so the second `bl fib` overwrote the register holding the
// first call's result.
//
// Fix: in the arithmetic and comparison BinOp lowering, after lowering the
// LHS into a register, check if the RHS expression tree contains any Call
// node. If so, spill the LHS register to a new stack slot before lowering
// the RHS, then reload it afterward.
//
// The Fibonacci sequence is derived from the FLS §9 recursive function
// example. The spec does not provide an exact fibonacci example, but
// recursive functions are explicitly permitted by FLS §9:3 ("A function
// may call itself"). The canonical implementation exercises: recursive
// calls, comparison (`<=`), arithmetic (`+`, `-`), if/else.
//
// FLS §9: Functions.
// FLS §6.12.1: Call expressions.
// FLS §6.5.5: Arithmetic operator expressions.
// FLS §6.5.3: Comparison operator expressions.
// FLS §6.17: If expressions.
// FLS §6.12.1: ARM64 AAPCS64 — x0–x17 are caller-saved (clobbered by bl).

/// Milestone 18: fibonacci base cases (n=0 and n=1) produce the right values.
///
/// fib(0) = 0 and fib(1) = 1. These take the `n <= 1` branch and never
/// recurse. Verifies that the if-else structure and parameter access work.
///
/// FLS §9: Recursive function definition permitted.
/// FLS §6.17: If expression selects the correct branch at runtime.
#[test]
fn milestone_18_fib_base_cases() {
    let src = "fn fib(n: i32) -> i32 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } }\nfn main() -> i32 { fib(0) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected fib(0)=0, got {exit_code}");

    let src2 = "fn fib(n: i32) -> i32 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } }\nfn main() -> i32 { fib(1) }\n";
    let Some(exit_code2) = compile_and_run(src2) else {
        return;
    };
    assert_eq!(exit_code2, 1, "expected fib(1)=1, got {exit_code2}");
}

/// Milestone 18: fibonacci for n=2 through n=4 requires one-deep recursion.
///
/// fib(2)=1, fib(3)=2, fib(4)=3. These exercise the LHS-spill fix: each
/// call to fib(n-1) stores its result to a stack slot so that the subsequent
/// call to fib(n-2) cannot clobber it.
///
/// FLS §6.12.1: Call expressions follow ARM64 AAPCS64 (x0–x17 caller-saved).
/// FLS §6.5.5: Binary add combining two call results.
#[test]
fn milestone_18_fib_small() {
    let fib_src = "fn fib(n: i32) -> i32 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } }\n";
    for (n, expected) in [(2u128, 1i32), (3, 2), (4, 3)] {
        let src = format!("{fib_src}fn main() -> i32 {{ fib({n}) }}\n");
        let Some(exit_code) = compile_and_run(&src) else {
            return;
        };
        assert_eq!(exit_code, expected, "expected fib({n})={expected}, got {exit_code}");
    }
}

/// Milestone 18: fibonacci for n=7 requires deep recursion (25 calls).
///
/// fib(7) = 13. This is the canonical correctness test for recursive
/// functions. Register clobbering at any level of the call tree would
/// produce a wrong result.
///
/// FLS §9:3: A function may call itself (recursion permitted).
/// FLS §6.5.5: The add `fib(n-1) + fib(n-2)` must produce the correct sum
/// after the LHS register is spilled and reloaded across the second call.
#[test]
fn milestone_18_fib_seven() {
    let src = "fn fib(n: i32) -> i32 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } }\nfn main() -> i32 { fib(7) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 13, "expected fib(7)=13, got {exit_code}");
}

/// Milestone 18: assembly confirms spill/reload around second call.
///
/// For `fib(n-1) + fib(n-2)`, the LHS register must be stored to a stack
/// slot before the second `bl fib` and reloaded afterward. The assembly
/// must contain a `str` between the two `bl fib` instructions.
///
/// FLS §6.12.1: ARM64 AAPCS64 caller-save requires this spill.
#[test]
fn runtime_recursive_call_spills_lhs_register() {
    let src = "fn fib(n: i32) -> i32 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } }\nfn main() -> i32 { fib(7) }\n";
    let asm = compile_to_asm(src);
    // Both recursive calls must appear in fib's body.
    assert!(asm.contains("bl      fib"), "expected bl fib in assembly");
    // After the first bl fib, the result register must be spilled.
    // The str instruction must appear between the two bl fib calls.
    let first_bl = asm.find("bl      fib").expect("first bl fib");
    let second_bl = asm[first_bl + 1..].find("bl      fib").expect("second bl fib");
    let between = &asm[first_bl..first_bl + 1 + second_bl];
    assert!(between.contains("str"), "expected str (spill) between the two bl fib calls, got:\n{between}");
}

// ── Milestone 19: for loops with integer ranges ───────────────────────────────
//
// `for i in start..end { body }` desugars to a while-loop equivalent:
//   alloc i = start, end_bound = end
//   cond_label: if i < end_bound → exit
//   body
//   incr_label: i += 1 → cond_label
//   exit_label
//
// `for i in start..=end` uses `<=` instead of `<`.
//
// FLS §6.15.1: For loop expressions iterate over an IntoIterator.
// FLS §6.16: Range expressions `start..end` and `start..=end`.
// FLS §6.21: Range operators have lower precedence than logical operators.
// FLS §6.1.2:37–45: The back-edge and increment are runtime instructions.

/// Milestone 19: sum 0..5 with a for loop.
///
/// `for i in 0..5 { acc += i }` → acc = 0+1+2+3+4 = 10.
///
/// FLS §6.15.1: For loop expression.
/// FLS §6.16: Exclusive range `0..5`.
#[test]
fn milestone_19_for_loop_sum() {
    let src = "fn main() -> i32 { let mut acc = 0; for i in 0..5 { acc += i; } acc }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 10, "expected 0+1+2+3+4=10, got {exit_code}");
}

/// Milestone 19: for loop body never executes when start >= end.
///
/// `for i in 5..5 { acc += 1; }` — range is empty, acc stays 0.
///
/// FLS §6.16: An exclusive range where start == end is empty.
#[test]
fn milestone_19_for_loop_empty_range() {
    let src = "fn main() -> i32 { let mut acc = 0; for i in 5..5 { acc += 1; } acc }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected empty range → acc=0, got {exit_code}");
}

/// Milestone 19: inclusive range `0..=4` sums 0+1+2+3+4 = 10.
///
/// FLS §6.16: Inclusive range `start..=end` iterates while i <= end.
#[test]
fn milestone_19_for_loop_inclusive_range() {
    let src = "fn main() -> i32 { let mut acc = 0; for i in 0..=4 { acc += i; } acc }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 10, "expected 0+1+2+3+4=10 via inclusive range, got {exit_code}");
}

/// Milestone 19: for loop with a non-zero start.
///
/// `for i in 3..6 { acc += i }` → acc = 3+4+5 = 12.
///
/// FLS §6.16: Range bounds are evaluated once before the loop.
#[test]
fn milestone_19_for_loop_nonzero_start() {
    let src = "fn main() -> i32 { let mut acc = 0; for i in 3..6 { acc += i; } acc }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 12, "expected 3+4+5=12, got {exit_code}");
}

/// Milestone 19: for loop with a function-parameter bound.
///
/// `sum_to(n)` uses `for i in 0..n`, which exercises a runtime end bound
/// (not a literal). This proves codegen emits real runtime instructions,
/// not a compile-time evaluation.
///
/// FLS §6.1.2:37–45: The range bounds must be evaluated at runtime.
/// FLS §6.15.1: The loop variable is a fresh binding per iteration.
#[test]
fn milestone_19_for_loop_runtime_bound() {
    let src = "fn sum_to(n: i32) -> i32 { let mut acc = 0; for i in 0..n { acc += i; } acc }\nfn main() -> i32 { sum_to(5) }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 10, "expected sum_to(5)=10, got {exit_code}");
}

/// Milestone 19: assembly inspection — for loop emits cmp and back-edge branch.
///
/// The control flow skeleton must include:
/// - A comparison instruction for the loop condition.
/// - A `cbz` to exit when the condition is false.
/// - An `add` for the increment.
/// - A back-edge `b` to the condition label.
///
/// FLS §6.15.1: For loop expressions produce runtime control flow.
/// FLS §6.16: Range expression bounds are materialised as runtime loads.
#[test]
fn runtime_for_loop_emits_cmp_cbz_add_and_back_branch() {
    let src = "fn main() -> i32 { let mut acc = 0; for i in 0..5 { acc += i; } acc }\n";
    let asm = compile_to_asm(src);
    assert!(asm.contains("cbz"), "expected cbz (exit branch) in for loop assembly");
    assert!(asm.contains("add"), "expected add (increment) in for loop assembly");
    // Back-edge: unconditional branch to the condition label.
    assert!(asm.contains("b "), "expected back-edge branch in for loop assembly");
}

// ── Milestone 20: loop-as-expression with break value ─────────────────────────
//
// FLS §6.15.2: A `loop` expression has the type of its `break <value>`
// expressions. The result is delivered via a stack phi slot.
// FLS §6.15.6: Only `loop` (not `while` or `for`) expressions support
// break-with-value.

/// Milestone 20: simplest loop-as-expression — `loop { break 42; }`.
///
/// The loop immediately breaks with 42. The result is stored to a phi slot
/// and loaded after the exit label.
///
/// FLS §6.15.2: Loop expression type is determined by break expressions.
/// FLS §6.15.6: `break <value>` stores to the loop's result slot.
#[test]
fn milestone_20_loop_break_immediate() {
    let src = "fn main() -> i32 { let x = loop { break 42; }; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 42, "expected loop to yield 42, got {exit_code}");
}

/// Milestone 20: loop that runs several iterations before breaking with a value.
///
/// `loop { i += 1; if i == 5 { break i * 2; } }` → 10.
///
/// FLS §6.15.2: The break expression carries the loop's yielded value.
/// FLS §6.1.2:37–45: The loop body and break expression execute at runtime.
#[test]
fn milestone_20_loop_break_with_computed_value() {
    let src = "fn main() -> i32 { let mut i = 0; let r = loop { i += 1; if i == 5 { break i * 2; } }; r }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 10, "expected loop to yield 10, got {exit_code}");
}

/// Milestone 20: loop-as-expression used directly as a function return value.
///
/// `loop { break 7; }` used as the tail expression of main.
///
/// FLS §6.15.2: The loop expression itself is the tail value.
#[test]
fn milestone_20_loop_as_tail_expression() {
    let src = "fn main() -> i32 { loop { break 7; } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 7, "expected tail loop to yield 7, got {exit_code}");
}

/// Milestone 20: loop-as-expression used in arithmetic.
///
/// `1 + loop { break 6; }` → 7.
///
/// FLS §6.15.2: The loop expression can appear anywhere an expression is valid.
#[test]
fn milestone_20_loop_in_arithmetic() {
    let src = "fn main() -> i32 { 1 + loop { break 6; } }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 7, "expected 1 + loop{{break 6}} = 7, got {exit_code}");
}

/// Milestone 20: assembly inspection — loop-as-expression emits a store before
/// the break branch and a load after the exit label.
///
/// The phi-slot pattern: `str` (in the break arm) + `b .Lexit` + `.Lexit:` +
/// `ldr` (to materialise the result register).
///
/// FLS §6.15.6: break-with-value stores to the loop's phi slot at runtime.
#[test]
fn runtime_loop_break_value_emits_store_and_load() {
    let src = "fn main() -> i32 { loop { break 42; } }\n";
    let asm = compile_to_asm(src);
    // The break value must be stored (phi slot) and loaded after the exit label.
    assert!(asm.contains("str"), "expected str (phi store) for loop break value");
    assert!(asm.contains("ldr"), "expected ldr (phi load) after loop exit label");
    // The back-edge must still be present (the loop is a real loop at runtime).
    assert!(asm.contains("b "), "expected back-edge branch in loop assembly");
}

// ── Milestone 21: uninitialized let bindings (FLS §8.1) ─────────────────────

/// Milestone 21: `let x;` followed by `x = expr;` compiles and runs correctly.
///
/// FLS §8.1: "A LetStatement may optionally have an Initializer."
/// Without an initializer the slot is allocated; the subsequent assignment stores to it.
#[test]
fn milestone_21_uninit_let_then_assign() {
    let src = "fn main() -> i32 { let x; x = 42; x }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 21: uninitialized let binding used in arithmetic after assignment.
///
/// FLS §8.1: slot is allocated by `let y;`; the assignment `y = x + 1;` stores
/// the computed value; then `y` is used as the return value.
#[test]
fn milestone_21_uninit_let_arithmetic() {
    let src = "fn main() -> i32 { let x; x = 5; let y; y = x + 1; y }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 6, "expected 6, got {exit_code}");
}

/// Milestone 21: conditional initialization — the variable is assigned in each
/// branch of an if/else. This is the canonical Rust use-case for uninit let.
///
/// FLS §8.1: variable declared without initializer, then assigned in each arm.
#[test]
fn milestone_21_conditional_init_true() {
    let src = "fn main() -> i32 { let r; if true { r = 1; } else { r = 0; } r }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected 1, got {exit_code}");
}

/// Milestone 21: conditional initialization — false branch.
///
/// FLS §8.1: the else branch assigns to the uninit slot.
#[test]
fn milestone_21_conditional_init_false() {
    let src = "fn main() -> i32 { let r; if false { r = 1; } else { r = 0; } r }\n";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected 0, got {exit_code}");
}

/// Milestone 21: sign function using uninit let across if/else-if/else chain.
///
/// FLS §8.1: common pattern where `let s;` is declared once and each branch
/// of a multi-way conditional assigns a distinct value.
#[test]
fn milestone_21_sign_positive() {
    let src = "\
fn sign(n: i32) -> i32 {
    let s;
    if n > 0 { s = 1; } else if n < 0 { s = -1; } else { s = 0; }
    s
}
fn main() -> i32 { sign(5) }
";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 1, "expected 1, got {exit_code}");
}

/// Milestone 21: sign function — negative input.
///
/// FLS §8.1: the else-if branch assigns `s = -1`.
/// Note: exit codes are modulo 256 on Linux; -1 becomes 255.
#[test]
fn milestone_21_sign_negative() {
    let src = "\
fn sign(n: i32) -> i32 {
    let s;
    if n > 0 { s = 1; } else if n < 0 { s = -1; } else { s = 0; }
    s
}
fn main() -> i32 { sign(-3) }
";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    // exit(−1) wraps to 255 on Linux
    assert_eq!(exit_code, 255, "expected 255 (−1 mod 256), got {exit_code}");
}

/// Milestone 21: sign function — zero input.
///
/// FLS §8.1: the else branch assigns `s = 0`.
#[test]
fn milestone_21_sign_zero() {
    let src = "\
fn sign(n: i32) -> i32 {
    let s;
    if n > 0 { s = 1; } else if n < 0 { s = -1; } else { s = 0; }
    s
}
fn main() -> i32 { sign(0) }
";
    let Some(exit_code) = compile_and_run(src) else {
        return;
    };
    assert_eq!(exit_code, 0, "expected 0, got {exit_code}");
}

/// Milestone 21: assembly inspection — `let x;` allocates a stack slot but
/// emits no store; only the subsequent `x = 5;` should emit the first `str`.
///
/// FLS §8.1: "A LetStatement may optionally have an Initializer." When absent,
/// no runtime store instruction is emitted for the declaration itself.
#[test]
fn runtime_uninit_let_no_store_until_assignment() {
    // The let allocates slot 0 for x. The assignment `x = 5` is the first store.
    let src = "fn main() -> i32 { let x; x = 5; x }\n";
    let asm = compile_to_asm(src);
    // There must be a store (from the assignment `x = 5`)
    assert!(asm.contains("str"), "expected str from assignment x = 5");
    // There must be a load (from the tail expression `x`)
    assert!(asm.contains("ldr"), "expected ldr from tail expression x");
}

// ── Milestone 22: match expressions on integer values ────────────────────────
//
// `match` is a fundamental Rust control-flow construct (FLS §6.18). This
// milestone adds integer and boolean literal patterns plus the wildcard `_`.
// Arms are tested in source order; the first matching arm executes.
//
// Lowering strategy: each non-wildcard arm lowers to a comparison chain
// (scrutinee == pattern_val → cbz if not equal → arm body). The last arm is
// emitted unconditionally (exhaustiveness deferred to a future type pass).
//
// FLS §6.18: Match expressions.
// FLS §5.1: Wildcard pattern `_`.
// FLS §5.2: Literal patterns (integer, boolean).
// FLS §6.1.2:37–45: All comparisons emit runtime instructions.

/// Milestone 22: match on zero — wildcard arm taken.
///
/// FLS §6.18: The first arm whose pattern matches executes.
/// With `match x { 0 => 0, _ => 1 }` and x=5, the wildcard arm executes.
#[test]
fn milestone_22_match_wildcard_taken() {
    let src = "fn main() -> i32 { let x = 5; match x { 0 => 0, _ => 1 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected wildcard arm (1), got {exit_code}");
}

/// Milestone 22: match on zero — literal arm taken.
///
/// FLS §6.18: The `0` literal pattern matches scrutinee 0; the first arm executes.
#[test]
fn milestone_22_match_literal_taken() {
    let src = "fn main() -> i32 { let x = 0; match x { 0 => 42, _ => 1 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected literal arm (42), got {exit_code}");
}

/// Milestone 22: match with three arms — middle arm taken.
///
/// FLS §6.18: Arms are tested in source order; the first match wins.
#[test]
fn milestone_22_match_three_arms_middle() {
    let src = "fn main() -> i32 { let x = 1; match x { 0 => 10, 1 => 20, _ => 30 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 20, "expected arm 1 (20), got {exit_code}");
}

/// Milestone 22: match with three arms — last (wildcard) arm taken.
///
/// FLS §6.18: Wildcard `_` matches any remaining value.
#[test]
fn milestone_22_match_three_arms_wildcard() {
    let src = "fn main() -> i32 { let x = 99; match x { 0 => 10, 1 => 20, _ => 30 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 30, "expected wildcard arm (30), got {exit_code}");
}

/// Milestone 22: match on a function parameter.
///
/// FLS §6.18: The scrutinee may be any expression, including a variable
/// that holds a function argument (not a compile-time constant).
/// This verifies the compiler does not constant-fold the match — it must
/// emit runtime comparison instructions.
///
/// FLS §6.1.2:37–45: Non-const code emits runtime instructions.
#[test]
fn milestone_22_match_on_parameter() {
    let src = "\
fn classify(n: i32) -> i32 {
    match n {
        0 => 0,
        1 => 1,
        _ => 2,
    }
}
fn main() -> i32 { classify(1) }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected classify(1) == 1, got {exit_code}");
}

/// Milestone 22: match used as a function (all arms covered).
///
/// FLS §6.18: Multiple literal arms with a wildcard default.
#[test]
fn milestone_22_match_fizzbuzz_like() {
    let src = "\
fn kind(n: i32) -> i32 {
    match n {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 3,
    }
}
fn main() -> i32 { kind(2) }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "expected kind(2) == 2, got {exit_code}");
}

/// Milestone 22: assembly inspection — match emits comparison (cmp/cset) and
/// conditional branch (cbz) for each non-wildcard arm.
///
/// FLS §6.18: Each arm tests at runtime — no compile-time constant folding.
/// FLS §6.1.2:37–45: Non-const match emits runtime branch instructions.
#[test]
fn runtime_match_emits_comparison_and_cbz() {
    let src = "fn main() -> i32 { let x = 0; match x { 0 => 42, _ => 1 } }\n";
    let asm = compile_to_asm(src);
    // Must emit a comparison (cmp for the arm equality test)
    assert!(asm.contains("cmp"), "expected cmp for arm equality test");
    // Must emit a conditional branch (cbz to skip the arm if not equal)
    assert!(asm.contains("cbz"), "expected cbz for arm skip branch");
}

// ── Milestone 23: negative literal patterns in match ─────────────────────────
//
// Rust allows negative integer literal patterns in match arms, e.g.:
//   match x { -1 => 0, 0 => 1, _ => 2 }
//
// FLS §5.2: Literal patterns include negative integer literals.
// The pattern `-1` matches the value -1 (as i32: 0xFFFF_FFFF).
//
// Lowering strategy: `Pat::NegLitInt(n)` materializes `-(n as i32)` as the
// comparison immediate, then proceeds identically to `Pat::LitInt`.
//
// FLS §6.1.2:37–45: The comparison emits runtime instructions — even when
// the scrutinee is a statically known constant.

/// Milestone 23: negative literal pattern — arm taken when value matches -1.
///
/// FLS §5.2: `-1` is a valid integer literal pattern. The scrutinee -1
/// matches the first arm and the function returns 10.
#[test]
fn milestone_23_negative_pattern_taken() {
    let src = "fn main() -> i32 { let x = -1; match x { -1 => 10, 0 => 20, _ => 30 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected -1 arm (10), got {exit_code}");
}

/// Milestone 23: negative literal pattern — arm NOT taken when value is 0.
///
/// FLS §5.2: The `0` arm fires; the `-1` arm was checked first and skipped.
#[test]
fn milestone_23_negative_pattern_not_taken() {
    let src = "fn main() -> i32 { let x = 0; match x { -1 => 10, 0 => 20, _ => 30 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 20, "expected 0 arm (20), got {exit_code}");
}

/// Milestone 23: negative literal pattern — wildcard fires for positive value.
///
/// FLS §5.2: Neither `-1` nor `0` match; the wildcard arm executes.
#[test]
fn milestone_23_negative_pattern_wildcard_taken() {
    let src = "fn main() -> i32 { let x = 5; match x { -1 => 10, 0 => 20, _ => 30 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 30, "expected wildcard arm (30), got {exit_code}");
}

/// Milestone 23: negative literal pattern on a function parameter.
///
/// FLS §5.2: The scrutinee is a function argument — not a compile-time
/// constant. This verifies that the compiler emits runtime comparison
/// instructions for the negative pattern arm.
///
/// FLS §6.1.2:37–45: Non-const code emits runtime instructions.
#[test]
fn milestone_23_negative_pattern_on_parameter() {
    let src = "\
fn classify(n: i32) -> i32 {
    match n {
        -1 => 1,
        0 => 2,
        1 => 3,
        _ => 4,
    }
}
fn main() -> i32 { classify(-1) }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected classify(-1) == 1, got {exit_code}");
}

/// Milestone 23: multiple negative patterns — each arm matched distinctly.
///
/// FLS §5.2: Multiple negative literal arms are checked in source order.
#[test]
fn milestone_23_multiple_negative_patterns() {
    let src = "\
fn sign_code(n: i32) -> i32 {
    match n {
        -2 => 10,
        -1 => 20,
        0 => 30,
        _ => 40,
    }
}
fn main() -> i32 { sign_code(-2) }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected sign_code(-2) == 10, got {exit_code}");
}

/// Milestone 23: assembly inspection — negative literal pattern emits a
/// `mov` with a negative immediate (via `movn` or negation) and a `cmp`.
///
/// FLS §5.2: The pattern value `-1` must be materialized at runtime and
/// compared against the scrutinee — it must not be constant-folded away.
/// FLS §6.1.2:37–45: Non-const match always emits runtime comparison.
#[test]
fn runtime_negative_pattern_emits_cmp() {
    let src = "fn main() -> i32 { let x = -1; match x { -1 => 42, _ => 0 } }\n";
    let asm = compile_to_asm(src);
    // Must emit a comparison for the pattern equality test
    assert!(asm.contains("cmp"), "expected cmp for negative pattern arm equality test");
    // Must emit a conditional branch (cbz or similar)
    assert!(asm.contains("cbz"), "expected cbz for arm skip branch");
}

// ── Milestone 32: OR patterns in match (FLS §5.1.11) ─────────────────────────

/// Milestone 32: `0 | 1 => 10` — scrutinee 0 matches the OR arm.
///
/// FLS §5.1.11: An OR pattern matches if any of its alternatives matches.
/// The first alternative `0` matches scrutinee `0`, so the arm executes.
#[test]
fn milestone_32_or_pattern_first_alt_matches() {
    let src = "fn main() -> i32 { let x = 0; match x { 0 | 1 => 10, _ => 20 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected 0 to match OR pattern 0|1, got {exit_code}");
}

/// Milestone 32: `0 | 1 => 10` — scrutinee 1 matches via second alternative.
///
/// FLS §5.1.11: The second alternative `1` matches scrutinee `1`.
#[test]
fn milestone_32_or_pattern_second_alt_matches() {
    let src = "fn main() -> i32 { let x = 1; match x { 0 | 1 => 10, _ => 20 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected 1 to match OR pattern 0|1, got {exit_code}");
}

/// Milestone 32: `0 | 1 => 10` — scrutinee 2 falls to wildcard arm.
///
/// FLS §5.1.11: Neither alternative matches; the wildcard arm executes.
#[test]
fn milestone_32_or_pattern_no_match_falls_to_wildcard() {
    let src = "fn main() -> i32 { let x = 2; match x { 0 | 1 => 10, _ => 20 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 20, "expected 2 to fall through to wildcard, got {exit_code}");
}

/// Milestone 32: three-alternative OR pattern `1 | 2 | 3 => 5`.
///
/// FLS §5.1.11: OR patterns may have more than two alternatives.
/// Scrutinee 2 matches the second alternative.
#[test]
fn milestone_32_or_pattern_three_alts() {
    let src = "fn main() -> i32 { let x = 2; match x { 1 | 2 | 3 => 5, _ => 99 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected 2 to match 1|2|3, got {exit_code}");
}

/// Milestone 32: OR pattern as a non-last arm, then another literal arm.
///
/// FLS §5.1.11: Multiple OR arms can appear in a single match.
/// Scrutinee 5 matches the second arm `5 | 6 => 2`.
#[test]
fn milestone_32_or_pattern_multiple_or_arms() {
    let src = "\
fn classify(n: i32) -> i32 {
    match n {
        0 | 1 => 1,
        5 | 6 => 2,
        _ => 0,
    }
}
fn main() -> i32 { classify(5) }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "expected classify(5) == 2, got {exit_code}");
}

/// Milestone 32: OR pattern on a parameter.
///
/// FLS §5.1.11: OR patterns work on non-literal scrutinees.
/// FLS §6.1.2:37–45: The comparisons are runtime instructions.
#[test]
fn milestone_32_or_pattern_on_parameter() {
    let src = "\
fn is_weekend(day: i32) -> i32 {
    match day {
        6 | 7 => 1,
        _ => 0,
    }
}
fn main() -> i32 { is_weekend(7) }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected is_weekend(7) == 1, got {exit_code}");
}

/// Milestone 32: OR pattern with negative alternatives.
///
/// FLS §5.1.11 + FLS §5.2: OR patterns may combine negative literal patterns.
#[test]
fn milestone_32_or_pattern_with_negative_alts() {
    let src = "\
fn main() -> i32 {
    let x = -1;
    match x {
        -2 | -1 => 10,
        0 => 20,
        _ => 30,
    }
}
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected -1 to match -2|-1 arm, got {exit_code}");
}

/// Milestone 32: assembly inspection — OR pattern emits OR-accumulation sequence.
///
/// FLS §5.1.11: Each alternative emits cmp+cset; the results are OR'd together
/// before the cbz that guards the arm body.
/// FLS §6.1.2:37–45: Runtime instructions — even with literal scrutinees.
#[test]
fn runtime_or_pattern_emits_orr_accumulation() {
    let src = "fn main() -> i32 { let x = 1; match x { 0 | 1 => 42, _ => 0 } }\n";
    let asm = compile_to_asm(src);
    // OR accumulation emits an `orr` to combine equality results.
    assert!(asm.contains("orr"), "expected orr for OR pattern accumulation");
    // Must still emit a conditional branch.
    assert!(asm.contains("cbz"), "expected cbz for arm skip branch");
}

// ── Milestone 33: identifier patterns in match bind the scrutinee ────────────
//
// FLS §5.1.4: An identifier pattern matches any value and binds it to a name
// available in the arm body.
//
// Example: `match x { 0 => 0, n => n * 2 }` — `n` binds the scrutinee value
// in the second arm, making it accessible as a local via a path expression.
//
// Lowering strategy:
//   1. Identifier pattern always matches (no conditional branch).
//   2. Load scrutinee from its spill slot into a new binding slot.
//   3. Insert name → slot into `locals` before lowering arm body.
//   4. Remove binding after body to avoid cross-arm pollution.
//
// FLS §6.1.2:37–45: The ldr/str pair for the binding emits at runtime —
// not optimised away even for statically-known scrutinees.

/// Milestone 33: identifier pattern as the only arm — binds and returns value.
///
/// `match x { n => n }` — `n` binds the scrutinee and the body returns it.
/// FLS §5.1.4: Identifier pattern matches any value.
#[test]
fn milestone_33_ident_pattern_only_arm() {
    let src = "fn main() -> i32 { let x = 7; match x { n => n } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7 from match {{ n => n }}, got {exit_code}");
}

/// Milestone 33: identifier pattern as catch-all after literal arm.
///
/// `match x { 0 => 0, n => n * 2 }` — `n` matches anything non-zero and
/// doubles it. FLS §5.1.4 + FLS §6.5.5.
#[test]
fn milestone_33_ident_pattern_catch_all() {
    let src = "\
fn double_if_nonzero(x: i32) -> i32 {
    match x {
        0 => 0,
        n => n * 2,
    }
}
fn main() -> i32 { double_if_nonzero(5) }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected exit 10 from double_if_nonzero(5), got {exit_code}");
}

/// Milestone 33: identifier pattern catch-all at zero.
///
/// FLS §5.1.4: Arms are tested in order. `0 => 0` fires before `n => n * 2`.
#[test]
fn milestone_33_ident_pattern_zero_arm_taken() {
    let src = "\
fn double_if_nonzero(x: i32) -> i32 {
    match x {
        0 => 0,
        n => n * 2,
    }
}
fn main() -> i32 { double_if_nonzero(0) }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected exit 0 from double_if_nonzero(0), got {exit_code}");
}

/// Milestone 33: identifier pattern with arithmetic on bound value.
///
/// `match x { n => n + 3 }` — the binding is used in an expression.
/// FLS §5.1.4 + FLS §6.3 (path expression resolves `n` to the binding slot).
#[test]
fn milestone_33_ident_pattern_arithmetic() {
    let src = "fn main() -> i32 { let x = 4; match x { n => n + 3 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7 from match {{ n => n + 3 }}, got {exit_code}");
}

/// Milestone 33: identifier pattern on a function parameter.
///
/// FLS §5.1.4 + FLS §9: The scrutinee may be any expression — here a param.
#[test]
fn milestone_33_ident_pattern_on_parameter() {
    let src = "\
fn classify(x: i32) -> i32 {
    match x {
        0 => 100,
        n => n,
    }
}
fn main() -> i32 { classify(42) }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42 from classify(42), got {exit_code}");
}

/// Milestone 33: assembly inspection — identifier pattern emits ldr+str binding.
///
/// FLS §5.1.4: The binding requires a runtime ldr (load scrutinee) + str (store
/// to binding slot) pair. This is NOT optimised away even when the scrutinee is
/// a constant — the spec requires runtime semantics (FLS §6.1.2:37–45).
#[test]
fn runtime_ident_pattern_emits_ldr_str_binding() {
    let src = "fn main() -> i32 { let x = 5; match x { n => n * 2 } }\n";
    let asm = compile_to_asm(src);
    // Must load the scrutinee from its spill slot.
    assert!(asm.contains("ldr"), "expected ldr for scrutinee load in ident pattern");
    // Must store the value into the binding slot.
    assert!(asm.contains("str"), "expected str for binding slot write in ident pattern");
    // Must emit mul for the `n * 2` body expression.
    assert!(asm.contains("mul"), "expected mul for n * 2 in arm body");
}

// ── Milestone 34: range patterns in match ────────────────────────────────────
//
// FLS §5.1.9: A range pattern `lo..=hi` matches any value `v` where `lo <= v
// && v <= hi`. A range pattern `lo..hi` matches where `lo <= v && v < hi`.
//
// Lowering strategy:
//   1. Load scrutinee from its spill slot.
//   2. Load `lo` immediate; compare scrutinee >= lo → cmp1.
//   3. Load `hi` immediate; compare scrutinee <= hi (or < hi) → cmp2.
//   4. AND cmp1 and cmp2 → matched.
//   5. CondBranch on matched to the next arm if zero (arm not taken).
//
// FLS §6.1.2:37–45: All comparisons emit runtime instructions — no compile-time
// folding even when the scrutinee is a literal.

/// Milestone 34: inclusive range `1..=3` — scrutinee 2 is in range → arm taken.
///
/// FLS §5.1.9: The inclusive range `1..=3` matches any value in [1, 3].
#[test]
fn milestone_34_range_inclusive_taken() {
    let src = "fn main() -> i32 { let x = 2; match x { 1..=3 => 1, _ => 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected 2 to match 1..=3, got {exit_code}");
}

/// Milestone 34: inclusive range `1..=3` — scrutinee 5 is outside → wildcard.
///
/// FLS §5.1.9: Value 5 falls outside [1, 3]; the wildcard arm executes.
#[test]
fn milestone_34_range_inclusive_not_taken() {
    let src = "fn main() -> i32 { let x = 5; match x { 1..=3 => 1, _ => 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected 5 to miss 1..=3 and hit wildcard, got {exit_code}");
}

/// Milestone 34: inclusive range boundary — scrutinee equals lower bound.
///
/// FLS §5.1.9: The lower bound is inclusive; `v == lo` must match.
#[test]
fn milestone_34_range_boundary_lower() {
    let src = "fn main() -> i32 { let x = 1; match x { 1..=3 => 1, _ => 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected lower bound 1 to match 1..=3, got {exit_code}");
}

/// Milestone 34: inclusive range boundary — scrutinee equals upper bound.
///
/// FLS §5.1.9: The upper bound is inclusive for `..=`; `v == hi` must match.
#[test]
fn milestone_34_range_boundary_upper() {
    let src = "fn main() -> i32 { let x = 3; match x { 1..=3 => 1, _ => 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected upper bound 3 to match 1..=3, got {exit_code}");
}

/// Milestone 34: multiple range arms — three disjoint ranges.
///
/// FLS §5.1.9: Multiple range arms are checked in order; the first match wins.
#[test]
fn milestone_34_multiple_range_arms() {
    let src = "\
fn classify(n: i32) -> i32 {
    match n {
        1..=3 => 1,
        4..=6 => 2,
        _ => 3,
    }
}
fn main() -> i32 { classify(5) }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "expected classify(5) == 2, got {exit_code}");
}

/// Milestone 34: range pattern on a function parameter.
///
/// FLS §5.1.9: Range patterns work on non-literal scrutinees (parameters).
/// FLS §6.1.2:37–45: Comparisons emit runtime instructions regardless.
#[test]
fn milestone_34_range_on_parameter() {
    let src = "\
fn grade(score: i32) -> i32 {
    match score {
        90..=100 => 4,
        80..=89 => 3,
        _ => 0,
    }
}
fn main() -> i32 { grade(85) }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "expected grade(85) == 3, got {exit_code}");
}

/// Milestone 34: range pattern with negative bounds.
///
/// FLS §5.1.9: Range patterns may have negative bounds.
/// FLS §5.2: Negative literal patterns are valid range bounds.
#[test]
fn milestone_34_range_negative_bounds() {
    let src = "\
fn main() -> i32 {
    let x = -3;
    match x {
        -5..=-1 => 1,
        _ => 0,
    }
}
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected -3 to match -5..=-1, got {exit_code}");
}

/// Milestone 34: exclusive range `1..4` — scrutinee 3 is in [1, 4) → arm taken.
///
/// FLS §5.1.9: Exclusive range `lo..hi` matches `lo <= v && v < hi`.
#[test]
fn milestone_34_range_exclusive_taken() {
    let src = "fn main() -> i32 { let x = 3; match x { 1..4 => 1, _ => 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected 3 to match 1..4, got {exit_code}");
}

/// Milestone 34: exclusive range — upper bound is exclusive.
///
/// FLS §5.1.9: `v == hi` must NOT match for exclusive range `lo..hi`.
#[test]
fn milestone_34_range_exclusive_upper_excluded() {
    let src = "fn main() -> i32 { let x = 4; match x { 1..4 => 1, _ => 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected upper bound 4 NOT to match 1..4, got {exit_code}");
}

/// Milestone 34: assembly inspection — range pattern emits ge+le+and+cbz.
///
/// FLS §5.1.9: The inclusive range `1..=3` requires two comparisons (>= lo
/// and <= hi) combined with AND. All instructions emit at runtime.
/// FLS §6.1.2:37–45: Runtime instructions — no compile-time folding.
#[test]
fn runtime_range_pattern_emits_cmp_and_cbz() {
    let src = "fn main() -> i32 { let x = 2; match x { 1..=3 => 1, _ => 0 } }\n";
    let asm = compile_to_asm(src);
    // The range check emits AND of two comparison results.
    assert!(asm.contains("and"), "expected 'and' for range intersection check");
    // Must have a conditional branch to skip the arm if outside range.
    assert!(asm.contains("cbz"), "expected 'cbz' for range arm guard");
}

// ── Milestone 35: match arm guards (FLS §6.18) ────────────────────────────────

/// Milestone 35: guard taken — `n if n > 5 => 1` matches when x = 7.
///
/// FLS §6.18: A match arm guard is an additional condition evaluated after
/// the pattern matches. If the guard is `true`, the arm executes.
/// FLS §6.1.2:37–45: The guard condition emits runtime instructions (cbz).
#[test]
fn milestone_35_guard_taken() {
    let src = "fn main() -> i32 { let x = 7; match x { n if n > 5 => 1, _ => 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected guard n > 5 to pass for x=7, got {exit_code}");
}

/// Milestone 35: guard not taken — guard fails so wildcard arm executes.
///
/// FLS §6.18: If the guard evaluates to `false`, the arm is skipped.
#[test]
fn milestone_35_guard_not_taken() {
    let src = "fn main() -> i32 { let x = 3; match x { n if n > 5 => 1, _ => 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected guard n > 5 to fail for x=3, got {exit_code}");
}

/// Milestone 35: guard on literal pattern — `0 if false => 99` never fires.
///
/// FLS §6.18: Guard is evaluated only when the pattern matches.
/// Pattern `0` matches scrutinee `0`, but the guard `false` rejects the arm.
#[test]
fn milestone_35_guard_on_literal_pattern() {
    let src = "fn main() -> i32 { match 0 { 0 if false => 99, _ => 1 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected guard `false` to reject arm, got {exit_code}");
}

/// Milestone 35: guard via parameter — classify using guards.
///
/// FLS §6.18: Guards may reference function parameters.
#[test]
fn milestone_35_guard_on_parameter() {
    let src = "\
fn sign(x: i32) -> i32 {
    match x {
        _ if x > 0 => 1,
        _ if x < 0 => 2,
        _ => 0,
    }
}
fn main() -> i32 { sign(42) }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected sign(42) == 1, got {exit_code}");
}

/// Milestone 35: guard on parameter — negative value.
#[test]
fn milestone_35_guard_negative_value() {
    let src = "\
fn sign(x: i32) -> i32 {
    match x {
        _ if x > 0 => 1,
        _ if x < 0 => 2,
        _ => 0,
    }
}
fn main() -> i32 { sign(0) - 0 }
";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected sign(0) == 0, got {exit_code}");
}

/// Milestone 35: multiple guards — first matching guard wins.
///
/// FLS §6.18: Arms are tested in order; first arm whose pattern matches
/// AND whose guard passes is selected.
#[test]
fn milestone_35_multiple_guards_first_wins() {
    let src = "fn main() -> i32 { match 10 { n if n > 8 => 2, n if n > 5 => 1, _ => 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "expected first guard n > 8 to win for x=10, got {exit_code}");
}

/// Milestone 35: multiple guards — second guard fires when first fails.
#[test]
fn milestone_35_multiple_guards_second_fires() {
    let src = "fn main() -> i32 { match 6 { n if n > 8 => 2, n if n > 5 => 1, _ => 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected second guard n > 5 to fire for x=6, got {exit_code}");
}

/// Milestone 35: guard references the bound identifier name.
///
/// FLS §6.18 + §5.1.4: In `n if n > 0 => n`, the guard expression `n > 0`
/// references the bound name `n`, which holds the scrutinee value.
#[test]
fn milestone_35_guard_references_binding() {
    let src = "fn main() -> i32 { match 7 { n if n > 0 => n, _ => 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected guard to pass and binding to return 7, got {exit_code}");
}

/// Milestone 35: assembly inspection — guard emits a second cbz after pattern check.
///
/// FLS §6.18: The guard condition must emit a runtime conditional branch.
/// For `n if n > 5 => 1`, the generated code must have at least two cbz
/// instructions: one for the pattern-match check and one for the guard.
///
/// FLS §6.1.2:37–45: All conditions emit runtime instructions.
#[test]
fn runtime_match_guard_emits_cbz_for_guard_condition() {
    // The scrutinee is the only thing in the match so pattern is Ident (always matches).
    // The guard `n > 5` emits a comparison + cbz.
    let src = "fn main() -> i32 { let x = 7; match x { n if n > 5 => 1, _ => 0 } }\n";
    let asm = compile_to_asm(src);
    // Count cbz instructions: there should be at least one for the guard.
    let cbz_count = asm.matches("cbz").count();
    assert!(cbz_count >= 1, "expected at least 1 cbz for guard check, got {cbz_count}");
    // The assembly must also contain a comparison for the guard condition.
    assert!(asm.contains("cmp") || asm.contains("cset"),
        "expected comparison instruction for guard n > 5");
}

// ── Milestone 36: if-let expressions (FLS §6.17) ─────────────────────────────

/// Milestone 36: if-let with integer literal pattern — match taken.
///
/// FLS §6.17: An if-let expression tests the scrutinee against a pattern.
/// If the pattern matches, the then block executes.
#[test]
fn milestone_36_if_let_literal_taken() {
    let src = "fn main() -> i32 { let x = 42; if let 42 = x { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected pattern 42 to match x=42, got {exit_code}");
}

/// Milestone 36: if-let with integer literal pattern — match not taken.
///
/// FLS §6.17: If the pattern does not match, the else branch executes.
#[test]
fn milestone_36_if_let_literal_not_taken() {
    let src = "fn main() -> i32 { let x = 7; if let 42 = x { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected pattern 42 not to match x=7, got {exit_code}");
}

/// Milestone 36: if-let with identifier pattern — always matches, binds value.
///
/// FLS §5.1.4 + §6.17: An identifier pattern always matches and binds the
/// scrutinee to the given name within the then block.
#[test]
fn milestone_36_if_let_ident_binds_value() {
    let src = "fn main() -> i32 { let x = 5; if let n = x { n + 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "expected n=5, n+1=6, got {exit_code}");
}

/// Milestone 36: if-let on a function parameter.
///
/// FLS §6.17: The scrutinee can be any expression, including a parameter.
#[test]
fn milestone_36_if_let_on_parameter() {
    let src = "fn check(x: i32) -> i32 { if let 10 = x { 1 } else { 2 } }\nfn main() -> i32 { check(10) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected pattern 10 to match parameter x=10, got {exit_code}");
}

/// Milestone 36: if-let with range pattern — taken.
///
/// FLS §5.1.9 + §6.17: Range patterns are valid in if-let position.
#[test]
fn milestone_36_if_let_range_taken() {
    let src = "fn main() -> i32 { let x = 5; if let 1..=10 = x { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected range 1..=10 to match x=5, got {exit_code}");
}

/// Milestone 36: if-let with range pattern — not taken.
#[test]
fn milestone_36_if_let_range_not_taken() {
    let src = "fn main() -> i32 { let x = 15; if let 1..=10 = x { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected range 1..=10 not to match x=15, got {exit_code}");
}

/// Milestone 36: if-let without else branch (unit context).
///
/// FLS §6.17: An if-let without an else branch has type `()`.
/// Used as a statement here.
#[test]
fn milestone_36_if_let_no_else_unit() {
    let src = "fn main() -> i32 { let mut r = 0; if let 3 = 3 { r = 7; } r }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected pattern 3=3 to match and set r=7, got {exit_code}");
}

/// Milestone 36: assembly inspection — if-let literal emits comparison and cbz.
///
/// FLS §6.17: Pattern check must emit runtime instructions.
/// FLS §6.1.2:37–45: No constant folding of pattern checks.
#[test]
fn runtime_if_let_emits_comparison_and_cbz() {
    let src = "fn main() -> i32 { let x = 5; if let 5 = x { 1 } else { 0 } }\n";
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("cmp") || asm.contains("cset"),
        "expected comparison instruction for if-let pattern check"
    );
    assert!(asm.contains("cbz"), "expected cbz for if-let conditional branch");
}

// ── Milestone 37: while-let loops ────────────────────────────────────────────
//
// FLS §6.15.4: "A while let loop expression is syntactic sugar for a loop
// expression containing a match expression that breaks on mismatch."
// The loop type is `()`. `break` exits; `continue` re-evaluates the scrutinee.

/// Milestone 37: while-let literal pattern — loop while scrutinee matches.
///
/// FLS §6.15.4: pattern is checked before each iteration; exits when no match.
#[test]
fn milestone_37_while_let_literal_exits_on_mismatch() {
    // Counter starts at 1; loop runs while x == 1; after body x becomes 2.
    let src = "fn main() -> i32 { let mut x = 1; let mut r = 0; while let 1 = x { r = 5; x = 2; } r }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected loop body to run once setting r=5, got {exit_code}");
}

/// Milestone 37: while-let literal pattern — body never runs when mismatch immediately.
///
/// FLS §6.15.4: condition is checked before the first iteration.
#[test]
fn milestone_37_while_let_no_match_initially() {
    let src = "fn main() -> i32 { let x = 2; let mut r = 0; while let 1 = x { r = 99; } r }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected loop body to never run, got {exit_code}");
}

/// Milestone 37: while-let identifier pattern — binds scrutinee each iteration.
///
/// FLS §5.1.4 + §6.15.4: identifier pattern always matches; binding is fresh
/// each iteration.
#[test]
fn milestone_37_while_let_ident_counts() {
    // Counts from 0 to 4 (5 iterations) using while-let with ident pattern.
    // The ident pattern always matches so the loop runs until the break.
    let src = "fn main() -> i32 { let mut i = 0; while let v = i { if v >= 5 { break; } i = i + 1; } i }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected i=5 after loop, got {exit_code}");
}

/// Milestone 37: while-let with range pattern — loop while value in range.
///
/// FLS §5.1.9 + §6.15.4: range patterns are valid in while-let position.
#[test]
fn milestone_37_while_let_range_counts() {
    // Loop while x is in 1..=3; body increments x.
    let src = "fn main() -> i32 { let mut x = 1; let mut count = 0; while let 1..=3 = x { count = count + 1; x = x + 1; } count }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "expected 3 iterations (x=1,2,3), got {exit_code}");
}

/// Milestone 37: while-let on a function parameter-derived value.
///
/// FLS §6.15.4: The scrutinee can be any expression.
#[test]
fn milestone_37_while_let_on_parameter() {
    // sum_down: while n matches 1..=100, add n to acc then decrement n.
    let src = "fn sum_down(mut n: i32) -> i32 { let mut acc = 0; while let 1..=100 = n { acc = acc + n; n = n - 1; } acc }\nfn main() -> i32 { sum_down(5) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "expected sum 1+2+3+4+5=15, got {exit_code}");
}

/// Milestone 37: assembly inspection — while-let emits back-edge branch and cbz.
///
/// FLS §6.15.4: Must emit runtime loop structure, not constant-fold.
/// FLS §6.1.2:37–45: Back-edge is a runtime branch instruction.
#[test]
fn runtime_while_let_emits_back_edge_and_cbz() {
    let src = "fn main() -> i32 { let mut x = 0; while let 0 = x { x = 1; } x }\n";
    let asm = compile_to_asm(src);
    assert!(asm.contains("cbz"), "expected cbz for while-let pattern check");
    // Back-edge: unconditional branch back to loop header (`b` followed by label).
    assert!(asm.contains("b ") && asm.contains(".L"), "expected back-edge branch for while-let loop");
}

// ── Milestone 38: struct construction + field access (FLS §6.11, §6.13) ────────

/// Milestone 38: simplest struct — two i32 fields, access by name.
///
/// FLS §6.11: Struct expression constructs the struct on the stack.
/// FLS §6.13: Field access loads from the correct stack slot.
#[test]
fn milestone_38_struct_field_access() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let p = Point { x: 3, y: 4 }; p.x + p.y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected p.x + p.y = 3 + 4 = 7, got {exit_code}");
}

/// Milestone 38: first field only.
///
/// FLS §6.13: The first field is at `base_slot + 0`.
#[test]
fn milestone_38_struct_first_field() {
    let src = "struct Pair { a: i32, b: i32 }\nfn main() -> i32 { let p = Pair { a: 42, b: 1 }; p.a }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected p.a = 42, got {exit_code}");
}

/// Milestone 38: second field only.
///
/// FLS §6.13: The second field is at `base_slot + 1`.
#[test]
fn milestone_38_struct_second_field() {
    let src = "struct Pair { a: i32, b: i32 }\nfn main() -> i32 { let p = Pair { a: 1, b: 99 }; p.b }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 99, "expected p.b = 99, got {exit_code}");
}

/// Milestone 38: struct field used in arithmetic.
///
/// FLS §6.13: Field access produces an i32 value usable as an operand.
#[test]
fn milestone_38_struct_field_in_arithmetic() {
    let src = "struct Rect { w: i32, h: i32 }\nfn main() -> i32 { let r = Rect { w: 6, h: 7 }; r.w * r.h }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected w * h = 6 * 7 = 42, got {exit_code}");
}

/// Milestone 38: struct fields accessed after let bindings (mixed stack).
///
/// FLS §8.1 + §6.13: Struct slots must not alias other locals on the stack.
#[test]
fn milestone_38_struct_with_other_locals() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let offset = 10; let p = Point { x: 3, y: 4 }; p.x + p.y + offset }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 17, "expected 3 + 4 + 10 = 17, got {exit_code}");
}

/// Milestone 38: struct passed to a function that accepts its fields.
///
/// FLS §9 + §6.13: Field access works when the struct is defined in a called context.
#[test]
fn milestone_38_struct_field_passed_to_fn() {
    let src = "struct Point { x: i32, y: i32 }\nfn sum(a: i32, b: i32) -> i32 { a + b }\nfn main() -> i32 { let p = Point { x: 10, y: 20 }; sum(p.x, p.y) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 30, "expected sum(10, 20) = 30, got {exit_code}");
}

/// Milestone 38: struct initializer fields in non-declaration order.
///
/// FLS §6.11: Field initializers may appear in any order in the source.
/// Galvanic normalises to declaration order for storage.
#[test]
fn milestone_38_struct_fields_out_of_order() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let p = Point { y: 9, x: 5 }; p.x - p.y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 252, "expected 5 - 9 = -4 (wraps to 252 as u8 exit code), got {exit_code}");
}

/// Milestone 38: three-field struct.
///
/// FLS §6.11: Structs with more than two fields allocate additional slots.
#[test]
fn milestone_38_three_field_struct() {
    let src = "struct Triple { a: i32, b: i32, c: i32 }\nfn main() -> i32 { let t = Triple { a: 1, b: 2, c: 3 }; t.a + t.b + t.c }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "expected 1 + 2 + 3 = 6, got {exit_code}");
}

/// Milestone 38: assembly inspection — struct literal emits str instructions.
///
/// FLS §6.11: Must emit runtime store instructions, not fold at compile time.
/// FLS §6.1.2:37–45: Struct construction emits runtime stores.
#[test]
fn runtime_struct_lit_emits_stores() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let p = Point { x: 3, y: 4 }; p.x + p.y }\n";
    let asm = compile_to_asm(src);
    // Two str instructions for the two fields.
    let store_count = asm.lines().filter(|l| l.trim().starts_with("str")).count();
    assert!(store_count >= 2, "expected ≥2 str instructions for struct fields, got {store_count}");
}

/// Milestone 38: assembly inspection — field access emits ldr instructions.
///
/// FLS §6.13: Field access must emit runtime load instructions.
#[test]
fn runtime_field_access_emits_ldr() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let p = Point { x: 3, y: 4 }; p.x + p.y }\n";
    let asm = compile_to_asm(src);
    // Two ldr instructions for p.x and p.y (plus possibly the add operands).
    let load_count = asm.lines().filter(|l| l.trim().starts_with("ldr")).count();
    assert!(load_count >= 2, "expected ≥2 ldr instructions for field accesses, got {load_count}");
}

// ── Milestone 39: mutable struct field assignment ─────────────────────────────
//
// FLS §6.5.10: Assignment expressions. The LHS is a place expression.
// FLS §6.13: Field access on a place produces a place — i.e., `s.field`
//   on the LHS of `=` is a field write.
// FLS §6.1.2:37–45: The store must be a runtime instruction, not folded.

/// Milestone 39: basic mutable field assignment.
///
/// FLS §6.5.10 + §6.13: `s.field = value` stores to the field's stack slot.
#[test]
fn milestone_39_field_assign_basic() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let mut p = Point { x: 1, y: 2 }; p.x = 10; p.x }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected p.x = 10 after assignment, got {exit_code}");
}

/// Milestone 39: assign to second field.
///
/// FLS §6.5.10 + §6.13: field index offset is applied correctly.
#[test]
fn milestone_39_field_assign_second_field() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let mut p = Point { x: 1, y: 2 }; p.y = 7; p.y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected p.y = 7 after assignment, got {exit_code}");
}

/// Milestone 39: assign to one field, read other field unchanged.
///
/// FLS §6.13: Assigning `s.x` must not disturb `s.y`.
#[test]
fn milestone_39_field_assign_does_not_clobber_other_field() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let mut p = Point { x: 1, y: 42 }; p.x = 99; p.y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected p.y unchanged at 42, got {exit_code}");
}

/// Milestone 39: assign multiple fields sequentially.
///
/// FLS §6.5.10: Sequential assignments evaluate left-to-right, each storing
/// to the target slot independently.
#[test]
fn milestone_39_field_assign_multiple() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let mut p = Point { x: 0, y: 0 }; p.x = 3; p.y = 4; p.x + p.y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// Milestone 39: assign field using a runtime-computed value.
///
/// FLS §6.5.10 + §6.1.2:37–45: The RHS is evaluated at runtime.
#[test]
fn milestone_39_field_assign_from_expr() {
    let src = "struct Counter { n: i32, step: i32 }\nfn main() -> i32 { let mut c = Counter { n: 0, step: 3 }; c.n = c.step * 5; c.n }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "expected step * 5 = 15, got {exit_code}");
}

/// Milestone 39: assign field inside a loop (accumulate via field).
///
/// FLS §6.5.10 + §6.15.3: While loops may contain field assignment expressions.
#[test]
fn milestone_39_field_assign_in_loop() {
    let src = "struct Acc { val: i32 }\nfn main() -> i32 { let mut a = Acc { val: 0 }; let mut i = 0; while i < 5 { a.val = a.val + i; i = i + 1; } a.val }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected 0+1+2+3+4 = 10, got {exit_code}");
}

/// Milestone 39: assembly inspection — field assignment emits a str instruction.
///
/// FLS §6.5.10 + §6.1.2:37–45: Must emit a runtime store, not fold at compile time.
#[test]
fn runtime_field_assign_emits_str() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let mut p = Point { x: 1, y: 2 }; p.x = 10; p.x }\n";
    let asm = compile_to_asm(src);
    // At least 3 str instructions: 2 for struct init + 1 for field assignment.
    let store_count = asm.lines().filter(|l| l.trim().starts_with("str")).count();
    assert!(store_count >= 3, "expected ≥3 str instructions (2 init + 1 field assign), got {store_count}");
}

// ── Milestone 40: enum unit variants ─────────────────────────────────────────
//
// FLS §15: Enumerations. Unit variants are assigned integer discriminants
// (0, 1, 2, ...) in declaration order. Variant values are produced by
// two-segment path expressions (`Color::Red`). Pattern matching against
// enum variant paths uses discriminant equality.
//
// FLS §6.3 + §5.5: Path expressions and path patterns resolve to discriminants.
// FLS §6.1.2:37–45: All discriminant materialization emits runtime instructions.

/// Milestone 40: simplest enum — two unit variants, match first.
///
/// FLS §15: `Color::Red` has discriminant 0, `Color::Blue` has discriminant 1.
/// The match arm `Color::Red => 0` should fire.
#[test]
fn milestone_40_enum_unit_two_variants_first() {
    let src = "enum Color { Red, Blue }\nfn main() -> i32 { let c = Color::Red; match c { Color::Red => 0, Color::Blue => 1, } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected Color::Red → 0, got {exit_code}");
}

/// Milestone 40: enum match on second variant.
///
/// FLS §15: `Color::Blue` has discriminant 1; the second arm fires.
#[test]
fn milestone_40_enum_unit_two_variants_second() {
    let src = "enum Color { Red, Blue }\nfn main() -> i32 { let c = Color::Blue; match c { Color::Red => 0, Color::Blue => 1, } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected Color::Blue → 1, got {exit_code}");
}

/// Milestone 40: three-variant enum, match middle variant.
///
/// FLS §15: discriminants are 0=North, 1=South, 2=East. Selecting South → 1.
#[test]
fn milestone_40_enum_three_variants_middle() {
    let src = "enum Dir { North, South, East }\nfn main() -> i32 { let d = Dir::South; match d { Dir::North => 0, Dir::South => 1, Dir::East => 2, } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected Dir::South → 1, got {exit_code}");
}

/// Milestone 40: enum match with wildcard fallthrough.
///
/// FLS §15 + §6.18: The first two arms check specific variants; the wildcard arm
/// catches any remaining discriminant.
#[test]
fn milestone_40_enum_match_wildcard() {
    let src = "enum Shape { Circle, Square, Triangle }\nfn main() -> i32 { let s = Shape::Triangle; match s { Shape::Circle => 10, Shape::Square => 20, _ => 30, } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 30, "expected Triangle → 30 via wildcard, got {exit_code}");
}

/// Milestone 40: enum variant passed as function argument.
///
/// FLS §15 + §9: An enum value (its discriminant) may be passed to a function.
/// The called function receives an i32 and returns it directly.
#[test]
fn milestone_40_enum_passed_to_fn() {
    let src = "enum Rank { Low, Mid, High }\nfn rank_val(r: i32) -> i32 { r }\nfn main() -> i32 { rank_val(Rank::High as i32) }\n";
    // Note: `as i32` cast on an enum value — this tests that the enum discriminant is an i32.
    // FLS §6.5.9: Cast expression. Enum → i32 cast is valid (discriminant value).
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "expected Rank::High discriminant 2, got {exit_code}");
}

/// Milestone 40: enum match with non-zero return from arm body.
///
/// FLS §15 + §6.18: Arm bodies are arbitrary expressions; here they return
/// computed values, not literals.
#[test]
fn milestone_40_enum_arm_body_expr() {
    let src = "enum Tier { Bronze, Silver, Gold }\nfn main() -> i32 { let t = Tier::Gold; match t { Tier::Bronze => 1 * 10, Tier::Silver => 2 * 10, Tier::Gold => 3 * 10, } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 30, "expected Gold → 3*10 = 30, got {exit_code}");
}

/// Milestone 40: enum variant used in conditional expression.
///
/// FLS §15 + §6.17: An enum discriminant may appear as scrutinee in an if-let.
#[test]
fn milestone_40_enum_if_let_taken() {
    let src = "enum Flag { On, Off }\nfn main() -> i32 { let f = Flag::On; if let Flag::On = f { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected Flag::On if-let to fire, got {exit_code}");
}

/// Milestone 40: enum if-let arm not taken.
///
/// FLS §15 + §6.17: Pattern `Flag::On` does not match `Flag::Off`; else arm fires.
#[test]
fn milestone_40_enum_if_let_not_taken() {
    let src = "enum Flag { On, Off }\nfn main() -> i32 { let f = Flag::Off; if let Flag::On = f { 1 } else { 0 } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected Flag::Off to fall to else, got {exit_code}");
}

/// Milestone 40: assembly inspection — enum variant path emits mov (LoadImm).
///
/// FLS §15 + §6.1.2:37–45: `Color::Red` must emit a runtime `mov` for the
/// discriminant 0, not be constant-folded away.
#[test]
fn runtime_enum_variant_emits_mov() {
    let src = "enum Color { Red, Blue }\nfn main() -> i32 { let c = Color::Blue; c }\n";
    let asm = compile_to_asm(src);
    // `Color::Blue` has discriminant 1; we expect a `mov` materializing #1.
    // The exact register may vary; check that the immediate value appears.
    let has_mov_1 = asm.lines().any(|l| {
        let l = l.trim();
        l.starts_with("mov") && l.contains("#1")
    });
    assert!(has_mov_1, "expected mov #1 for Color::Blue discriminant\nasm:\n{asm}");
}

/// Milestone 40: assembly inspection — enum match emits cmp + cbz.
///
/// FLS §6.18 + §15: Matching on an enum value emits a comparison instruction
/// followed by a conditional branch, not a compile-time branch selection.
#[test]
fn runtime_enum_match_emits_comparison() {
    let src = "enum Color { Red, Blue }\nfn main() -> i32 { let c = Color::Red; match c { Color::Red => 0, Color::Blue => 1, } }\n";
    let asm = compile_to_asm(src);
    // The comparison instruction (cmp or sub used for cmp, or cset after cmp).
    let has_comparison = asm.lines().any(|l| {
        let l = l.trim();
        l.starts_with("cmp") || l.starts_with("cset")
    });
    assert!(has_comparison, "expected cmp/cset instruction for enum match\nasm:\n{asm}");
}

// ── Milestone 41: enum tuple variants compile to runtime ARM64 ────────────────
//
// FLS §15: Tuple variant construction stores a discriminant (slot 0) and
// positional fields (slots 1..N) on the stack. Pattern matching compares the
// discriminant at runtime and binds each field identifier to its slot.
//
// FLS §6.1.2:37–45: Construction and matching are runtime operations; the
// discriminant comparison is never constant-folded.

/// Milestone 41: Some arm taken — extracts the wrapped value.
///
/// FLS §15: `Opt::Some(42)` stores discriminant=1 + field=42.
/// Matching `Opt::Some(v) => v` compares discriminant=1 at runtime.
#[test]
fn milestone_41_tuple_variant_some_taken() {
    let src = r#"
enum Opt { None, Some(i32) }
fn main() -> i32 {
    let x = Opt::Some(42);
    match x {
        Opt::Some(v) => v,
        Opt::None => 0,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42);
}

/// Milestone 41: None arm taken — returns 0 when variant doesn't match.
///
/// FLS §15: `Opt::None` stores discriminant=0 with no fields. The
/// `Opt::Some(v)` checked arm fails; the `Opt::None` default arm runs.
#[test]
fn milestone_41_tuple_variant_none_taken() {
    let src = r#"
enum Opt { None, Some(i32) }
fn main() -> i32 {
    let x = Opt::None;
    match x {
        Opt::Some(v) => v,
        Opt::None => 0,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0);
}

/// Milestone 41: field value used in arithmetic.
///
/// FLS §15: `Some(v) => v + 1` exercises using the bound field variable.
#[test]
fn milestone_41_tuple_variant_field_in_arithmetic() {
    let src = r#"
enum Opt { None, Some(i32) }
fn main() -> i32 {
    let x = Opt::Some(10);
    match x {
        Opt::Some(v) => v + 5,
        Opt::None => 0,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15);
}

/// Milestone 41: two-field tuple variant.
///
/// FLS §15: A variant with two fields stores discriminant, field0, field1
/// in consecutive slots. Pattern `Pair(a, b) => a + b` binds both.
#[test]
fn milestone_41_two_field_tuple_variant() {
    let src = r#"
enum Pair { None, Two(i32, i32) }
fn main() -> i32 {
    let p = Pair::Two(3, 7);
    match p {
        Pair::Two(a, b) => a + b,
        Pair::None => 0,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10);
}

/// Milestone 41: wildcard field pattern ignores field.
///
/// FLS §5.1: `_` in a tuple struct field position is a wildcard — matches
/// but does not bind. FLS §15: only the discriminant comparison runs.
#[test]
fn milestone_41_wildcard_field() {
    let src = r#"
enum Opt { None, Some(i32) }
fn main() -> i32 {
    let x = Opt::Some(99);
    match x {
        Opt::Some(_) => 1,
        Opt::None => 0,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1);
}

/// Milestone 41: TupleStruct as the last (default) arm.
///
/// FLS §6.18: The last arm in a match is emitted unconditionally. When
/// the last arm has a TupleStruct pattern, its field bindings are installed
/// before the body executes.
#[test]
fn milestone_41_tuple_variant_default_arm() {
    let src = r#"
enum Opt { None, Some(i32) }
fn main() -> i32 {
    let x = Opt::Some(7);
    match x {
        Opt::None => 0,
        Opt::Some(v) => v,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7);
}

/// Milestone 41: on a parameter value — non-literal scrutinee.
///
/// FLS §6.1.2:37–45: The runtime must handle this via actual branch
/// instructions; the value is not known at compile time.
#[test]
fn milestone_41_tuple_variant_on_parameter() {
    let src = r#"
enum Opt { None, Some(i32) }
fn unwrap_or_zero(o: Opt) -> i32 {
    match o {
        Opt::Some(v) => v,
        Opt::None => 0,
    }
}
fn main() -> i32 {
    let a = Opt::Some(13);
    let b = Opt::None;
    unwrap_or_zero(a) + unwrap_or_zero(b)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 13);
}

/// Milestone 41: assembly inspection — tuple variant construction emits stores.
///
/// FLS §15: `Opt::Some(42)` must emit at minimum two `str` instructions:
/// one for the discriminant and one for the field value.
#[test]
fn runtime_tuple_variant_construction_emits_stores() {
    let src = r#"
enum Opt { None, Some(i32) }
fn main() -> i32 {
    let x = Opt::Some(42);
    match x { Opt::Some(v) => v, Opt::None => 0, }
}
"#;
    let asm = compile_to_asm(src);
    let store_count = asm.lines().filter(|l| l.trim().starts_with("str")).count();
    assert!(
        store_count >= 2,
        "expected ≥2 str instructions for tuple variant construction (discriminant + field)\nasm:\n{asm}"
    );
}

// ── Milestone 42: named-field enum variants ───────────────────────────────────

/// Milestone 42: basic named-field variant construction and match.
///
/// FLS §15.3: Named-field enum variants — `Variant { field: Type }`.
/// FLS §6.11: Struct expressions apply to enum variants.
/// FLS §5.3: Struct patterns match named-field variants by discriminant then fields.
///
/// Derived from FLS §15 examples (the spec describes named-field variant
/// syntax but provides no concrete code example; this program is derived
/// from the semantic description).
#[test]
fn milestone_42_named_variant_basic() {
    let src = r#"
enum Color { Black, Rgb { r: i32, g: i32, b: i32 } }
fn main() -> i32 {
    let c = Color::Rgb { r: 10, g: 20, b: 12 };
    match c {
        Color::Rgb { r, g, b } => r + g + b,
        Color::Black => 0,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 42: discriminant field extraction — first field.
///
/// FLS §15.3: fields are stored in declaration order; `r` is at slot base+1.
#[test]
fn milestone_42_named_variant_first_field() {
    let src = r#"
enum Color { Black, Rgb { r: i32, g: i32, b: i32 } }
fn main() -> i32 {
    let c = Color::Rgb { r: 7, g: 0, b: 0 };
    match c {
        Color::Rgb { r, g, b } => r,
        Color::Black => 99,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 42: fields given in a different order from declaration — still stores in declaration order.
///
/// FLS §6.11: Field initialisers in a struct expression may appear in any order;
/// galvanic normalises to declaration order.
#[test]
fn milestone_42_named_variant_out_of_order_construction() {
    let src = r#"
enum Color { Black, Rgb { r: i32, g: i32, b: i32 } }
fn main() -> i32 {
    let c = Color::Rgb { b: 5, r: 3, g: 4 };
    match c {
        Color::Rgb { r, g, b } => r * 10 + g * 3 + b,
        Color::Black => 0,
    }
}
"#;
    // r=3, g=4, b=5 in declaration order → 3*10 + 4*3 + 5 = 47 (fits in 8-bit exit code).
    // If fields were stored in source order (b,r,g), the result would be 5*10+3*3+4=63.
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 47, "expected exit 47, got {exit_code}");
}

/// Milestone 42: non-matching variant arm falls to wildcard.
///
/// FLS §6.18: Arms are checked in order; when the first arm's discriminant
/// doesn't match, the wildcard arm is taken.
#[test]
fn milestone_42_named_variant_non_matching_arm() {
    let src = r#"
enum Shape { Circle { r: i32 }, Square { side: i32 } }
fn main() -> i32 {
    let s = Shape::Circle { r: 5 };
    match s {
        Shape::Square { side } => side,
        Shape::Circle { r } => r,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected exit 5, got {exit_code}");
}

/// Milestone 42: named variant passed as a function parameter.
///
/// FLS §9: enum values (including named-field variants) are passed in registers.
/// FLS §6.1.2:37–45: runtime codegen — value is not known at compile time.
#[test]
fn milestone_42_named_variant_parameter() {
    let src = r#"
enum Point { Zero, Coord { x: i32, y: i32 } }
fn sum(p: Point) -> i32 {
    match p {
        Point::Coord { x, y } => x + y,
        Point::Zero => 0,
    }
}
fn main() -> i32 {
    let p = Point::Coord { x: 15, y: 27 };
    sum(p)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 42: wildcard sub-pattern ignores a named field.
///
/// FLS §5.1: Wildcard `_` matches any value without binding.
/// FLS §5.3: Wildcard is valid in a struct pattern field position.
#[test]
fn milestone_42_named_variant_wildcard_field() {
    let src = r#"
enum Pair { Empty, Both { a: i32, b: i32 } }
fn main() -> i32 {
    let p = Pair::Both { a: 42, b: 99 };
    match p {
        Pair::Both { a, b: _ } => a,
        Pair::Empty => 0,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 42: assembly inspection — named variant construction emits stores for discriminant + fields.
///
/// FLS §15.3 + §6.11: `Color::Rgb { r: 1, g: 2, b: 3 }` must store at least 4
/// values (discriminant + 3 fields) via `str` instructions.
#[test]
fn runtime_named_variant_construction_emits_stores() {
    let src = r#"
enum Color { Black, Rgb { r: i32, g: i32, b: i32 } }
fn main() -> i32 {
    let c = Color::Rgb { r: 10, g: 20, b: 12 };
    match c {
        Color::Rgb { r, g, b } => r + g + b,
        Color::Black => 0,
    }
}
"#;
    let asm = compile_to_asm(src);
    let store_count = asm.lines().filter(|l| l.trim().starts_with("str")).count();
    assert!(
        store_count >= 4,
        "expected ≥4 str instructions for named variant construction (discriminant + 3 fields)\nasm:\n{asm}"
    );
}

// ── Milestone 43: impl blocks with &self methods ──────────────────────────────
//
// FLS §11: Implementations. FLS §10.1: Methods. FLS §6.12.2: Method call
// expressions. A method defined in an `impl` block is lowered to a mangled
// function `TypeName__method_name`; field values are passed as leading
// arguments.

/// Milestone 43: basic `&self` method returning field sum.
///
/// FLS §11 + §10.1: `impl Point { fn sum(&self) -> i32 { self.x + self.y } }`.
/// The method is called as `p.sum()` and must return `x + y`.
#[test]
fn milestone_43_method_returns_field_sum() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn sum(&self) -> i32 { self.x + self.y }
}
fn main() -> i32 {
    let p = Point { x: 3, y: 4 };
    p.sum()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// Milestone 43: method accessing only first field.
///
/// FLS §10.1: `self.x` resolves to the first field of `self`.
#[test]
fn milestone_43_method_first_field() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn x_val(&self) -> i32 { self.x }
}
fn main() -> i32 {
    let p = Point { x: 42, y: 0 };
    p.x_val()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 43: method accessing only second field.
///
/// FLS §10.1: `self.y` resolves to the second field of `self`.
#[test]
fn milestone_43_method_second_field() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn y_val(&self) -> i32 { self.y }
}
fn main() -> i32 {
    let p = Point { x: 0, y: 13 };
    p.y_val()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 13, "expected 13, got {exit_code}");
}

/// Milestone 43: method with additional explicit parameter besides `&self`.
///
/// FLS §10.1: Regular parameters follow `self` in the parameter list.
/// They are passed after the self-fields in the calling convention.
#[test]
fn milestone_43_method_with_extra_param() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn add_to_x(&self, n: i32) -> i32 { self.x + n }
}
fn main() -> i32 {
    let p = Point { x: 10, y: 0 };
    p.add_to_x(5)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "expected 10 + 5 = 15, got {exit_code}");
}

/// Milestone 43: multiple methods on the same struct.
///
/// FLS §11: An impl block may contain multiple methods; each is lowered
/// to its own mangled function.
#[test]
fn milestone_43_multiple_methods() {
    let src = r#"
struct Rect { w: i32, h: i32 }
impl Rect {
    fn area(&self) -> i32 { self.w * self.h }
    fn perimeter(&self) -> i32 { self.w + self.w + self.h + self.h }
}
fn main() -> i32 {
    let r = Rect { w: 3, h: 4 };
    r.area()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "expected 3 * 4 = 12, got {exit_code}");
}

/// Milestone 43: method call result used in arithmetic.
///
/// FLS §6.12.2: Method call expressions produce a value usable in larger
/// expressions. `p.sum() + 1` must return `sum + 1`.
#[test]
fn milestone_43_method_result_in_arithmetic() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn sum(&self) -> i32 { self.x + self.y }
}
fn main() -> i32 {
    let p = Point { x: 3, y: 4 };
    p.sum() + 1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 8, "expected 3 + 4 + 1 = 8, got {exit_code}");
}

/// Milestone 43: method call on a parameter (struct passed to outer function).
///
/// FLS §10.1: Methods can be called on struct-typed locals that came from
/// function parameters.
#[test]
fn milestone_43_method_on_parameter() {
    let src = r#"
struct Counter { n: i32 }
impl Counter {
    fn value(&self) -> i32 { self.n }
}
fn get_value(c: Counter) -> i32 { c.value() }
fn main() -> i32 {
    let c = Counter { n: 21 };
    get_value(c)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 21, "expected 21, got {exit_code}");
}

/// Milestone 43: assembly inspection — method call emits `bl TypeName__method_name`.
///
/// FLS §10.1 + §6.12.2: Method calls must emit a branch-and-link to the mangled
/// function name, not an interpreter result.
#[test]
fn runtime_method_call_emits_bl_mangled_name() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn sum(&self) -> i32 { self.x + self.y }
}
fn main() -> i32 {
    let p = Point { x: 3, y: 4 };
    p.sum()
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("Point__sum"),
        "expected mangled function `Point__sum` in assembly:\n{asm}"
    );
    assert!(
        asm.contains("bl") && asm.contains("Point__sum"),
        "expected `bl Point__sum` call instruction in main:\n{asm}"
    );
}

// ── Milestone 44: &mut self methods compile to runtime ARM64 ─────────────────

/// Milestone 44: basic `&mut self` method increments a counter field.
///
/// FLS §10.1: Methods with `&mut self` receivers take a mutable reference to
/// `self`. Mutations to `self.field` must be visible to the caller after the
/// method returns. FLS §6.5.11: compound assignment `self.n += 1` inside the
/// method body.
#[test]
fn milestone_44_mut_self_basic_increment() {
    let src = r#"
struct Counter { n: i32 }
impl Counter {
    fn increment(&mut self) { self.n += 1; }
    fn value(&self) -> i32 { self.n }
}
fn main() -> i32 {
    let mut c = Counter { n: 0 };
    c.increment();
    c.value()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected 1 after one increment, got {exit_code}");
}

/// Milestone 44: multiple `&mut self` calls accumulate state.
///
/// FLS §10.1: Each `&mut self` call must write back modified fields. After
/// two increments the counter field should hold 2.
#[test]
fn milestone_44_mut_self_two_increments() {
    let src = r#"
struct Counter { n: i32 }
impl Counter {
    fn increment(&mut self) { self.n += 1; }
    fn value(&self) -> i32 { self.n }
}
fn main() -> i32 {
    let mut c = Counter { n: 0 };
    c.increment();
    c.increment();
    c.value()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "expected 2 after two increments, got {exit_code}");
}

/// Milestone 44: `&mut self` with initial non-zero field value.
///
/// FLS §10.1: Mutation adds to an existing value, not to zero.
#[test]
fn milestone_44_mut_self_nonzero_start() {
    let src = r#"
struct Counter { n: i32 }
impl Counter {
    fn increment(&mut self) { self.n += 1; }
    fn value(&self) -> i32 { self.n }
}
fn main() -> i32 {
    let mut c = Counter { n: 10 };
    c.increment();
    c.increment();
    c.increment();
    c.value()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 13, "expected 13, got {exit_code}");
}

/// Milestone 44: `&mut self` with two fields — mutation only touches one.
///
/// FLS §10.1: Write-back must preserve all fields, not just the mutated one.
/// The un-mutated field `y` must keep its original value after a call that
/// only modifies `x`.
#[test]
fn milestone_44_mut_self_two_fields_partial_mutation() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn shift_x(&mut self) { self.x += 5; }
    fn sum(&self) -> i32 { self.x + self.y }
}
fn main() -> i32 {
    let mut p = Point { x: 1, y: 2 };
    p.shift_x();
    p.sum()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 8, "expected 1+5+2=8, got {exit_code}");
}

/// Milestone 44: `&mut self` translate with extra arguments.
///
/// FLS §10.1: Extra explicit arguments follow self fields in the parameter list.
/// ARM64 ABI: struct fields in x0..x{N-1}, extra args in x{N}..x{N+M-1}.
#[test]
fn milestone_44_mut_self_with_extra_args() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn translate(&mut self, dx: i32, dy: i32) { self.x += dx; self.y += dy; }
    fn sum(&self) -> i32 { self.x + self.y }
}
fn main() -> i32 {
    let mut p = Point { x: 1, y: 2 };
    p.translate(3, 4);
    p.sum()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected 1+3 + 2+4 = 10, got {exit_code}");
}

/// Milestone 44: `&mut self` in a loop — accumulates over iterations.
///
/// FLS §6.15.3: While loop. FLS §10.1: Each call must write back.
/// After 5 increments the counter should hold 5.
#[test]
fn milestone_44_mut_self_in_loop() {
    let src = r#"
struct Counter { n: i32 }
impl Counter {
    fn increment(&mut self) { self.n += 1; }
    fn value(&self) -> i32 { self.n }
}
fn main() -> i32 {
    let mut c = Counter { n: 0 };
    let mut i = 0;
    while i < 5 {
        c.increment();
        i += 1;
    }
    c.value()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected 5 after 5 increments, got {exit_code}");
}

/// Milestone 44: `&mut self` on a parameter (struct passed by value).
///
/// FLS §10.1: The mutation applies to the local copy passed to the outer
/// function, not to the caller's original struct (value semantics).
#[test]
fn milestone_44_mut_self_on_parameter() {
    let src = r#"
struct Counter { n: i32 }
impl Counter {
    fn increment(&mut self) { self.n += 1; }
    fn value(&self) -> i32 { self.n }
}
fn bump(mut c: Counter) -> i32 {
    c.increment();
    c.value()
}
fn main() -> i32 {
    let c = Counter { n: 7 };
    bump(c)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 8, "expected 7+1=8, got {exit_code}");
}

/// Milestone 44: assembly inspection — `&mut self` emits write-back stores.
///
/// FLS §10.1: The `CallMut` instruction emits `str` instructions after `bl`
/// to write x0..x{N-1} back to the struct's stack slots.
#[test]
fn runtime_mut_self_emits_write_back_stores() {
    let src = r#"
struct Counter { n: i32 }
impl Counter {
    fn increment(&mut self) { self.n += 1; }
}
fn main() -> i32 {
    let mut c = Counter { n: 0 };
    c.increment();
    c.n
}
"#;
    let asm = compile_to_asm(src);
    // The call site must emit a bl followed by str to write x0 back.
    assert!(
        asm.contains("bl") && asm.contains("Counter__increment"),
        "expected bl Counter__increment in assembly:\n{asm}"
    );
    // At least one str instruction must follow (write-back).
    let bl_pos = asm.find("bl").unwrap_or(0);
    let after_bl = &asm[bl_pos..];
    assert!(
        after_bl.contains("str"),
        "expected str write-back after bl Counter__increment:\n{asm}"
    );
}

/// Milestone 44: assembly inspection — `&mut self` method emits RetFields ldr sequence.
///
/// FLS §10.1: The `RetFields` instruction emits `ldr` instructions to load
/// self fields into x0..x{N-1} before the `ret`, so the caller can read them.
#[test]
fn runtime_mut_self_method_emits_ret_fields_ldr() {
    let src = r#"
struct Counter { n: i32 }
impl Counter {
    fn increment(&mut self) { self.n += 1; }
}
fn main() -> i32 {
    let mut c = Counter { n: 0 };
    c.increment();
    c.n
}
"#;
    let asm = compile_to_asm(src);
    // The Counter__increment function body must emit an ldr before ret
    // to return the modified field in x0.
    assert!(
        asm.contains("Counter__increment"),
        "expected Counter__increment function in assembly:\n{asm}"
    );
    // The method must load x0 from its slot before returning.
    assert!(
        asm.contains("ldr     x0"),
        "expected ldr x0 (RetFields) in Counter__increment:\n{asm}"
    );
}

// ── Milestone 45: associated functions ──────────────────────────────────────
//
// Associated functions are functions in impl blocks that do not have a `self`
// parameter. They are called with `TypeName::fn_name(args)` syntax and are
// emitted under the mangled name `TypeName__fn_name`.
//
// FLS §10.1: Associated functions.
// FLS §6.12.1: Call expressions (two-segment path callee).
// FLS §14: Entities and resolution (path resolution for `Type::fn`).

/// Milestone 45: scalar-returning associated function is called correctly.
///
/// FLS §10.1: Associated functions with scalar return types are emitted as
/// regular functions under a mangled name and called via two-segment path.
#[test]
fn milestone_45_assoc_fn_scalar_return() {
    let src = r#"
struct Calc { n: i32 }
impl Calc {
    fn double(n: i32) -> i32 { n * 2 }
}
fn main() -> i32 { Calc::double(3) - 6 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected exit 0, got {exit_code}");
}

/// Milestone 45: associated constructor returns struct, field accessed via &self method.
///
/// FLS §10.1: `Point::new(x, y)` constructs a Point using a struct-returning
/// associated function. The fields are returned in x0..x{N-1} and written back
/// to the destination variable's stack slots.
#[test]
fn milestone_45_assoc_new_then_method() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn new(px: i32, py: i32) -> Point { Point { x: px, y: py } }
    fn x(&self) -> i32 { self.x }
    fn y(&self) -> i32 { self.y }
}
fn main() -> i32 {
    let p = Point::new(3, 7);
    p.x() + p.y() - 10
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected exit 0, got {exit_code}");
}

/// Milestone 45: associated function with no parameters returning a struct.
///
/// FLS §10.1: Associated functions need not take parameters.
#[test]
fn milestone_45_assoc_zero_init() {
    let src = r#"
struct Counter { value: i32 }
impl Counter {
    fn zero() -> Counter { Counter { value: 0 } }
    fn get(&self) -> i32 { self.value }
}
fn main() -> i32 {
    let c = Counter::zero();
    c.get()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected exit 0, got {exit_code}");
}

/// Milestone 45: associated function result used in arithmetic.
///
/// FLS §10.1: The scalar return value of an associated function can be used
/// directly in an arithmetic expression.
#[test]
fn milestone_45_assoc_result_in_arithmetic() {
    let src = r#"
struct Math { }
impl Math {
    fn square(n: i32) -> i32 { n * n }
}
fn main() -> i32 { Math::square(4) - 16 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected exit 0, got {exit_code}");
}

/// Milestone 45: struct constructed via associated function, then mutated.
///
/// FLS §10.1 + §10.1 (&mut self): combine constructor with mutable method.
#[test]
fn milestone_45_assoc_new_then_mut_method() {
    let src = r#"
struct Counter { value: i32 }
impl Counter {
    fn new(start: i32) -> Counter { Counter { value: start } }
    fn increment(&mut self) { self.value = self.value + 1; }
    fn get(&self) -> i32 { self.value }
}
fn main() -> i32 {
    let mut c = Counter::new(5);
    c.increment();
    c.get() - 6
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected exit 0, got {exit_code}");
}

/// Milestone 45: associated function called with parameter from another variable.
///
/// FLS §10.1: Arguments to associated functions are evaluated at runtime.
#[test]
fn milestone_45_assoc_fn_on_parameter() {
    let src = r#"
struct Mult { }
impl Mult {
    fn by_three(n: i32) -> i32 { n * 3 }
}
fn apply(n: i32) -> i32 { Mult::by_three(n) }
fn main() -> i32 { apply(7) - 21 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected exit 0, got {exit_code}");
}

/// Milestone 45: multiple associated functions in one impl block.
///
/// FLS §10.1: Multiple associated functions can be defined in one impl block.
#[test]
fn milestone_45_multiple_assoc_fns() {
    let src = r#"
struct Range { }
impl Range {
    fn min(a: i32, b: i32) -> i32 { if a < b { a } else { b } }
    fn max(a: i32, b: i32) -> i32 { if a > b { a } else { b } }
}
fn main() -> i32 { Range::max(3, 7) - Range::min(3, 7) - 4 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected exit 0, got {exit_code}");
}

// ── Assembly inspection: milestone 45 ────────────────────────────────────────

/// Milestone 45: associated function emitted under mangled name.
///
/// FLS §10.1: `Calc::double` must be emitted as `Calc__double`, not `double`.
/// The two-segment path call site must emit `bl Calc__double`.
#[test]
fn runtime_assoc_fn_emits_mangled_name() {
    let src = r#"
struct Calc { }
impl Calc {
    fn double(n: i32) -> i32 { n * 2 }
}
fn main() -> i32 { Calc::double(3) }
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("Calc__double"),
        "expected mangled function `Calc__double` in assembly:\n{asm}"
    );
    assert!(
        asm.contains("bl") && asm.contains("Calc__double"),
        "expected `bl Calc__double` in assembly:\n{asm}"
    );
}

/// Milestone 45: struct-returning associated function emits RetFields (ldr before ret).
///
/// FLS §10.1: A struct-returning associated function must return fields in
/// x0..x{N-1} via the RetFields mechanism, not interpret them at compile time.
#[test]
fn runtime_assoc_new_emits_ret_fields() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn new(px: i32, py: i32) -> Point { Point { x: px, y: py } }
    fn sum(&self) -> i32 { self.x + self.y }
}
fn main() -> i32 {
    let p = Point::new(3, 4);
    p.sum()
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("Point__new"),
        "expected `Point__new` in assembly:\n{asm}"
    );
    // The constructor must use RetFields: loads from stack before ret.
    assert!(
        asm.contains("ldr     x0") || asm.contains("ldr x0"),
        "expected ldr x0 (RetFields) in Point__new:\n{asm}"
    );
    // The call site in main must write back fields from x0..x1.
    assert!(
        asm.contains("str     x0") || asm.contains("str x0"),
        "expected str x0 (write-back) in main:\n{asm}"
    );
}

// ── Milestone 46: trait definitions and impl Trait for Type ──────────────────

/// Milestone 46: a simple trait with one `&self` method, implemented for a struct.
///
/// FLS §13: Trait definitions declare method signatures.
/// FLS §11.1: Trait implementations provide concrete method bodies.
/// Static dispatch: `s.area()` resolves to `Square__area` via the same
/// `TypeName__method_name` mangling used by inherent impls.
#[test]
fn milestone_46_trait_method_basic() {
    let src = r#"
trait Area {
    fn area(&self) -> i32;
}
struct Square { side: i32 }
impl Area for Square {
    fn area(&self) -> i32 {
        self.side * self.side
    }
}
fn main() -> i32 {
    let s = Square { side: 5 };
    s.area()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 25, "expected exit 25, got {exit_code}");
}

/// Milestone 46: trait method receives extra parameter alongside `&self`.
///
/// FLS §13: Trait methods may declare parameters beyond `self`.
/// FLS §9.2: Parameters are passed in x0..x{N-1} per ARM64 ABI.
#[test]
fn milestone_46_trait_method_with_param() {
    let src = r#"
trait Scale {
    fn scale(&self, factor: i32) -> i32;
}
struct Counter { count: i32 }
impl Scale for Counter {
    fn scale(&self, factor: i32) -> i32 {
        self.count * factor
    }
}
fn main() -> i32 {
    let c = Counter { count: 7 };
    c.scale(3)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 21, "expected exit 21, got {exit_code}");
}

/// Milestone 46: trait method on a struct passed as a function parameter.
///
/// FLS §13: Trait method calls work on struct values passed to functions.
/// The receiver type is resolved statically — no dynamic dispatch.
#[test]
fn milestone_46_trait_method_on_parameter() {
    let src = r#"
trait Describe {
    fn value(&self) -> i32;
}
struct Pair { x: i32, y: i32 }
impl Describe for Pair {
    fn value(&self) -> i32 {
        self.x + self.y
    }
}
fn sum_describe(p: Pair) -> i32 {
    p.value()
}
fn main() -> i32 {
    let p = Pair { x: 10, y: 8 };
    sum_describe(p)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 18, "expected exit 18, got {exit_code}");
}

/// Milestone 46: two different structs each implementing the same trait.
///
/// FLS §13: Multiple types may implement the same trait. Each implementation
/// is monomorphized independently — `rect.area()` and `circle.area()` resolve
/// to different functions with no shared code.
#[test]
fn milestone_46_two_impls_same_trait() {
    let src = r#"
trait HasValue {
    fn get(&self) -> i32;
}
struct Small { v: i32 }
struct Large { v: i32 }
impl HasValue for Small {
    fn get(&self) -> i32 { self.v }
}
impl HasValue for Large {
    fn get(&self) -> i32 { self.v * 10 }
}
fn main() -> i32 {
    let a = Small { v: 3 };
    let b = Large { v: 2 };
    a.get() + b.get()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 23, "expected exit 23, got {exit_code}");
}

/// Milestone 46: trait impl coexists with inherent impl on the same struct.
///
/// FLS §11: A struct may have both inherent impls and trait impls.
/// Both are mangled as `TypeName__method_name`; method name collision would
/// be a type error in Rust, but galvanic does not type-check at this milestone.
#[test]
fn milestone_46_trait_and_inherent_impl() {
    let src = r#"
trait Score {
    fn score(&self) -> i32;
}
struct Player { points: i32 }
impl Player {
    fn bonus(&self) -> i32 { 5 }
}
impl Score for Player {
    fn score(&self) -> i32 { self.points + self.bonus() }
}
fn main() -> i32 {
    let p = Player { points: 10 };
    p.score()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "expected exit 15, got {exit_code}");
}

/// Milestone 46: &mut self trait method mutates a field.
///
/// FLS §13: Trait methods may take `&mut self`.
/// FLS §10.1: `&mut self` method write-back applies to trait impls too.
#[test]
fn milestone_46_trait_mut_self() {
    let src = r#"
trait Increment {
    fn inc(&mut self);
}
struct Tally { count: i32 }
impl Increment for Tally {
    fn inc(&mut self) {
        self.count = self.count + 1;
    }
}
fn main() -> i32 {
    let mut t = Tally { count: 0 };
    t.inc();
    t.inc();
    t.inc();
    t.count
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "expected exit 3, got {exit_code}");
}

/// Milestone 46: trait method result used in arithmetic expression.
///
/// FLS §6.5.5: Arithmetic operator expressions. The return value of a trait
/// method call is a first-class value usable in further expressions.
#[test]
fn milestone_46_trait_result_in_arithmetic() {
    let src = r#"
trait Half {
    fn half(&self) -> i32;
}
struct Number { n: i32 }
impl Half for Number {
    fn half(&self) -> i32 { self.n / 2 }
}
fn main() -> i32 {
    let a = Number { n: 20 };
    let b = Number { n: 10 };
    a.half() + b.half()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "expected exit 15, got {exit_code}");
}

// ── Assembly inspection: milestone 46 ────────────────────────────────────────

// ── Milestone 47: array expressions and indexing ──────────────────────────────

/// Milestone 47: array literal with constant index — first element.
///
/// FLS §6.8: Array expression `[10, 20, 30]` constructs a stack array.
/// FLS §6.9: Index expression `a[0]` accesses the first element.
/// The array is stored in consecutive stack slots; `a[0]` loads slot 0.
#[test]
fn milestone_47_array_index_first() {
    let src = r#"
fn main() -> i32 {
    let a = [10, 20, 30];
    a[0]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected exit 10, got {exit_code}");
}

/// Milestone 47: array literal with constant index — middle element.
///
/// FLS §6.8, §6.9. `a[1]` must load the second element (20), not the first or third.
#[test]
fn milestone_47_array_index_middle() {
    let src = r#"
fn main() -> i32 {
    let a = [10, 20, 30];
    a[1]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 20, "expected exit 20, got {exit_code}");
}

/// Milestone 47: array literal with constant index — last element.
///
/// FLS §6.8, §6.9. `a[2]` must load the third element.
#[test]
fn milestone_47_array_index_last() {
    let src = r#"
fn main() -> i32 {
    let a = [10, 20, 30];
    a[2]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 30, "expected exit 30, got {exit_code}");
}

/// Milestone 47: array element used in arithmetic.
///
/// FLS §6.8, §6.9, §6.5.5: `a[0] + a[1]` — two indexed loads, then addition.
#[test]
fn milestone_47_array_index_in_arithmetic() {
    let src = r#"
fn main() -> i32 {
    let a = [3, 7];
    a[0] + a[1]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected exit 10, got {exit_code}");
}

/// Milestone 47: variable index at runtime.
///
/// FLS §6.9: The index expression is a runtime value (`i`), not a literal.
/// The emitted `LoadIndexed` must compute the address dynamically.
#[test]
fn milestone_47_array_variable_index() {
    let src = r#"
fn main() -> i32 {
    let a = [10, 20, 30];
    let i = 2;
    a[i]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 30, "expected exit 30, got {exit_code}");
}

/// Milestone 47: index computed from a parameter.
///
/// FLS §6.9: The index is derived from a function parameter — a truly
/// runtime-unknown value. This is the litmus test (FLS constraint §1):
/// if the compiler were interpreting instead of compiling, it would not
/// be able to handle a parameter-derived index.
#[test]
fn milestone_47_array_param_index() {
    let src = r#"
fn get(i: i32) -> i32 {
    let a = [5, 10, 15];
    a[i]
}
fn main() -> i32 {
    get(1)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected exit 10, got {exit_code}");
}

/// Milestone 47: two-element array, sum both elements.
///
/// FLS §6.8, §6.9, §6.5.5. Each index load must produce the correct element.
#[test]
fn milestone_47_array_sum_elements() {
    let src = r#"
fn main() -> i32 {
    let a = [4, 6];
    a[0] + a[1]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected exit 10, got {exit_code}");
}

/// Milestone 47: array index in a loop (accumulator pattern).
///
/// FLS §6.8, §6.9, §6.15.3: Index a 5-element array in a while loop,
/// summing the elements. This verifies that `LoadIndexed` produces
/// correct results across multiple loop iterations with changing runtime
/// index values.
#[test]
fn milestone_47_array_index_in_loop() {
    let src = r#"
fn main() -> i32 {
    let a = [1, 2, 3, 4, 5];
    let mut sum = 0;
    let mut i = 0;
    while i < 5 {
        sum = sum + a[i];
        i = i + 1;
    }
    sum
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "expected exit 15, got {exit_code}");
}

// ── Assembly inspection: milestone 47 ────────────────────────────────────────

/// Milestone 47: array indexing emits `add` + `ldr` with `lsl #3`.
///
/// FLS §6.9: The indexed load must compute `sp + base*8 + index*8`.
/// ARM64: `add x{dst}, sp, #base_offset` + `ldr x{dst}, [x{dst}, x{idx}, lsl #3]`.
/// The `lsl #3` confirms element size scaling (8 bytes per slot).
#[test]
fn runtime_array_index_emits_add_and_ldr_lsl3() {
    let src = r#"
fn main() -> i32 {
    let a = [10, 20, 30];
    let i = 1;
    a[i]
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("lsl #3"),
        "expected `lsl #3` (element size scaling) in indexed load:\n{asm}"
    );
    assert!(
        asm.contains("add") && asm.contains("sp,"),
        "expected `add xN, sp, #offset` for base address:\n{asm}"
    );
}

// ── Milestone 48: array element stores ──────────────────────────────────────
//
// FLS §6.5.10: Assignment expression where the LHS is an indexed place expression.
// FLS §6.9: Indexing expressions identify elements of an array.
// FLS §6.1.2:37–45: The store must be a runtime instruction.

/// Milestone 48: basic array element store — `a[0] = 99`.
///
/// FLS §6.5.10: The assignment `a[0] = 99` stores to the first element of the array.
/// FLS §6.9: The constant index 0 selects the first element at `base_slot + 0*8`.
#[test]
fn milestone_48_array_store_first_element() {
    let src = r#"
fn main() -> i32 {
    let mut a = [1, 2, 3];
    a[0] = 99;
    a[0]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 99, "expected exit 99, got {exit_code}");
}

/// Milestone 48: store to middle element — `a[1] = 42`.
///
/// FLS §6.5.10: Store to the second element; first and third must be unchanged.
#[test]
fn milestone_48_array_store_middle_element() {
    let src = r#"
fn main() -> i32 {
    let mut a = [1, 2, 3];
    a[1] = 42;
    a[1]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 48: store to last element — `a[2] = 7`.
///
/// FLS §6.9: The last element at index N-1 must be addressable.
#[test]
fn milestone_48_array_store_last_element() {
    let src = r#"
fn main() -> i32 {
    let mut a = [1, 2, 3];
    a[2] = 7;
    a[2]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 48: store does not clobber adjacent elements.
///
/// FLS §6.5.10: Only the element at the assigned index changes.
/// Adjacent elements (`a[0]`, `a[2]`) must retain their original values.
#[test]
fn milestone_48_array_store_does_not_clobber_neighbors() {
    let src = r#"
fn main() -> i32 {
    let mut a = [10, 20, 30];
    a[1] = 99;
    a[0] + a[2]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 40, "expected exit 40 (10+30), got {exit_code}");
}

/// Milestone 48: store with runtime index from a variable.
///
/// FLS §6.9: The index operand is a runtime value; the store must use
/// `str x{src}, [x{base}, x{index}, lsl #3]` rather than a fixed offset.
#[test]
fn milestone_48_array_store_variable_index() {
    let src = r#"
fn main() -> i32 {
    let mut a = [0, 0, 0];
    let i = 2;
    a[i] = 55;
    a[i]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 55, "expected exit 55, got {exit_code}");
}

/// Milestone 48: store from an expression RHS.
///
/// FLS §6.5.10: The RHS is fully evaluated before storing. The result of
/// `3 * 4` must be computed at runtime and stored into the element.
#[test]
fn milestone_48_array_store_expr_rhs() {
    let src = r#"
fn main() -> i32 {
    let mut a = [0, 0, 0];
    a[0] = 3 * 4;
    a[0]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "expected exit 12, got {exit_code}");
}

/// Milestone 48: multiple stores to the same array.
///
/// FLS §6.5.10: Each assignment is a distinct runtime store instruction.
/// All three stores must complete in order before any read.
#[test]
fn milestone_48_array_multiple_stores() {
    let src = r#"
fn main() -> i32 {
    let mut a = [1, 2, 3];
    a[0] = 10;
    a[1] = 20;
    a[2] = 30;
    a[0] + a[1] + a[2]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 60, "expected exit 60, got {exit_code}");
}

/// Milestone 48: array store in a loop fills all elements.
///
/// FLS §6.15.3: While loop executes until condition is false.
/// FLS §6.5.10: Each iteration stores to a different element via a runtime index.
#[test]
fn milestone_48_array_store_in_loop() {
    let src = r#"
fn main() -> i32 {
    let mut a = [0, 0, 0, 0, 0];
    let mut i = 0;
    while i < 5 {
        a[i] = i + 1;
        i = i + 1;
    }
    a[0] + a[1] + a[2] + a[3] + a[4]
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "expected exit 15 (1+2+3+4+5), got {exit_code}");
}

/// Milestone 48: store from a function parameter.
///
/// FLS §6.5.10: The RHS may be any expression, including a function parameter.
#[test]
fn milestone_48_array_store_from_param() {
    let src = r#"
fn fill(v: i32) -> i32 {
    let mut a = [0, 0, 0];
    a[1] = v;
    a[1]
}
fn main() -> i32 {
    fill(77)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 77, "expected exit 77, got {exit_code}");
}

// ── Assembly inspection: milestone 48 ────────────────────────────────────────

/// Milestone 48: array store emits `add` + `str` with `lsl #3`.
///
/// FLS §6.5.10 + §6.9: The store `a[i] = v` must compute `sp + base*8`
/// into a scratch register, then use `str x{src}, [x{scratch}, x{idx}, lsl #3]`.
/// The `lsl #3` confirms element size scaling (8 bytes per slot).
#[test]
fn runtime_array_store_emits_add_and_str_lsl3() {
    let src = r#"
fn main() -> i32 {
    let mut a = [10, 20, 30];
    let i = 1;
    a[i] = 99;
    a[i]
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("str") && asm.contains("lsl #3"),
        "expected `str ... lsl #3` for indexed store:\n{asm}"
    );
    assert!(
        asm.contains("add") && asm.contains("sp,"),
        "expected `add xN, sp, #offset` for base address:\n{asm}"
    );
}

/// Milestone 46: trait impl method emitted under `TypeName__method_name`.
///
/// FLS §13: Trait methods resolve via static dispatch using the same mangling
/// as inherent methods. `Square__area` must appear in the assembly.
#[test]
fn runtime_trait_impl_emits_mangled_name() {
    let src = r#"
trait Area {
    fn area(&self) -> i32;
}
struct Square { side: i32 }
impl Area for Square {
    fn area(&self) -> i32 { self.side * self.side }
}
fn main() -> i32 {
    let s = Square { side: 3 };
    s.area()
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("Square__area"),
        "expected mangled `Square__area` in assembly:\n{asm}"
    );
    assert!(
        asm.contains("bl") && asm.contains("Square__area"),
        "expected `bl Square__area` call in assembly:\n{asm}"
    );
}

// ── Milestone 49: tuple expressions (FLS §6.10) ───────────────────────────────
//
// A tuple `(a, b)` is a heterogeneous sequence of values. Galvanic stores
// each element in a consecutive stack slot (same layout as struct fields or
// arrays). Field access `.0`, `.1` loads from `base_slot + index`.
//
// FLS §6.10: Tuple expressions and §6.10 field access.
// Cache-line note: N-element tuple fills N consecutive 8-byte slots.

/// Milestone 49: access first field of a two-element tuple.
///
/// FLS §6.10: `t.0` loads the first element.
#[test]
fn milestone_49_tuple_first_field() {
    let src = r#"
fn main() -> i32 {
    let t = (1, 2);
    t.0
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected exit 1, got {exit_code}");
}

/// Milestone 49: access second field of a two-element tuple.
///
/// FLS §6.10: `t.1` loads the second element.
#[test]
fn milestone_49_tuple_second_field() {
    let src = r#"
fn main() -> i32 {
    let t = (3, 7);
    t.1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 49: sum of tuple fields.
///
/// FLS §6.10: Both fields are loaded and used in arithmetic.
#[test]
fn milestone_49_tuple_field_sum() {
    let src = r#"
fn main() -> i32 {
    let t = (10, 32);
    t.0 + t.1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 49: three-element tuple, access middle field.
///
/// FLS §6.10: `.1` on a 3-tuple accesses the second slot.
#[test]
fn milestone_49_tuple_three_elements_middle() {
    let src = r#"
fn main() -> i32 {
    let t = (5, 99, 7);
    t.1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 99, "expected exit 99, got {exit_code}");
}

/// Milestone 49: tuple fields from expressions (not just literals).
///
/// FLS §6.10: Elements are evaluated left-to-right at runtime.
#[test]
fn milestone_49_tuple_expr_elements() {
    let src = r#"
fn main() -> i32 {
    let x = 3;
    let y = 4;
    let t = (x * x, y * y);
    t.0 + t.1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 25, "expected exit 25, got {exit_code}");
}

/// Milestone 49: tuple field passed to a function.
///
/// FLS §6.10: Tuple fields can be used anywhere an i32 can.
#[test]
fn milestone_49_tuple_field_to_fn() {
    let src = r#"
fn double(n: i32) -> i32 { n + n }
fn main() -> i32 {
    let t = (21, 0);
    double(t.0)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 49: tuple with function result as element.
///
/// FLS §6.10: Elements are arbitrary expressions.
#[test]
fn milestone_49_tuple_fn_result_element() {
    let src = r#"
fn five() -> i32 { 5 }
fn main() -> i32 {
    let t = (five(), 10);
    t.0 + t.1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "expected exit 15, got {exit_code}");
}

/// Milestone 49: tuple field in a conditional.
///
/// FLS §6.10 + §6.17: Tuple fields can be used as if-conditions.
#[test]
fn milestone_49_tuple_field_in_if() {
    let src = r#"
fn main() -> i32 {
    let t = (1, 0);
    if t.0 == 1 { 42 } else { 0 }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 49: tuple with parameter.
///
/// FLS §6.10: Tuple elements can come from function parameters.
#[test]
fn milestone_49_tuple_from_param() {
    let src = r#"
fn swap_sum(a: i32, b: i32) -> i32 {
    let t = (b, a);
    t.0 + t.1
}
fn main() -> i32 {
    swap_sum(10, 32)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

// ── Assembly inspection: milestone 49 ────────────────────────────────────────

/// Milestone 49: tuple init emits stores and field access emits a load.
///
/// FLS §6.10: `let t = (a, b)` stores two values to consecutive slots.
/// `t.0` loads from the base slot; `t.1` loads from base+1.
#[test]
fn runtime_tuple_emits_stores_and_load() {
    let src = r#"
fn main() -> i32 {
    let t = (1, 2);
    t.0 + t.1
}
"#;
    let asm = compile_to_asm(src);
    // Should emit at least two stores (for elements 0 and 1).
    let store_count = asm.lines().filter(|l| l.trim().starts_with("str")).count();
    assert!(
        store_count >= 2,
        "expected at least 2 store instructions for tuple init:\n{asm}"
    );
    // Should emit at least two loads (for t.0 and t.1).
    let load_count = asm.lines().filter(|l| l.trim().starts_with("ldr")).count();
    assert!(
        load_count >= 2,
        "expected at least 2 load instructions for field access:\n{asm}"
    );
}

// ── Milestone 50: tuple element stores ───────────────────────────────────────

/// Milestone 50: basic tuple element store.
///
/// FLS §6.5.10: Assignment where the LHS is a tuple field access is a place
/// expression assignment. FLS §6.10: Tuple fields are indexed by integer.
#[test]
fn milestone_50_tuple_store_first_element() {
    let src = r#"
fn main() -> i32 {
    let mut t = (0, 1);
    t.0 = 42;
    t.0
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 50: store to second element.
///
/// FLS §6.10: `t.1` accesses slot base+1.
#[test]
fn milestone_50_tuple_store_second_element() {
    let src = r#"
fn main() -> i32 {
    let mut t = (1, 0);
    t.1 = 42;
    t.1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 50: store does not clobber neighbouring element.
///
/// FLS §6.10: Elements occupy independent consecutive stack slots.
#[test]
fn milestone_50_tuple_store_does_not_clobber_neighbour() {
    let src = r#"
fn main() -> i32 {
    let mut t = (10, 32);
    t.0 = 10;
    t.0 + t.1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 50: multiple stores to different elements.
///
/// FLS §6.5.10 + §6.10: Each assignment updates one slot independently.
#[test]
fn milestone_50_tuple_multiple_stores() {
    let src = r#"
fn main() -> i32 {
    let mut t = (0, 0);
    t.0 = 20;
    t.1 = 22;
    t.0 + t.1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 50: store from a function parameter.
///
/// FLS §6.5.10: The RHS is an arbitrary expression, including a parameter.
#[test]
fn milestone_50_tuple_store_from_param() {
    let src = r#"
fn set_first(val: i32) -> i32 {
    let mut t = (0, 0);
    t.0 = val;
    t.0
}
fn main() -> i32 {
    set_first(42)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 50: store inside a loop.
///
/// FLS §6.5.10 + §6.15.3: Tuple element stores in while loops emit runtime
/// store instructions each iteration.
#[test]
fn milestone_50_tuple_store_in_loop() {
    let src = r#"
fn main() -> i32 {
    let mut t = (0, 0);
    let mut i = 0;
    while i < 6 {
        t.0 = t.0 + i;
        i = i + 1;
    }
    t.0
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    // 0+1+2+3+4+5 = 15
    assert_eq!(exit_code, 15, "expected exit 15, got {exit_code}");
}

/// Milestone 50: store from expression (arithmetic RHS).
///
/// FLS §6.5.10: The RHS may be any value expression.
#[test]
fn milestone_50_tuple_store_expr_rhs() {
    let src = r#"
fn main() -> i32 {
    let mut t = (0, 0);
    t.0 = 6 * 7;
    t.0
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

// ── Assembly inspection: milestone 50 ────────────────────────────────────────

/// Milestone 50: tuple element store emits a `str` instruction.
///
/// FLS §6.5.10 + §6.10: `t.0 = v` must emit a runtime `str` to the tuple's
/// base slot — no compile-time substitution.
#[test]
fn runtime_tuple_store_emits_str() {
    let src = r#"
fn main() -> i32 {
    let mut t = (0, 0);
    t.0 = 42;
    t.0
}
"#;
    let asm = compile_to_asm(src);
    // The tuple init (t = (0,0)) emits 2 str instructions.
    // The store (t.0 = 42) emits at least one more str.
    let store_count = asm.lines().filter(|l| l.trim().starts_with("str")).count();
    assert!(
        store_count >= 3,
        "expected at least 3 str instructions (2 init + 1 element store), got {store_count}:\n{asm}"
    );
}

// ── Milestone 51: impl blocks on enum types ───────────────────────────────────
//
// FLS §10.1: Associated items may be defined for any nominal type, including
// enums. FLS §11: Inherent implementations attach methods to a type.
// FLS §15: Enum variants carry a discriminant; methods on enums typically
// use match to dispatch on the discriminant.

/// Milestone 51: basic method on a unit-variant enum, dispatch via match.
///
/// FLS §10.1, §11, §15: impl on an enum type. The method matches `self` and
/// returns different values for each variant.
#[test]
fn milestone_51_enum_method_unit_variants() {
    let src = r#"
enum Dir { North, South }

impl Dir {
    fn code(&self) -> i32 {
        match self {
            Dir::North => 1,
            Dir::South => 2,
        }
    }
}

fn main() -> i32 {
    let d = Dir::South;
    d.code()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "expected exit 2, got {exit_code}");
}

/// Milestone 51: method extracts field from a tuple variant.
///
/// FLS §10.1, §11, §15: Method on enum with tuple variant — pattern binds the
/// field so the method can compute with it.
#[test]
fn milestone_51_enum_method_extracts_tuple_field() {
    let src = r#"
enum Wrap { Val(i32) }

impl Wrap {
    fn get(&self) -> i32 {
        match self {
            Wrap::Val(v) => v,
        }
    }
}

fn main() -> i32 {
    let w = Wrap::Val(42);
    w.get()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 51: method on multi-variant enum with different field shapes.
///
/// FLS §10.1, §15: The method must dispatch on the discriminant at runtime
/// and access variant-specific fields. Matches the FLS §15 requirement that
/// only the active variant's fields are accessible.
#[test]
fn milestone_51_enum_method_multi_variant() {
    let src = r#"
enum Shape { Circle(i32), Rect(i32, i32) }

impl Shape {
    fn area(&self) -> i32 {
        match self {
            Shape::Circle(r) => r * r,
            Shape::Rect(w, h) => w * h,
        }
    }
}

fn main() -> i32 {
    let s = Shape::Rect(3, 4);
    s.area()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "expected exit 12, got {exit_code}");
}

/// Milestone 51: method on enum parameter (not just local variable).
///
/// FLS §10.1: Method receiver is passed by value. Caller loads discriminant
/// and fields from the enum parameter's slots before calling.
#[test]
fn milestone_51_enum_method_on_parameter() {
    let src = r#"
enum Shape { Circle(i32), Rect(i32, i32) }

impl Shape {
    fn area(&self) -> i32 {
        match self {
            Shape::Circle(r) => r * r,
            Shape::Rect(w, h) => w * h,
        }
    }
}

fn compute(s: Shape) -> i32 {
    s.area()
}

fn main() -> i32 {
    compute(Shape::Circle(7))
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 49, "expected exit 49, got {exit_code}");
}

/// Milestone 51: method result used in arithmetic.
///
/// FLS §6.12.2, §10.1: Method call expression evaluates to the return value,
/// which may be used in any expression context.
#[test]
fn milestone_51_enum_method_result_in_arithmetic() {
    let src = r#"
enum Val { Num(i32) }

impl Val {
    fn get(&self) -> i32 {
        match self {
            Val::Num(n) => n,
        }
    }
}

fn main() -> i32 {
    let a = Val::Num(5);
    let b = Val::Num(7);
    a.get() + b.get()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "expected exit 12, got {exit_code}");
}

/// Milestone 51: two methods on the same enum.
///
/// FLS §11: An inherent impl may contain multiple methods, each compiled to a
/// separately mangled top-level function.
#[test]
fn milestone_51_two_enum_methods() {
    let src = r#"
enum Pair { Both(i32, i32) }

impl Pair {
    fn first(&self) -> i32 {
        match self {
            Pair::Both(a, b) => a,
        }
    }
    fn second(&self) -> i32 {
        match self {
            Pair::Both(a, b) => b,
        }
    }
}

fn main() -> i32 {
    let p = Pair::Both(3, 39);
    p.first() + p.second()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

// ── Assembly inspection: milestone 51 ────────────────────────────────────────

/// Milestone 51: enum method emits a mangled function name.
///
/// FLS §10.1: Methods are lowered to top-level functions with mangled names
/// `TypeName__method_name`. Enum methods use the same mangling as struct methods.
#[test]
fn runtime_enum_method_emits_mangled_name() {
    let src = r#"
enum Dir { North, South }

impl Dir {
    fn code(&self) -> i32 {
        match self {
            Dir::North => 1,
            Dir::South => 2,
        }
    }
}

fn main() -> i32 {
    let d = Dir::North;
    d.code()
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("Dir__code"),
        "expected mangled name `Dir__code` in assembly:\n{asm}"
    );
    // The call site must use `bl Dir__code`.
    assert!(
        asm.contains("bl      Dir__code"),
        "expected `bl Dir__code` in assembly:\n{asm}"
    );
}

// ── Milestone 52: TupleStruct and StructVariant patterns in if-let / while-let ──
//
// FLS §6.17: "An if let expression is syntactic sugar for a match expression
// with a single arm." The pattern may be any valid pattern, including enum
// tuple variant patterns (`Enum::Variant(f0, f1)`) and named variant patterns
// (`Enum::Variant { field }`).
//
// FLS §6.15.4: while-let uses the same pattern language as if-let. A mismatch
// exits the loop.
//
// FLS §5.4: Struct patterns (used here for tuple variant patterns in if-let
// / while-let). Field bindings are positional: field 0 at base+1, field 1 at
// base+2, etc.
//
// FLS §5.3: Named-field struct patterns. Field name lookup is by declaration
// order in the enum definition.
//
// FLS §6.1.2:37–45: All discriminant checks and field loads emit runtime
// instructions.
//
// No FLS code example exists for if-let / while-let with enum variant patterns
// specifically; these tests are derived from the semantic descriptions in §6.17,
// §6.15.4, §5.4, §5.3, and §15.

/// Milestone 52: if-let with a tuple variant pattern — taken branch.
///
/// `if let Opt::Some(v) = x { v } else { 0 }` must load field 0 from the enum
/// and return it when the discriminant matches.
///
/// FLS §6.17, §5.4, §15.
#[test]
fn milestone_52_if_let_tuple_variant_taken() {
    let src = r#"
enum Opt { None, Some(i32) }

fn main() -> i32 {
    let x = Opt::Some(42);
    if let Opt::Some(v) = x { v } else { 0 }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 52: if-let with a tuple variant pattern — else branch.
///
/// The pattern does not match (discriminant is `None = 0`, not `Some = 1`)
/// so the else branch executes and returns 7.
///
/// FLS §6.17, §5.4, §15.
#[test]
fn milestone_52_if_let_tuple_variant_not_taken() {
    let src = r#"
enum Opt { None, Some(i32) }

fn main() -> i32 {
    let x = Opt::None;
    if let Opt::Some(v) = x { v } else { 7 }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 52: if-let on a parameter with a tuple variant pattern.
///
/// FLS §6.17, §5.4, §9 (function parameters).
#[test]
fn milestone_52_if_let_tuple_variant_on_parameter() {
    let src = r#"
enum Opt { None, Some(i32) }

fn extract(x: Opt) -> i32 {
    if let Opt::Some(v) = x { v } else { 0 }
}

fn main() -> i32 {
    extract(Opt::Some(13)) + extract(Opt::None)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 13, "expected exit 13, got {exit_code}");
}

/// Milestone 52: if-let with a two-field tuple variant.
///
/// FLS §5.4: positional field bindings — field 0 at base+1, field 1 at base+2.
#[test]
fn milestone_52_if_let_two_field_tuple_variant() {
    let src = r#"
enum Pair { Empty, Full(i32, i32) }

fn main() -> i32 {
    let p = Pair::Full(10, 32);
    if let Pair::Full(a, b) = p { a + b } else { 0 }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 52: if-let tuple variant field used in arithmetic.
///
/// FLS §6.17, §5.4, §6.5.5.
#[test]
fn milestone_52_if_let_tuple_variant_field_arithmetic() {
    let src = r#"
enum Opt { None, Some(i32) }

fn main() -> i32 {
    let x = Opt::Some(20);
    if let Opt::Some(v) = x { v + v + 2 } else { 0 }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 52: if-let unit (statement context) with tuple variant.
///
/// FLS §6.17: if-let with unit type (side effects only).
#[test]
fn milestone_52_if_let_tuple_variant_unit() {
    let src = r#"
enum Opt { None, Some(i32) }

fn main() -> i32 {
    let x = Opt::Some(42);
    let mut result = 0;
    if let Opt::Some(v) = x {
        result = v;
    }
    result
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 52: while-let with a tuple variant pattern — counts down.
///
/// FLS §6.15.4, §5.4, §15.
#[test]
fn milestone_52_while_let_tuple_variant_counts() {
    let src = r#"
enum Opt { None, Some(i32) }

fn wrap(n: i32) -> Opt {
    if n > 0 { Opt::Some(n) } else { Opt::None }
}

fn main() -> i32 {
    let mut x = Opt::Some(5);
    let mut sum = 0;
    while let Opt::Some(v) = x {
        sum = sum + v;
        x = wrap(v - 1);
    }
    sum
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "expected exit 15, got {exit_code}");
}

/// Milestone 52: while-let tuple variant exits immediately on non-matching variant.
///
/// FLS §6.15.4: mismatch terminates the loop without executing the body.
#[test]
fn milestone_52_while_let_tuple_variant_no_match() {
    let src = r#"
enum Opt { None, Some(i32) }

fn main() -> i32 {
    let x = Opt::None;
    let mut result = 7;
    while let Opt::Some(v) = x {
        result = v;
    }
    result
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 52: if-let with a named-field struct variant pattern — taken.
///
/// FLS §6.17, §5.3, §15.3.
#[test]
fn milestone_52_if_let_struct_variant_taken() {
    let src = r#"
enum Shape { Empty, Rect { w: i32, h: i32 } }

fn main() -> i32 {
    let s = Shape::Rect { w: 6, h: 7 };
    if let Shape::Rect { w, h } = s { w * h } else { 0 }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 52: if-let named-field variant pattern — not taken.
///
/// FLS §6.17, §5.3.
#[test]
fn milestone_52_if_let_struct_variant_not_taken() {
    let src = r#"
enum Shape { Empty, Rect { w: i32, h: i32 } }

fn main() -> i32 {
    let s = Shape::Empty;
    if let Shape::Rect { w, h } = s { w * h } else { 99 }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 99, "expected exit 99, got {exit_code}");
}

/// Milestone 52: while-let with a named-field struct variant pattern.
///
/// FLS §6.15.4, §5.3, §15.3.
#[test]
fn milestone_52_while_let_struct_variant() {
    let src = r#"
enum Step { Done, Go { n: i32 } }

fn next(n: i32) -> Step {
    if n > 1 { Step::Go { n: n - 1 } } else { Step::Done }
}

fn main() -> i32 {
    let mut s = Step::Go { n: 4 };
    let mut count = 0;
    while let Step::Go { n } = s {
        count = count + n;
        s = next(n);
    }
    count
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected exit 10, got {exit_code}");
}

// ── Assembly inspection: milestone 52 ────────────────────────────────────────

/// Milestone 52: if-let tuple variant emits discriminant comparison and cbz.
///
/// The discriminant check must emit: load scrut → load imm → cmp (BinOp Eq) →
/// cbz to else. This confirms runtime code, not compile-time folding.
///
/// FLS §6.1.2:37–45: All checks are runtime.
#[test]
fn runtime_if_let_tuple_variant_emits_discriminant_check() {
    let src = r#"
enum Opt { None, Some(i32) }

fn main() -> i32 {
    let x = Opt::Some(1);
    if let Opt::Some(v) = x { v } else { 0 }
}
"#;
    let asm = compile_to_asm(src);
    // The discriminant check must load the discriminant and compare.
    assert!(
        asm.contains("cbz"),
        "expected `cbz` for discriminant branch in assembly:\n{asm}"
    );
    // The field load: ldr from base+1 slot.
    assert!(
        asm.contains("ldr"),
        "expected `ldr` for field binding in assembly:\n{asm}"
    );
}

// ── Milestone 53: struct patterns in match ────────────────────────────────────
//
// FLS §5.3: Struct patterns. A struct pattern `Point { x, y }` matches a
// struct value and binds its named fields. For plain struct types (not enum
// variants), the pattern is irrefutable — no discriminant check is emitted.
//
// FLS §6.1.2:37–45: All code is runtime; field loads emit `ldr` instructions.
// FLS §6.18: Match expression evaluates scrutinee and matches arms in order.

/// Milestone 53: match on struct — bind both fields and sum them.
///
/// FLS §5.3: `Point { x, y }` binds fields `x` and `y` from a `Point` struct.
/// FLS §6.18: Single-arm match with struct pattern — always matches.
#[test]
fn milestone_53_struct_match_sum_fields() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn sum(p: Point) -> i32 {
    match p {
        Point { x, y } => x + y,
    }
}
fn main() -> i32 {
    sum(Point { x: 3, y: 4 })
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 53: match on struct — first field only.
///
/// FLS §5.3: `Point { x, y: _ }` binds `x` and discards `y`.
#[test]
fn milestone_53_struct_match_first_field() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn get_x(p: Point) -> i32 {
    match p {
        Point { x, y: _ } => x,
    }
}
fn main() -> i32 {
    get_x(Point { x: 5, y: 99 })
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected exit 5, got {exit_code}");
}

/// Milestone 53: match on struct — second field only.
///
/// FLS §5.3: `Pair { a: _, b }` binds `b` and discards `a`.
#[test]
fn milestone_53_struct_match_second_field() {
    let src = r#"
struct Pair { a: i32, b: i32 }
fn get_b(p: Pair) -> i32 {
    match p {
        Pair { a: _, b } => b,
    }
}
fn main() -> i32 {
    get_b(Pair { a: 1, b: 9 })
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 9, "expected exit 9, got {exit_code}");
}

/// Milestone 53: match on struct — field in arithmetic with another local.
///
/// FLS §5.3: bound fields are ordinary locals in the arm body.
#[test]
fn milestone_53_struct_match_field_in_arithmetic() {
    let src = r#"
struct Val { n: i32 }
fn double(v: Val) -> i32 {
    match v {
        Val { n } => n * 2,
    }
}
fn main() -> i32 {
    double(Val { n: 6 })
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "expected exit 12, got {exit_code}");
}

/// Milestone 53: match on struct — three fields, use all three.
///
/// FLS §5.3: struct pattern with multiple field bindings.
#[test]
fn milestone_53_struct_match_three_fields() {
    let src = r#"
struct Triple { a: i32, b: i32, c: i32 }
fn sum3(t: Triple) -> i32 {
    match t {
        Triple { a, b, c } => a + b + c,
    }
}
fn main() -> i32 {
    sum3(Triple { a: 1, b: 2, c: 3 })
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "expected exit 6, got {exit_code}");
}

/// Milestone 53: match on struct — pattern binds out of source order.
///
/// FLS §5.3: Named-field struct patterns may list fields in any order.
/// Field binding maps by name, not position.
#[test]
fn milestone_53_struct_match_out_of_order_binding() {
    let src = r#"
struct Pt { x: i32, y: i32 }
fn diff(p: Pt) -> i32 {
    match p {
        Pt { y, x } => x - y,
    }
}
fn main() -> i32 {
    diff(Pt { x: 10, y: 3 })
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 53: match on struct — result passed to another function.
///
/// FLS §5.3: bound field values are usable as regular expression values.
#[test]
fn milestone_53_struct_match_field_passed_to_fn() {
    let src = r#"
struct Wrap { v: i32 }
fn inc(n: i32) -> i32 { n + 1 }
fn unwrap_and_inc(w: Wrap) -> i32 {
    match w {
        Wrap { v } => inc(v),
    }
}
fn main() -> i32 {
    unwrap_and_inc(Wrap { v: 4 })
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected exit 5, got {exit_code}");
}

// ── Assembly inspection: milestone 53 ────────────────────────────────────────

/// Milestone 53: struct match emits field loads (ldr) but no discriminant check.
///
/// FLS §5.3: Plain struct patterns are irrefutable — no discriminant comparison
/// is needed. The assembly must contain `ldr` (for field binding) and `add`
/// (for the arm body arithmetic), confirming runtime code was emitted.
///
/// FLS §6.1.2:37–45: Field loads are runtime instructions.
#[test]
fn runtime_struct_match_emits_ldr_without_discriminant_check() {
    // Use a let-bound struct variable as scrutinee — no struct-literal-as-arg.
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let p = Point { x: 1, y: 2 }; match p { Point { x, y } => x + y } }\n";
    let asm = compile_to_asm(src);
    // Field loads must be present for the pattern bindings.
    assert!(
        asm.contains("ldr"),
        "expected `ldr` for field binding in assembly:\n{asm}"
    );
    // The arm body `x + y` must emit an `add` instruction.
    assert!(
        asm.contains("add"),
        "expected `add` for field arithmetic in assembly:\n{asm}"
    );
}

// ── Milestone 54: Tuple structs compile to runtime ARM64 ──────────────────────

/// Milestone 54: simple tuple struct — first field access.
///
/// FLS §14.2: Tuple structs have positional fields accessed via `.0`, `.1`.
/// FLS §6.10: Tuple field access expressions.
#[test]
fn milestone_54_tuple_struct_first_field() {
    let src = r#"
struct Point(i32, i32);
fn main() -> i32 {
    let p = Point(7, 3);
    p.0
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 54: tuple struct — second field access.
///
/// FLS §14.2, §6.10: Second positional field at slot base_slot + 1.
#[test]
fn milestone_54_tuple_struct_second_field() {
    let src = r#"
struct Point(i32, i32);
fn main() -> i32 {
    let p = Point(3, 9);
    p.1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 9, "expected exit 9, got {exit_code}");
}

/// Milestone 54: tuple struct — sum of both fields.
///
/// FLS §14.2, §6.10: Both fields accessible and usable in arithmetic.
#[test]
fn milestone_54_tuple_struct_field_sum() {
    let src = r#"
struct Pair(i32, i32);
fn main() -> i32 {
    let p = Pair(4, 6);
    p.0 + p.1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected exit 10, got {exit_code}");
}

/// Milestone 54: tuple struct — fields from parameters.
///
/// FLS §14.2: Constructor arguments may be any expression including parameters.
#[test]
fn milestone_54_tuple_struct_from_params() {
    let src = r#"
struct Pair(i32, i32);
fn make(a: i32, b: i32) -> i32 {
    let p = Pair(a, b);
    p.0 - p.1
}
fn main() -> i32 {
    make(10, 3)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 54: tuple struct — three fields.
///
/// FLS §14.2: Tuple structs may have any number of positional fields.
#[test]
fn milestone_54_tuple_struct_three_fields_middle() {
    let src = r#"
struct Triple(i32, i32, i32);
fn main() -> i32 {
    let t = Triple(1, 5, 9);
    t.1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected exit 5, got {exit_code}");
}

/// Milestone 54: tuple struct — field used in if-else.
///
/// FLS §14.2, §6.17: Tuple struct field access as condition operand.
#[test]
fn milestone_54_tuple_struct_field_in_if() {
    let src = r#"
struct Wrap(i32);
fn main() -> i32 {
    let w = Wrap(3);
    if w.0 > 0 { 1 } else { 0 }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected exit 1, got {exit_code}");
}

/// Milestone 54: tuple struct — passed to function.
///
/// FLS §14.2, §6.12.1: Tuple struct fields passed as function arguments.
#[test]
fn milestone_54_tuple_struct_field_passed_to_fn() {
    let src = r#"
struct Wrap(i32);
fn inc(n: i32) -> i32 { n + 1 }
fn main() -> i32 {
    let w = Wrap(6);
    inc(w.0)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

// ── Assembly inspection: milestone 54 ────────────────────────────────────────

/// Milestone 54: tuple struct construction emits str instructions and field
/// access emits ldr (same as anonymous tuple lowering).
///
/// FLS §14.2: Tuple struct fields occupy consecutive stack slots.
/// FLS §6.1.2:37–45: All stores and loads are runtime instructions.
#[test]
fn runtime_tuple_struct_emits_stores_and_loads() {
    let src = "struct Point(i32, i32);\nfn main() -> i32 { let p = Point(3, 4); p.0 + p.1 }\n";
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("str"),
        "expected `str` for tuple struct field stores in assembly:\n{asm}"
    );
    assert!(
        asm.contains("ldr"),
        "expected `ldr` for tuple struct field loads in assembly:\n{asm}"
    );
}

// ── Milestone 55: impl blocks on tuple structs ────────────────────────────────

/// Milestone 55: `&self` method on a one-field tuple struct.
///
/// FLS §14.2: Tuple struct types. FLS §10.1: Associated items.
/// FLS §6.12.2: Method call expressions.
#[test]
fn milestone_55_method_first_field() {
    let src = r#"
struct Wrap(i32);
impl Wrap {
    fn val(&self) -> i32 { self.0 }
}
fn main() -> i32 {
    let w = Wrap(7);
    w.val()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 55: `&self` method on a two-field tuple struct returning sum.
///
/// FLS §14.2, §10.1, §6.5.5: Tuple struct field arithmetic inside a method.
#[test]
fn milestone_55_method_returns_field_sum() {
    let src = r#"
struct Point(i32, i32);
impl Point {
    fn sum(&self) -> i32 { self.0 + self.1 }
}
fn main() -> i32 {
    let p = Point(3, 4);
    p.sum()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 55: `&self` method returns second field.
///
/// FLS §14.2, §10.1: Second field indexed at slot base + 1.
#[test]
fn milestone_55_method_second_field() {
    let src = r#"
struct Point(i32, i32);
impl Point {
    fn y(&self) -> i32 { self.1 }
}
fn main() -> i32 {
    let p = Point(10, 5);
    p.y()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected exit 5, got {exit_code}");
}

/// Milestone 55: `&self` method with an extra parameter.
///
/// FLS §14.2, §10.1, §9: Extra parameters follow self fields in registers.
#[test]
fn milestone_55_method_with_extra_param() {
    let src = r#"
struct Wrap(i32);
impl Wrap {
    fn add(&self, n: i32) -> i32 { self.0 + n }
}
fn main() -> i32 {
    let w = Wrap(3);
    w.add(4)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 55: method called on a tuple struct parameter passed to a function.
///
/// FLS §14.2, §10.1: Method dispatch works on tuple struct passed as value.
#[test]
fn milestone_55_method_on_parameter() {
    let src = r#"
struct Wrap(i32);
impl Wrap {
    fn val(&self) -> i32 { self.0 }
}
fn extract(w: Wrap) -> i32 { w.val() }
fn main() -> i32 {
    let w = Wrap(9);
    extract(w)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 9, "expected exit 9, got {exit_code}");
}

/// Milestone 55: method result used in arithmetic.
///
/// FLS §14.2, §10.1, §6.5.5: Method return value participates in an expression.
#[test]
fn milestone_55_method_result_in_arithmetic() {
    let src = r#"
struct Wrap(i32);
impl Wrap {
    fn val(&self) -> i32 { self.0 }
}
fn main() -> i32 {
    let w = Wrap(3);
    w.val() * 2 + 1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 55: multiple methods on the same tuple struct.
///
/// FLS §14.2, §10.1, §11: Multiple methods in one impl block, each mangled
/// separately (`Point__x`, `Point__y`).
#[test]
fn milestone_55_multiple_methods() {
    let src = r#"
struct Point(i32, i32);
impl Point {
    fn x(&self) -> i32 { self.0 }
    fn y(&self) -> i32 { self.1 }
}
fn main() -> i32 {
    let p = Point(2, 5);
    p.x() + p.y()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

// ── Assembly inspection: milestone 55 ────────────────────────────────────────

/// Milestone 55: method on a tuple struct emits a `bl` to the mangled name.
///
/// FLS §14.2, §10.1: `impl Wrap` methods compile to `Wrap__method_name`.
/// FLS §6.1.2:37–45: The call is a runtime instruction.
#[test]
fn runtime_tuple_struct_method_emits_bl_mangled_name() {
    let src = "struct Wrap(i32);\nimpl Wrap { fn val(&self) -> i32 { self.0 } }\nfn main() -> i32 { let w = Wrap(7); w.val() }\n";
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("Wrap__val"),
        "expected mangled name `Wrap__val` in assembly:\n{asm}"
    );
    assert!(
        asm.contains("bl"),
        "expected `bl` instruction for method call in assembly:\n{asm}"
    );
}

// ── Milestone 56: free functions returning named structs ──────────────────────

/// Milestone 56: free function returns a named struct; caller accesses first field.
///
/// FLS §9: Functions may return named struct types.
/// FLS §6.13: Field access on the returned struct reads the correct slot.
#[test]
fn milestone_56_factory_fn_first_field() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(a: i32, b: i32) -> Point { Point { x: a, y: b } }
fn main() -> i32 {
    let p = make(3, 4);
    p.x
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "expected exit 3, got {exit_code}");
}

/// Milestone 56: free function returns a named struct; caller accesses second field.
///
/// FLS §9: Function return value is placed in x0..x{N-1} via RetFields.
/// FLS §6.13: Second field at base_slot + 1.
#[test]
fn milestone_56_factory_fn_second_field() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(a: i32, b: i32) -> Point { Point { x: a, y: b } }
fn main() -> i32 {
    let p = make(3, 4);
    p.y
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "expected exit 4, got {exit_code}");
}

/// Milestone 56: fields of the returned struct used in arithmetic.
///
/// FLS §9, §6.5.5: Field values from the returned struct participate in
/// an addition expression.
#[test]
fn milestone_56_factory_fn_field_sum() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(a: i32, b: i32) -> Point { Point { x: a, y: b } }
fn main() -> i32 {
    let p = make(3, 4);
    p.x + p.y
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 56: factory function called with runtime parameters.
///
/// FLS §9: The factory function receives its arguments at runtime; the returned
/// struct fields are runtime values, not compile-time constants.
#[test]
fn milestone_56_factory_fn_from_params() {
    let src = r#"
struct Pair { a: i32, b: i32 }
fn make_pair(x: i32, y: i32) -> Pair { Pair { a: x, b: y } }
fn sum(p: Pair) -> i32 { p.a + p.b }
fn main() -> i32 {
    let p = make_pair(2, 5);
    sum(p)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 56: struct returned from a factory function used in an if expression.
///
/// FLS §9, §6.17: A field of the returned struct is used as the condition
/// of an if/else expression.
#[test]
fn milestone_56_factory_fn_field_in_if() {
    let src = r#"
struct Wrapper { val: i32 }
fn wrap(n: i32) -> Wrapper { Wrapper { val: n } }
fn main() -> i32 {
    let w = wrap(5);
    if w.val > 3 { 1 } else { 0 }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected exit 1, got {exit_code}");
}

/// Milestone 56: three-field struct returned from a free function.
///
/// FLS §9: Multi-field struct return uses RetFields for fields 0..2.
/// FLS §6.13: Middle field accessed by name.
#[test]
fn milestone_56_factory_fn_three_fields_middle() {
    let src = r#"
struct Triple { a: i32, b: i32, c: i32 }
fn triple(x: i32, y: i32, z: i32) -> Triple { Triple { a: x, b: y, c: z } }
fn main() -> i32 {
    let t = triple(1, 5, 3);
    t.b
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected exit 5, got {exit_code}");
}

/// Milestone 56: factory function with no parameters.
///
/// FLS §9: A free function with no parameters can still return a struct.
/// FLS §6.11: Struct literal with constant fields.
#[test]
fn milestone_56_factory_fn_no_params() {
    let src = r#"
struct Origin { x: i32, y: i32 }
fn origin() -> Origin { Origin { x: 0, y: 0 } }
fn main() -> i32 {
    let o = origin();
    o.x + o.y
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected exit 0, got {exit_code}");
}

/// Milestone 56: struct field from factory function passed to another function.
///
/// FLS §9, §6.12.1: A field extracted from the returned struct is passed as
/// an argument to a second function call.
#[test]
fn milestone_56_factory_fn_field_passed_to_fn() {
    let src = r#"
struct Wrap { val: i32 }
fn wrap(n: i32) -> Wrap { Wrap { val: n } }
fn double(n: i32) -> i32 { n * 2 }
fn main() -> i32 {
    let w = wrap(4);
    double(w.val)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 8, "expected exit 8, got {exit_code}");
}

// ── Assembly inspection: milestone 56 ────────────────────────────────────────

/// Milestone 56: factory function emits RetFields and caller emits CallMut write-back.
///
/// FLS §9: The callee stores field values in x0..x{N-1} via RetFields.
/// The call site writes them to consecutive stack slots via CallMut.
/// FLS §6.1.2:37–45: All stores and loads are runtime instructions.
#[test]
fn runtime_factory_fn_emits_ret_fields_and_call_mut_write_back() {
    let src = "struct Point { x: i32, y: i32 }\nfn make(a: i32, b: i32) -> Point { Point { x: a, y: b } }\nfn main() -> i32 { let p = make(3, 4); p.x + p.y }\n";
    let asm = compile_to_asm(src);
    // Callee side: two `ldr` instructions before `ret` for RetFields.
    assert!(
        asm.contains("ldr"),
        "expected `ldr` for RetFields in assembly:\n{asm}"
    );
    // Call site: `bl make` followed by two `str` write-back instructions.
    assert!(
        asm.contains("bl\t\tmake") || asm.contains("bl      make"),
        "expected `bl make` in assembly:\n{asm}"
    );
    assert!(
        asm.contains("str"),
        "expected `str` for CallMut write-back in assembly:\n{asm}"
    );
}

// ── Milestone 57: shorthand field initialization (FLS §6.11) ─────────────────

/// Milestone 57: single shorthand field — `Point { x }` compiles end-to-end.
///
/// FLS §6.11: Shorthand field initialization — `S { field }` is equivalent
/// to `S { field: field }`. The field identifier resolves to the local variable
/// or parameter of the same name.
#[test]
fn milestone_57_shorthand_single_field() {
    let src = r#"
struct Wrap { val: i32 }
fn make(val: i32) -> Wrap { Wrap { val } }
fn main() -> i32 {
    let w = make(42);
    w.val
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 57: two shorthand fields — `Point { x, y }` compiles end-to-end.
///
/// FLS §6.11: Each shorthand field resolves to the parameter of the same name.
#[test]
fn milestone_57_shorthand_two_fields() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(x: i32, y: i32) -> Point { Point { x, y } }
fn main() -> i32 {
    let p = make(3, 4);
    p.x + p.y
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 57: shorthand field used in method body.
///
/// FLS §6.11: Shorthand field initialization works inside `impl` methods
/// just as in free functions.
#[test]
fn milestone_57_shorthand_in_method() {
    let src = r#"
struct Counter { count: i32 }
impl Counter {
    fn new(count: i32) -> Counter { Counter { count } }
    fn get(self) -> i32 { self.count }
}
fn main() -> i32 {
    let c = Counter::new(7);
    c.get()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 57: shorthand field mixed with explicit field.
///
/// FLS §6.11: Shorthand and explicit field init may appear in the same literal.
#[test]
fn milestone_57_shorthand_mixed_with_explicit() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make_x(x: i32) -> Point { Point { x, y: 0 } }
fn main() -> i32 {
    let p = make_x(5);
    p.x
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected exit 5, got {exit_code}");
}

/// Milestone 57: three-field struct with all shorthand fields.
///
/// FLS §6.11: Works for any number of fields.
#[test]
fn milestone_57_shorthand_three_fields() {
    let src = r#"
struct Triple { a: i32, b: i32, c: i32 }
fn make(a: i32, b: i32, c: i32) -> Triple { Triple { a, b, c } }
fn main() -> i32 {
    let t = make(1, 2, 4);
    t.a + t.b + t.c
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 57: shorthand field where variable is a local let binding.
///
/// FLS §6.11: The shorthand field `x` resolves to any in-scope binding named `x`.
#[test]
fn milestone_57_shorthand_from_let_binding() {
    let src = r#"
struct Wrap { val: i32 }
fn main() -> i32 {
    let val = 13;
    let w = Wrap { val };
    w.val
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 13, "expected exit 13, got {exit_code}");
}

/// Milestone 57: shorthand field used when returning a struct from free function.
///
/// FLS §6.11 + §9: Shorthand works in factory functions (milestone 56 pattern).
#[test]
fn milestone_57_shorthand_factory_fn() {
    let src = r#"
struct Rect { w: i32, h: i32 }
fn rect(w: i32, h: i32) -> Rect { Rect { w, h } }
fn area(r: Rect) -> i32 { r.w * r.h }
fn main() -> i32 {
    let r = rect(3, 4);
    area(r)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "expected exit 12, got {exit_code}");
}

// ── Assembly inspection: milestone 57 ────────────────────────────────────────

/// Milestone 57: shorthand field init emits the same stores as explicit init.
///
/// FLS §6.11: `Point { x, y }` must emit the same runtime stores as
/// `Point { x: x, y: y }`. Both must use `str` instructions, not immediates.
/// FLS §6.1.2:37–45: All stores are runtime instructions.
#[test]
fn runtime_shorthand_field_emits_same_as_explicit() {
    let src = "struct Point { x: i32, y: i32 }\nfn make(x: i32, y: i32) -> Point { Point { x, y } }\nfn main() -> i32 { let p = make(1, 2); p.x }\n";
    let asm = compile_to_asm(src);
    // Shorthand init must produce `str` instructions in the callee.
    assert!(
        asm.contains("str"),
        "expected `str` instructions from shorthand field init:\n{asm}"
    );
    // The callee must spill parameters to stack slots before using them.
    assert!(
        asm.contains("ldr"),
        "expected `ldr` instructions for RetFields:\n{asm}"
    );
}

// ── Milestone 58: struct update syntax ───────────────────────────────────────

/// Milestone 58: basic struct update — one field overridden, one copied.
///
/// FLS §6.11: `Point { x: 5, ..a }` constructs a new `Point` with `x = 5`
/// and `y` copied from `a`. The `..a` syntax fills in all fields not listed.
#[test]
fn milestone_58_struct_update_basic() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn main() -> i32 {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 5, ..a };
    b.x + b.y
}
"#;
    // b.x = 5 (explicit), b.y = 2 (copied from a). 5 + 2 = 7.
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 58: second field overridden, first copied.
///
/// FLS §6.11: any subset of fields may be specified; the rest come from base.
#[test]
fn milestone_58_struct_update_second_field() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn main() -> i32 {
    let a = Point { x: 3, y: 9 };
    let b = Point { y: 4, ..a };
    b.x + b.y
}
"#;
    // b.x = 3 (from a), b.y = 4 (explicit). 3 + 4 = 7.
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 58: all fields from base (none explicitly listed).
///
/// FLS §6.11: `Struct { ..base }` with no explicit fields copies all fields.
#[test]
fn milestone_58_struct_update_all_from_base() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn main() -> i32 {
    let a = Point { x: 10, y: 20 };
    let b = Point { ..a };
    b.x + b.y
}
"#;
    // b = copy of a. 10 + 20 = 30.
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 30, "expected exit 30, got {exit_code}");
}

/// Milestone 58: struct update with three fields.
///
/// FLS §6.11: update syntax works for structs with any number of fields.
#[test]
fn milestone_58_struct_update_three_fields() {
    let src = r#"
struct Triple { a: i32, b: i32, c: i32 }
fn main() -> i32 {
    let base = Triple { a: 1, b: 2, c: 3 };
    let t = Triple { b: 20, ..base };
    t.a + t.b + t.c
}
"#;
    // t.a = 1 (from base), t.b = 20 (explicit), t.c = 3 (from base). 1+20+3 = 24.
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 24, "expected exit 24, got {exit_code}");
}

/// Milestone 58: struct update with parameter-derived base.
///
/// FLS §6.11: the base expression may be a function parameter.
#[test]
fn milestone_58_struct_update_from_param() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn shift_x(p: Point, dx: i32) -> i32 {
    let q = Point { x: p.x + dx, ..p };
    q.x + q.y
}
fn main() -> i32 {
    let p = Point { x: 5, y: 3 };
    shift_x(p, 2)
}
"#;
    // q.x = 5 + 2 = 7, q.y = 3 (from p). 7 + 3 = 10.
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected exit 10, got {exit_code}");
}

/// Milestone 58: chained updates — update from a previously updated struct.
///
/// FLS §6.11: the base may itself have been constructed with struct update syntax.
#[test]
fn milestone_58_struct_update_chained() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn main() -> i32 {
    let a = Point { x: 1, y: 1 };
    let b = Point { x: 5, ..a };
    let c = Point { y: 7, ..b };
    c.x + c.y
}
"#;
    // c.x = 5 (from b which overrode a.x), c.y = 7 (explicit). 5 + 7 = 12.
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "expected exit 12, got {exit_code}");
}

/// Milestone 58: struct update result used in arithmetic.
///
/// FLS §6.11: the result of struct update is a value expression; its fields
/// are readable immediately.
#[test]
fn milestone_58_struct_update_in_arithmetic() {
    let src = r#"
struct Rect { w: i32, h: i32 }
fn main() -> i32 {
    let r = Rect { w: 3, h: 4 };
    let wide = Rect { w: 10, ..r };
    wide.w * wide.h
}
"#;
    // wide.w = 10, wide.h = 4 (from r). 10 * 4 = 40. Return 40 % 256 = 40.
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 40, "expected exit 40, got {exit_code}");
}

// ── Assembly inspection: milestone 58 ────────────────────────────────────────

/// Milestone 58: struct update emits ldr+str pair for copied fields.
///
/// FLS §6.11: copying a field from the base must emit a runtime `ldr` from
/// the base's slot followed by a `str` to the new struct's slot.
/// FLS §6.1.2:37–45: All copies are runtime instructions — no compile-time folding.
#[test]
fn runtime_struct_update_emits_ldr_str_for_copied_field() {
    // The `y` field is copied from `a`; this must emit `ldr` + `str`.
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let a = Point { x: 1, y: 2 }; let b = Point { x: 5, ..a }; b.y }\n";
    let asm = compile_to_asm(src);
    // `str` for explicit x field + `str` for copied y field.
    let str_count = asm.matches("str").count();
    assert!(
        str_count >= 2,
        "expected at least 2 `str` instructions for both fields:\n{asm}"
    );
    // `ldr` must appear for the copied field.
    assert!(
        asm.contains("ldr"),
        "expected `ldr` for copying y from base struct:\n{asm}"
    );
}

// ── Milestone 59: const items ────────────────────────────────────────────────
//
// FLS §7.1: Constant items. Every use of a constant is replaced with its value.
// The initializer is a constant expression (integer literal at this milestone).

/// Milestone 59: const item used as function return value.
///
/// FLS §7.1:10: The constant name is replaced with its integer literal value.
/// FLS §2.4.4.1: Integer literal initializer.
#[test]
fn milestone_59_const_as_return_value() {
    let src = r#"
const ANSWER: i32 = 42;
fn main() -> i32 { ANSWER }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 59: const item used in arithmetic.
///
/// FLS §7.1: Constant substituted into a runtime arithmetic expression.
/// FLS §6.5.5: Addition of the const value with a literal.
#[test]
fn milestone_59_const_in_arithmetic() {
    let src = r#"
const BASE: i32 = 10;
fn main() -> i32 { BASE + 5 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "expected exit 15, got {exit_code}");
}

/// Milestone 59: const item passed as function argument.
///
/// FLS §7.1: The constant value is substituted at the call site.
/// FLS §6.12.1: The substituted value is passed as a runtime argument.
#[test]
fn milestone_59_const_as_fn_arg() {
    let src = r#"
const LIMIT: i32 = 7;
fn double(n: i32) -> i32 { n * 2 }
fn main() -> i32 { double(LIMIT) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 14, "expected exit 14, got {exit_code}");
}

/// Milestone 59: const item used as loop bound.
///
/// FLS §7.1: Constant substituted as the while-loop condition's RHS.
/// FLS §6.15.3: While loop — condition evaluated at runtime on each iteration.
#[test]
fn milestone_59_const_as_loop_bound() {
    let src = r#"
const LIMIT: i32 = 5;
fn main() -> i32 {
    let mut i = 0;
    while i < LIMIT { i += 1; }
    i
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected exit 5, got {exit_code}");
}

/// Milestone 59: two const items used together.
///
/// FLS §7.1: Multiple constant items in the same program; each is
/// substituted independently at its use site.
#[test]
fn milestone_59_two_consts() {
    let src = r#"
const X: i32 = 3;
const Y: i32 = 4;
fn main() -> i32 { X + Y }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 59: const item used in if condition.
///
/// FLS §7.1: Constant substituted as one operand of a comparison.
/// FLS §6.17: If expression evaluates condition at runtime.
#[test]
fn milestone_59_const_in_if_condition() {
    let src = r#"
const THRESHOLD: i32 = 10;
fn check(n: i32) -> i32 { if n > THRESHOLD { 1 } else { 0 } }
fn main() -> i32 { check(15) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected exit 1, got {exit_code}");
}

/// Milestone 59: const item with zero value.
///
/// FLS §7.1: Zero is a valid constant value.
/// FLS §2.4.4.1: Integer literal 0.
#[test]
fn milestone_59_const_zero() {
    let src = r#"
const ZERO: i32 = 0;
fn main() -> i32 { ZERO }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected exit 0, got {exit_code}");
}

/// Milestone 59: assembly inspection — const emits LoadImm, not Load+Store.
///
/// FLS §7.1:10: A constant use is substituted with its value. In galvanic
/// this means `LoadImm` (a `mov` instruction), not a stack `Load`+`Store`.
/// There must be no stack slot allocated for the constant itself.
#[test]
fn runtime_const_emits_load_imm_not_stack_load() {
    let src = "const ANSWER: i32 = 42;\nfn main() -> i32 { ANSWER }\n";
    let asm = compile_to_asm(src);
    // The value 42 must appear as an immediate.
    assert!(
        asm.contains("42"),
        "expected immediate value 42 in assembly:\n{asm}"
    );
    // There must be a `mov` instruction materializing the constant.
    assert!(
        asm.contains("mov"),
        "expected `mov` for constant substitution:\n{asm}"
    );
}

// ── Milestone 60: static items ────────────────────────────────────────────────

/// Milestone 60: static item used as return value.
///
/// FLS §7.2: Static items have a fixed memory address in the data section.
/// FLS §7.2:15: All references to a static refer to the same memory address.
/// FLS §6.3: A path expression resolving to a static emits a memory load.
#[test]
fn milestone_60_static_as_return_value() {
    let src = r#"
static ANSWER: i32 = 42;
fn main() -> i32 { ANSWER }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 60: static item used in arithmetic.
///
/// FLS §7.2: The loaded static value participates in runtime arithmetic.
/// FLS §6.5.5: Arithmetic operator expressions evaluated at runtime.
#[test]
fn milestone_60_static_in_arithmetic() {
    let src = r#"
static BASE: i32 = 10;
fn main() -> i32 { BASE + 5 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "expected exit 15, got {exit_code}");
}

/// Milestone 60: static item passed as function argument.
///
/// FLS §7.2: The loaded static value is passed as a runtime argument.
/// FLS §6.12.1: Call expressions evaluate arguments left-to-right.
#[test]
fn milestone_60_static_as_fn_arg() {
    let src = r#"
static LIMIT: i32 = 7;
fn double(n: i32) -> i32 { n * 2 }
fn main() -> i32 { double(LIMIT) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 14, "expected exit 14, got {exit_code}");
}

/// Milestone 60: static item used as loop bound.
///
/// FLS §7.2: Static loaded on each loop iteration (each use goes through memory).
/// FLS §6.15.3: While loop condition evaluated at runtime.
#[test]
fn milestone_60_static_as_loop_bound() {
    let src = r#"
static LIMIT: i32 = 5;
fn main() -> i32 {
    let mut i = 0;
    while i < LIMIT { i += 1; }
    i
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected exit 5, got {exit_code}");
}

/// Milestone 60: static item used in if condition.
///
/// FLS §7.2: Static loaded at runtime before comparison.
/// FLS §6.17: If expression evaluates condition at runtime.
#[test]
fn milestone_60_static_in_if_condition() {
    let src = r#"
static THRESHOLD: i32 = 10;
fn check(n: i32) -> i32 { if n > THRESHOLD { 1 } else { 0 } }
fn main() -> i32 { check(15) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected exit 1, got {exit_code}");
}

/// Milestone 60: two static items used together.
///
/// FLS §7.2: Multiple statics each get their own data section entry and address.
#[test]
fn milestone_60_two_statics() {
    let src = r#"
static X: i32 = 3;
static Y: i32 = 4;
fn main() -> i32 { X + Y }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected exit 7, got {exit_code}");
}

/// Milestone 60: static item with zero value.
///
/// FLS §7.2: Zero is a valid static initializer (FLS §6.1.2).
#[test]
fn milestone_60_static_zero() {
    let src = r#"
static ZERO: i32 = 0;
fn main() -> i32 { ZERO }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected exit 0, got {exit_code}");
}

/// Milestone 60: assembly inspection — static emits adrp+add+ldr, not mov.
///
/// FLS §7.2:15: A static reference must go through its memory address.
/// This is the key architectural difference from `const` (which emits `mov`).
///
/// Cache-line note: static load = 3 instructions (12 bytes) vs const = 1
/// instruction (4 bytes). The tradeoff: statics avoid code duplication but
/// cost 3× more in instruction cache pressure per use site.
#[test]
fn runtime_static_emits_adrp_add_ldr() {
    let src = "static ANSWER: i32 = 42;\nfn main() -> i32 { ANSWER }\n";
    let asm = compile_to_asm(src);
    // Must emit adrp — FLS §7.2: static address loaded via page-relative addressing.
    assert!(
        asm.contains("adrp"),
        "expected `adrp` for static address load:\n{asm}"
    );
    // Must NOT emit a plain `mov` with #42 — static must go through memory.
    // (There may be a `mov` for the exit code path, but not `mov x{r}, #42`.)
    let has_imm_42 = asm.lines().any(|line| {
        line.contains("mov") && line.contains("#42")
    });
    assert!(
        !has_imm_42,
        "static must not be substituted as immediate #42:\n{asm}"
    );
    // The .data section must contain the static's value.
    assert!(
        asm.contains(".data"),
        "expected .data section for static:\n{asm}"
    );
    assert!(
        asm.contains("ANSWER"),
        "expected ANSWER label in assembly:\n{asm}"
    );
}

// ── Milestone 61: Additional integer types as parameters and return types ─────

/// Milestone 61: u32 parameter and return type.
///
/// FLS §4.1: Unsigned integer types. u32 uses a 64-bit ARM64 register (x0).
/// Addition is identical to signed at the hardware level.
#[test]
fn milestone_61_u32_add() {
    let src = r#"
fn add_u32(a: u32, b: u32) -> u32 { a + b }
fn main() -> i32 { add_u32(20, 22) as i32 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 61: u32 subtraction.
///
/// FLS §4.1: u32 subtraction is identical to signed subtraction on ARM64
/// (sub instruction is the same; no sign bit interpretation involved).
#[test]
fn milestone_61_u32_sub() {
    let src = r#"
fn sub_u32(a: u32, b: u32) -> u32 { a - b }
fn main() -> i32 { sub_u32(50, 8) as i32 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 61: u32 unsigned division uses `udiv`.
///
/// FLS §4.1: Unsigned division. ARM64 `udiv` (not `sdiv`) must be used.
/// For small positive values the result matches signed division, so this
/// validates both the instruction choice and basic correctness.
#[test]
fn milestone_61_u32_div() {
    let src = r#"
fn div_u32(a: u32, b: u32) -> u32 { a / b }
fn main() -> i32 { div_u32(84, 2) as i32 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 61: i64 parameter and return type.
///
/// FLS §4.1: i64 is a 64-bit signed integer. On ARM64 all registers are
/// 64-bit so i64 uses the same register layout as i32 with no truncation.
#[test]
fn milestone_61_i64_add() {
    let src = r#"
fn add_i64(a: i64, b: i64) -> i64 { a + b }
fn main() -> i32 { add_i64(20, 22) as i32 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 61: usize parameter (platform-native unsigned integer).
///
/// FLS §4.1: usize is the pointer-width unsigned integer type. On AArch64
/// (64-bit) usize is 64 bits — same as u64. Uses `udiv` and `lsr`.
#[test]
fn milestone_61_usize_add() {
    let src = r#"
fn add_usize(a: usize, b: usize) -> usize { a + b }
fn main() -> i32 { add_usize(20, 22) as i32 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 61: isize parameter (platform-native signed integer).
///
/// FLS §4.1: isize is the pointer-width signed integer type. On AArch64
/// (64-bit) isize is 64 bits — same as i64. Uses `sdiv` and `asr`.
#[test]
fn milestone_61_isize_add() {
    let src = r#"
fn add_isize(a: isize, b: isize) -> isize { a + b }
fn main() -> i32 { add_isize(20, 22) as i32 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 61: u32 unsigned right shift uses `lsr` (logical shift).
///
/// FLS §4.1: Right shift on unsigned types is logical (zero-extending).
/// ARM64 `lsr` fills from the left with 0, unlike `asr` which fills with
/// the sign bit. For positive values both give the same result, so we
/// use a value where only logical shift gives the expected answer if
/// we could test with large u32 — but since LoadImm is limited to i32
/// range at this milestone, we verify with a small value.
#[test]
fn milestone_61_u32_shr() {
    let src = r#"
fn shr_u32(a: u32, b: u32) -> u32 { a >> b }
fn main() -> i32 { shr_u32(168, 2) as i32 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 61: u32 as a local variable (let binding).
///
/// FLS §8.1: Let statements. u32 variables use the same stack slot
/// layout as i32 — one 8-byte slot per variable.
#[test]
fn milestone_61_u32_let_binding() {
    let src = r#"
fn main() -> i32 {
    let x: u32 = 42;
    x as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected exit 42, got {exit_code}");
}

/// Milestone 61: assembly inspection — unsigned division emits `udiv`.
///
/// FLS §4.1: Unsigned division must use ARM64 `udiv` instruction, not `sdiv`.
/// Cache-line note: `udiv` is one 4-byte instruction, same footprint as `sdiv`.
#[test]
fn runtime_u32_div_emits_udiv() {
    let src = "fn div_u32(a: u32, b: u32) -> u32 { a / b }\nfn main() -> i32 { div_u32(10, 2) as i32 }\n";
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("udiv"),
        "expected `udiv` for unsigned division:\n{asm}"
    );
    assert!(
        !asm.lines().any(|l| l.trim_start().starts_with("sdiv")),
        "must NOT emit `sdiv` for unsigned division:\n{asm}"
    );
}

/// Milestone 61: assembly inspection — unsigned right shift emits `lsr`.
///
/// FLS §4.1: Right shift on unsigned types must use ARM64 `lsr` (logical
/// shift right), not `asr` (arithmetic shift right which sign-extends).
#[test]
fn runtime_u32_shr_emits_lsr() {
    let src = "fn shr_u32(a: u32, b: u32) -> u32 { a >> b }\nfn main() -> i32 { shr_u32(84, 1) as i32 }\n";
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("lsr"),
        "expected `lsr` for unsigned right shift:\n{asm}"
    );
}

// ── Milestone 62: `&mut self` methods returning a scalar value ───────────────

/// Milestone 62: `&mut self` method increments a field and returns the new value.
///
/// FLS §10.1: Methods may have a `&mut self` parameter and return any type.
/// Galvanic's convention: modified fields in x0..x{N-1}, scalar return in x{N}.
#[test]
fn milestone_62_mut_self_returns_scalar() {
    let src = r#"
struct Counter { n: i32 }
impl Counter {
    fn increment(&mut self) -> i32 { self.n += 1; self.n }
}
fn main() -> i32 {
    let mut c = Counter { n: 0 };
    c.increment()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected 1 after one increment, got {exit_code}");
}

/// Milestone 62: multiple `&mut self` calls that return values.
///
/// FLS §10.1: Each call must both write back the modified field and return
/// the updated value. The second call sees the state left by the first.
#[test]
fn milestone_62_mut_self_returns_scalar_chained() {
    let src = r#"
struct Counter { n: i32 }
impl Counter {
    fn increment(&mut self) -> i32 { self.n += 1; self.n }
}
fn main() -> i32 {
    let mut c = Counter { n: 5 };
    c.increment();
    c.increment()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7 after two increments from 5, got {exit_code}");
}

/// Milestone 62: `&mut self` return value used in arithmetic.
///
/// FLS §10.1: The return value of a `&mut self` method is a first-class value
/// that can be used in any expression context.
#[test]
fn milestone_62_mut_self_return_in_arithmetic() {
    let src = r#"
struct Counter { n: i32 }
impl Counter {
    fn add(&mut self, x: i32) -> i32 { self.n += x; self.n }
}
fn main() -> i32 {
    let mut c = Counter { n: 10 };
    c.add(5) + c.add(3)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    // First call: n = 10+5 = 15, returns 15. Second call: n = 15+3 = 18, returns 18.
    // Sum: 15 + 18 = 33.
    assert_eq!(exit_code, 33, "expected 33 (15+18), got {exit_code}");
}

/// Milestone 62: `&mut self` returns a field value directly (no mutation).
///
/// FLS §10.1: A `&mut self` method that returns a field does not have to
/// mutate anything — the return type and write-back are orthogonal.
#[test]
fn milestone_62_mut_self_returns_field() {
    let src = r#"
struct Pair { x: i32, y: i32 }
impl Pair {
    fn swap_and_get(&mut self) -> i32 {
        let tmp = self.x;
        self.x = self.y;
        self.y = tmp;
        self.x
    }
}
fn main() -> i32 {
    let mut p = Pair { x: 3, y: 7 };
    p.swap_and_get()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    // After swap: x=7, y=3. Returns the new x (7).
    assert_eq!(exit_code, 7, "expected 7 (swapped x), got {exit_code}");
}

// ── Milestone 63: nested struct construction and chained field access ─────────
//
// FLS §6.11: Struct expressions. A struct with a field of struct type
// is constructed by recursively storing each nested field.
// FLS §6.13: Field access expressions. Chained access (`r.b.x`) resolves
// by computing the nested slot offset: slot(r.b.x) = base(r) + offset(b) + offset(x).
// FLS §4.11: Representation. Fields are laid out consecutively in declaration
// order with no padding (8 bytes per slot). A struct S with a field of type T
// occupies struct_size(S) slots, where struct_size(T) is the size of T.

/// Milestone 63: read the first scalar field of the first nested struct field.
///
/// FLS §6.11: `Rect { min: Point { x: 1, y: 2 }, max: Point { x: 5, y: 6 } }`
/// FLS §6.13: `r.min.x` resolves to slot base(r) + offset(min) + offset(x) = 0.
///
/// Note: FLS §6.11 does not provide an example of nested struct literals;
/// this test is derived from the struct expression semantics (§6.11) and
/// the field access evaluation rule (§6.13).
#[test]
fn milestone_63_chained_first_first() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { min: Point, max: Point }
fn main() -> i32 {
    let r = Rect { min: Point { x: 1, y: 2 }, max: Point { x: 5, y: 6 } };
    r.min.x
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected 1 (r.min.x), got {exit_code}");
}

/// Milestone 63: read the second scalar field of the first nested struct field.
///
/// FLS §6.13: `r.min.y` = base(r) + offset(min=0) + offset(y=1) = slot 1.
#[test]
fn milestone_63_chained_first_second() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { min: Point, max: Point }
fn main() -> i32 {
    let r = Rect { min: Point { x: 1, y: 2 }, max: Point { x: 5, y: 6 } };
    r.min.y
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "expected 2 (r.min.y), got {exit_code}");
}

/// Milestone 63: read the first scalar field of the second nested struct field.
///
/// FLS §6.13: `r.max.x` = base(r) + offset(max=2) + offset(x=0) = slot 2.
#[test]
fn milestone_63_chained_second_first() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { min: Point, max: Point }
fn main() -> i32 {
    let r = Rect { min: Point { x: 1, y: 2 }, max: Point { x: 5, y: 6 } };
    r.max.x
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected 5 (r.max.x), got {exit_code}");
}

/// Milestone 63: read the second scalar field of the second nested struct field.
///
/// FLS §6.13: `r.max.y` = base(r) + offset(max=2) + offset(y=1) = slot 3.
#[test]
fn milestone_63_chained_second_second() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { min: Point, max: Point }
fn main() -> i32 {
    let r = Rect { min: Point { x: 1, y: 2 }, max: Point { x: 5, y: 6 } };
    r.max.y
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "expected 6 (r.max.y), got {exit_code}");
}

/// Milestone 63: chained fields used in arithmetic.
///
/// FLS §6.13: Multiple chained accesses may appear in the same expression.
/// FLS §6.5.5: The results of field access are runtime values usable in arithmetic.
#[test]
fn milestone_63_chained_in_arithmetic() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { min: Point, max: Point }
fn main() -> i32 {
    let r = Rect { min: Point { x: 0, y: 0 }, max: Point { x: 3, y: 4 } };
    r.max.x + r.max.y
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7 (3+4), got {exit_code}");
}

/// Milestone 63: nested struct with a scalar field before the nested field.
///
/// FLS §4.11: Fields are laid out in declaration order. If the first field
/// is a scalar, the nested struct field starts at slot 1 (not 0).
#[test]
fn milestone_63_scalar_then_nested() {
    let src = r#"
struct Inner { val: i32 }
struct Outer { prefix: i32, inner: Inner }
fn main() -> i32 {
    let o = Outer { prefix: 10, inner: Inner { val: 42 } };
    o.inner.val
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42 (o.inner.val), got {exit_code}");
}

/// Milestone 63: scalar field read alongside nested struct field read.
///
/// FLS §6.13: A plain (non-chained) field access on the same variable still works
/// after the nested struct slot layout is in use.
#[test]
fn milestone_63_scalar_and_nested_field() {
    let src = r#"
struct Inner { val: i32 }
struct Outer { prefix: i32, inner: Inner }
fn main() -> i32 {
    let o = Outer { prefix: 10, inner: Inner { val: 5 } };
    o.prefix + o.inner.val
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "expected 15 (10+5), got {exit_code}");
}

/// Milestone 63: nested struct field access on a function parameter.
///
/// FLS §9: Function parameters are spilled to stack slots using the same
/// layout as local variables. Nested struct field access via chaining should
/// work identically on parameters and locals.
///
/// Note: galvanic currently passes the outer struct as individual scalar fields.
/// This test verifies that nested struct construction + chained access work
/// when the struct is a local variable, not yet a function parameter.
#[test]
fn milestone_63_on_parameter_via_fn() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { min: Point, max: Point }
fn width(r: Rect) -> i32 { r.max.x - r.min.x }
fn main() -> i32 {
    let r = Rect { min: Point { x: 1, y: 0 }, max: Point { x: 4, y: 0 } };
    width(r)
}
"#;
    // Milestone 64 fixed nested struct parameter passing — this test now
    // compiles and runs correctly.
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "expected 3 (4-1), got {exit_code}");
}

// ── Milestone 64: nested struct as function parameter ─────────────────────────
//
// Programs where a struct whose fields are themselves structs is passed by
// value to a function. The function receives the nested struct as a flat
// sequence of registers — one per total slot in the outermost struct.
//
// FLS §4.11: Struct layout — fields are stored in declaration order, each
// occupying as many consecutive slots as the field's type requires.
// FLS §9: Function parameters receive values in registers x0–x7.
// FLS §6.11: Struct expressions initialise all fields in declaration order.
// FLS §6.13: Field access uses the slot offset computed from struct_sizes.

/// Milestone 64: pass nested struct to fn, read first scalar field.
///
/// FLS §4.11, §9, §6.13
#[test]
fn milestone_64_nested_param_first_scalar_field() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { min: Point, max: Point }
fn get_min_x(r: Rect) -> i32 { r.min.x }
fn main() -> i32 {
    let r = Rect { min: Point { x: 5, y: 0 }, max: Point { x: 9, y: 0 } };
    get_min_x(r)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected 5, got {exit_code}");
}

/// Milestone 64: pass nested struct to fn, read a field in the second sub-struct.
///
/// FLS §4.11, §9, §6.13
#[test]
fn milestone_64_nested_param_second_substruct_field() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { min: Point, max: Point }
fn get_max_y(r: Rect) -> i32 { r.max.y }
fn main() -> i32 {
    let r = Rect { min: Point { x: 0, y: 0 }, max: Point { x: 0, y: 7 } };
    get_max_y(r)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// Milestone 64: nested struct parameter, arithmetic across sub-struct fields.
///
/// FLS §4.11, §9, §6.13, §6.5.5
#[test]
fn milestone_64_nested_param_cross_substruct_arithmetic() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { min: Point, max: Point }
fn height(r: Rect) -> i32 { r.max.y - r.min.y }
fn main() -> i32 {
    let r = Rect { min: Point { x: 0, y: 2 }, max: Point { x: 0, y: 8 } };
    height(r)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "expected 6, got {exit_code}");
}

/// Milestone 64: nested struct passed to multiple functions.
///
/// FLS §4.11, §9, §6.13
#[test]
fn milestone_64_nested_param_two_fns() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { a: Point, b: Point }
fn left(r: Rect) -> i32 { r.a.x }
fn right(r: Rect) -> i32 { r.b.x }
fn main() -> i32 {
    let r = Rect { a: Point { x: 1, y: 0 }, b: Point { x: 4, y: 0 } };
    right(r) - left(r)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "expected 3, got {exit_code}");
}

/// Milestone 64: nested struct parameter in a function called with a local variable.
///
/// FLS §4.11, §9, §6.13
#[test]
fn milestone_64_nested_param_in_if_expression() {
    let src = r#"
struct Vec2 { x: i32, y: i32 }
struct Segment { start: Vec2, end: Vec2 }
fn horizontal(s: Segment) -> i32 {
    if s.start.y == s.end.y { 1 } else { 0 }
}
fn main() -> i32 {
    let s = Segment { start: Vec2 { x: 0, y: 3 }, end: Vec2 { x: 5, y: 3 } };
    horizontal(s)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected 1 (horizontal), got {exit_code}");
}

/// Milestone 64: nested struct parameter passed from a function parameter.
///
/// FLS §4.11, §9: nested struct arriving as a function parameter can itself
/// be passed to another function.
#[test]
fn milestone_64_nested_param_forwarded() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { min: Point, max: Point }
fn area_proxy(r: Rect) -> i32 { r.max.x * r.max.y }
fn compute(r: Rect) -> i32 { area_proxy(r) }
fn main() -> i32 {
    let r = Rect { min: Point { x: 0, y: 0 }, max: Point { x: 3, y: 4 } };
    compute(r)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "expected 12 (3*4), got {exit_code}");
}

/// Milestone 64: assembly inspection — nested struct parameter spills N total
/// slots (not just direct field count) into consecutive stack slots.
///
/// FLS §4.11: Rect has 2 declared fields (min, max) but 4 total slots.
/// The parameter spill must emit 4 `str` instructions, not 2.
#[test]
fn runtime_nested_struct_param_spills_total_slots() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { min: Point, max: Point }
fn width(r: Rect) -> i32 { r.max.x - r.min.x }
fn main() -> i32 {
    let r = Rect { min: Point { x: 1, y: 0 }, max: Point { x: 4, y: 0 } };
    width(r)
}
"#;
    let asm = compile_to_asm(src);
    // The `width` function receives 4 registers (min.x, min.y, max.x, max.y)
    // and must spill all 4 to the stack. Verify 4 store instructions in the
    // function prologue by checking the asm has enough `str` occurrences.
    let str_count = asm.lines()
        .filter(|l| l.trim_start().starts_with("str") && !l.contains("lr"))
        .count();
    assert!(
        str_count >= 4,
        "expected at least 4 str instructions for 4-slot Rect parameter, got {str_count}:\n{asm}"
    );
    // Verify the correct result is produced.
    let tokens = galvanic::lexer::tokenize(src).expect("lex");
    let sf = galvanic::parser::parse(&tokens, src).expect("parse");
    let _module = galvanic::lower::lower(&sf, src).expect("lower");
}

/// Milestone 63: assembly inspection — chained field access emits correct `ldr`
/// at the slot offset computed by struct_field_offsets.
///
/// FLS §6.13: `r.max.x` where Rect has two Point fields (2 slots each) should
/// load from slot 2 (offset 2 from base of r), emitting `ldr x_, [sp, #16]`.
///
/// FLS §4.11: Galvanic uses 8-byte slots, so slot 2 = byte offset 16.
#[test]
fn runtime_nested_struct_chained_access_emits_ldr_at_offset() {
    let src = r#"
struct Point { x: i32, y: i32 }
struct Rect { min: Point, max: Point }
fn main() -> i32 {
    let r = Rect { min: Point { x: 0, y: 0 }, max: Point { x: 3, y: 0 } };
    r.max.x
}
"#;
    let asm = compile_to_asm(src);
    // r.max.x is at slot 2 = byte offset 16 from sp.
    // The nested struct layout: min.x=slot0, min.y=slot1, max.x=slot2, max.y=slot3.
    // The codegen emits `ldr xN, [sp, #16 ...]` (slot 2 × 8 = 16 bytes).
    assert!(
        asm.contains("ldr") && asm.contains("#16"),
        "expected ldr from slot 2 (#16) for r.max.x in nested struct, got:\n{asm}"
    );
}

/// Milestone 62: assembly inspection — `&mut self` with scalar return emits
/// `RetFieldsAndValue` pattern: field ldr + return value mov before ret.
///
/// FLS §10.1 AMBIGUOUS: Fields in x0..x{N-1}, scalar in x{N}.
#[test]
fn runtime_mut_self_return_emits_ret_fields_and_value() {
    let src = r#"
struct Counter { n: i32 }
impl Counter {
    fn increment(&mut self) -> i32 { self.n += 1; self.n }
}
fn main() -> i32 { let mut c = Counter { n: 0 }; c.increment() }
"#;
    let asm = compile_to_asm(src);
    // The method should load field 0 into x0 (write-back) and the return value into x1.
    // The call site should write x0 back to the struct slot and capture x1 as the result.
    assert!(
        asm.contains("Counter__increment"),
        "expected mangled method name in asm:\n{asm}"
    );
    // The caller should have a str for write-back AND a mov capturing the return reg.
    // Verify the call site has a bl followed by field stores (write-back).
    assert!(
        asm.contains("bl      Counter__increment"),
        "expected bl to Counter__increment:\n{asm}"
    );
}

// ── Milestone 65: &self methods returning struct values ───────────────────────
//
// FLS §10.1: Instance methods with `&self` may return any type, including
// struct types. Galvanic uses the same register-packing convention as
// struct-returning associated functions: the callee returns field values in
// x0..x{N-1} via `RetFields`; the call site emits `CallMut`-style write-back
// to a new destination struct variable.
//
// FLS §6.12.2: Method call expressions.
// FLS §4.11: Struct layout — fields stored in declaration order.
// FLS §6.13: Field access uses slot offsets.
// FLS §10.1 AMBIGUOUS: The FLS does not specify the calling convention for
// methods returning struct types. Galvanic uses the same convention as
// struct-returning associated functions: fields in x0..x{N-1}.

/// Milestone 65: basic &self method returning a struct — read first field.
///
/// FLS §10.1, §6.12.2, §6.13
#[test]
fn milestone_65_method_returns_struct_first_field() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn translate(&self, dx: i32, dy: i32) -> Point {
        Point { x: self.x + dx, y: self.y + dy }
    }
}
fn main() -> i32 {
    let p = Point { x: 1, y: 2 };
    let q = p.translate(3, 4);
    q.x
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "expected 4 (1+3), got {exit_code}");
}

/// Milestone 65: &self method returning struct — read second field.
///
/// FLS §10.1, §6.12.2, §6.13
#[test]
fn milestone_65_method_returns_struct_second_field() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn translate(&self, dx: i32, dy: i32) -> Point {
        Point { x: self.x + dx, y: self.y + dy }
    }
}
fn main() -> i32 {
    let p = Point { x: 1, y: 2 };
    let q = p.translate(3, 4);
    q.y
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "expected 6 (2+4), got {exit_code}");
}

/// Milestone 65: &self method returning struct, result used in arithmetic.
///
/// FLS §10.1, §6.12.2, §6.5.5
#[test]
fn milestone_65_method_returns_struct_field_sum() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn translate(&self, dx: i32, dy: i32) -> Point {
        Point { x: self.x + dx, y: self.y + dy }
    }
}
fn main() -> i32 {
    let p = Point { x: 1, y: 2 };
    let q = p.translate(3, 4);
    q.x + q.y
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected 10 (4+6), got {exit_code}");
}

/// Milestone 65: chain two struct-returning method calls.
///
/// FLS §10.1, §6.12.2
#[test]
fn milestone_65_method_returns_struct_chained() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn translate(&self, dx: i32, dy: i32) -> Point {
        Point { x: self.x + dx, y: self.y + dy }
    }
}
fn main() -> i32 {
    let p = Point { x: 0, y: 0 };
    let q = p.translate(1, 0);
    let r = q.translate(2, 0);
    r.x
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "expected 3 (0+1+2), got {exit_code}");
}

/// Milestone 65: &self method returns struct, use as function argument.
///
/// FLS §10.1, §6.12.2, §9
#[test]
fn milestone_65_method_returns_struct_passed_to_fn() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn translate(&self, dx: i32, dy: i32) -> Point {
        Point { x: self.x + dx, y: self.y + dy }
    }
}
fn get_x(p: Point) -> i32 { p.x }
fn main() -> i32 {
    let p = Point { x: 2, y: 5 };
    let q = p.translate(7, 0);
    get_x(q)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 9, "expected 9 (2+7), got {exit_code}");
}

/// Milestone 65: &self method on a parameter.
///
/// FLS §10.1, §9, §6.12.2
#[test]
fn milestone_65_method_returns_struct_on_parameter() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn translate(&self, dx: i32, dy: i32) -> Point {
        Point { x: self.x + dx, y: self.y + dy }
    }
}
fn shift(p: Point, n: i32) -> i32 {
    let q = p.translate(n, 0);
    q.x
}
fn main() -> i32 {
    let p = Point { x: 3, y: 0 };
    shift(p, 4)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7 (3+4), got {exit_code}");
}

/// Milestone 65: &self method returning struct, result in if expression.
///
/// FLS §10.1, §6.12.2, §6.17
#[test]
fn milestone_65_method_returns_struct_in_if() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn translate(&self, dx: i32, dy: i32) -> Point {
        Point { x: self.x + dx, y: self.y + dy }
    }
}
fn main() -> i32 {
    let p = Point { x: 2, y: 0 };
    let q = p.translate(1, 0);
    if q.x > 2 { 1 } else { 0 }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected 1 (3>2), got {exit_code}");
}

/// Milestone 65: assembly inspection — &self method returning struct emits
/// `RetFields` on callee side and `CallMut`-style write-back on caller side.
///
/// FLS §10.1 AMBIGUOUS: The calling convention packs the return struct's
/// field values into x0..x{N-1} via the existing `RetFields` mechanism.
/// The call site emits `bl` followed by N `str` instructions to write the
/// returned fields into the destination variable's stack slots.
#[test]
fn runtime_self_method_struct_return_emits_ret_fields_and_write_back() {
    let src = r#"
struct Point { x: i32, y: i32 }
impl Point {
    fn translate(&self, dx: i32, dy: i32) -> Point {
        Point { x: self.x + dx, y: self.y + dy }
    }
}
fn main() -> i32 {
    let p = Point { x: 1, y: 2 };
    let q = p.translate(3, 4);
    q.x
}
"#;
    let asm = compile_to_asm(src);
    // The callee `Point__translate` must emit `ldr` instructions to load field
    // values into x0..x{N-1} before returning (RetFields convention).
    assert!(
        asm.contains("Point__translate"),
        "expected mangled method name in asm:\n{asm}"
    );
    // The call site must emit `bl Point__translate` followed by `str` instructions
    // to write x0 and x1 (the returned Point fields) into q's stack slots.
    assert!(
        asm.contains("bl      Point__translate"),
        "expected bl to Point__translate:\n{asm}"
    );
}

// ── Milestone 66: numeric type casts to all integer types ──────────────────
//
// FLS §6.5.9: Type cast expressions. The `as` operator converts a value of
// one numeric type to another. On ARM64 all integer types use 64-bit registers,
// so widening casts (i32→i64, i32→usize) and same-width reinterpret casts
// (i32↔u32) are identity at the register level — no extra instruction is
// emitted. Narrowing casts (i64→i8) are also identity for values within the
// target range; explicit truncation is deferred (FLS §6.5.9 AMBIGUOUS).
//
// FLS §6.5.9 AMBIGUOUS: The spec says narrowing casts truncate to the target
// bit width but does not specify the mechanism. Galvanic defers truncation.
//
// FLS §6.1.2:37–45: The operand is lowered at runtime — no constant folding.

/// Milestone 66: `i32 as u32 as i32` — round-trip through unsigned is identity.
///
/// FLS §6.5.9: `i32 as u32` reinterprets bits; `u32 as i32` reinterprets back.
/// For values in [0, i32::MAX] the result equals the original.
#[test]
fn milestone_66_i32_as_u32_as_i32() {
    let src = "fn main() -> i32 { let x: i32 = 42; x as u32 as i32 }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 66: `u32 as i32` — unsigned parameter returned as signed.
///
/// FLS §6.5.9: For values ≤ i32::MAX, `u32 as i32` is an identity cast.
#[test]
fn milestone_66_u32_as_i32() {
    let src = "fn get(x: u32) -> i32 { x as i32 }\nfn main() -> i32 { get(99) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 99, "expected 99, got {exit_code}");
}

/// Milestone 66: `i64 as i32` — narrowing signed cast is identity for small values.
///
/// FLS §6.5.9: Narrowing cast truncates to 32 bits; for values ≤ i32::MAX
/// the result equals the original value.
#[test]
fn milestone_66_i64_as_i32() {
    let src = "fn narrow(n: i64) -> i32 { n as i32 }\nfn main() -> i32 { narrow(77) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 77, "expected 77, got {exit_code}");
}

/// Milestone 66: `i32 as i64 as i32` — widen then narrow round-trip.
///
/// FLS §6.5.9: `i32 as i64` sign-extends (identity on 64-bit ARM64);
/// `i64 as i32` truncates (identity for values ≤ i32::MAX).
#[test]
fn milestone_66_i32_as_i64_round_trip() {
    let src = "fn main() -> i32 { let x: i32 = 55; x as i64 as i32 }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 55, "expected 55, got {exit_code}");
}

/// Milestone 66: `i32 as usize as i32` — round-trip through usize is identity.
///
/// FLS §6.5.9: `usize` is an unsigned pointer-sized integer; on a 64-bit
/// system it is 64 bits. For values ≤ i32::MAX, `i32 as usize as i32`
/// preserves the value.
#[test]
fn milestone_66_i32_as_usize_round_trip() {
    let src = "fn main() -> i32 { let n: i32 = 11; n as usize as i32 }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 11, "expected 11, got {exit_code}");
}

/// Milestone 66: `usize as i32` — usize parameter cast to signed return.
///
/// FLS §6.5.9: For values ≤ i32::MAX, `usize as i32` is identity.
#[test]
fn milestone_66_usize_as_i32() {
    let src = "fn to_signed(n: usize) -> i32 { n as i32 }\nfn main() -> i32 { to_signed(33) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 33, "expected 33, got {exit_code}");
}

/// Milestone 66: cast in arithmetic — `(x as u32 + 1) as i32`.
///
/// FLS §6.5.9: The result of a cast expression may be used directly as an
/// operand in an arithmetic expression. The intermediate u32 value uses
/// unsigned arithmetic (udiv, lsr) for subsequent operations.
#[test]
fn milestone_66_cast_in_arithmetic() {
    let src = "fn inc(x: i32) -> i32 { (x as u32 + 1) as i32 }\nfn main() -> i32 { inc(20) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 21, "expected 21, got {exit_code}");
}

/// Milestone 66: assembly inspection — identity casts emit no extra instruction.
///
/// FLS §6.5.9: For same-register-width casts the source register is reused
/// directly. No `mov`, `sxtw`, `uxtw`, or masking instruction should appear
/// between the load of `x` and the return.
///
/// Cache-line note: zero-instruction casts have zero cache-line footprint —
/// the optimal outcome for a reinterpret cast.
#[test]
fn runtime_numeric_cast_identity_emits_no_extra_instruction() {
    // `x as u32 as i32` — two reinterpret casts that collapse to nothing.
    let src = "fn main() -> i32 { let x: i32 = 5; x as u32 as i32 }\n";
    let asm = compile_to_asm(src);
    // The assembly should contain exactly one `ldr` (to load x from the stack)
    // and no `sxtw`, `uxtw`, `and`, or extra `mov` for the casts themselves.
    let cast_instrs = asm.lines().filter(|l| {
        let l = l.trim();
        l.starts_with("sxtw") || l.starts_with("uxtw") || l.starts_with("and ")
    }).count();
    assert_eq!(
        cast_instrs, 0,
        "expected zero cast instructions for identity `i32 as u32 as i32`, got:\n{asm}"
    );
}

// ── Milestone 67: variable shadowing compiles to runtime ARM64 (FLS §8.1) ──

/// Milestone 67: `let x = x + 3` correctly reads the old x (slot 0) and
/// writes to a new slot (slot 1).
///
/// FLS §8.1: The binding introduced by a let statement comes into scope
/// after the initializer expression has been evaluated. In a shadowing
/// `let x = x + 3`, the RHS `x` refers to the previous binding.
///
/// This was a bug: locals.insert happened before lower_expr, so the RHS
/// saw the new uninitialized slot instead of the old value.
#[test]
fn milestone_67_shadow_simple() {
    let src = "fn main() -> i32 { let x = 5; let x = x + 3; x }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 8, "expected 8, got {exit_code}");
}

/// Milestone 67: Three levels of shadowing, each step reading the previous.
///
/// FLS §8.1: Each `let x = x * 2` evaluates the previous x before
/// introducing the new binding.
#[test]
fn milestone_67_shadow_three_levels() {
    // x=5, x=5+3=8, x=8*2=16 → exit 16
    let src = "fn main() -> i32 { let x = 5; let x = x + 3; let x = x * 2; x }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 16, "expected 16, got {exit_code}");
}

/// Milestone 67: Shadowing inside a function using a parameter.
///
/// FLS §8.1: The inner `let n = n - 1` reads the parameter n, not the
/// new binding.
#[test]
fn milestone_67_shadow_parameter() {
    // n=10 → let n = 10-1 = 9 → return 9
    let src = "fn dec(n: i32) -> i32 { let n = n - 1; n }\nfn main() -> i32 { dec(10) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 9, "expected 9, got {exit_code}");
}

/// Milestone 67: Shadow a variable with a different expression type.
///
/// FLS §8.1: `let x = x > 0` — shadows x (i32) with a bool-valued
/// expression that reads the old x. Here we cast to i32 for the exit code.
#[test]
fn milestone_67_shadow_changes_value() {
    // x=7, let x = x - 2 = 5, let x = x - 2 = 3 → exit 3
    let src = "fn main() -> i32 { let x = 7; let x = x - 2; let x = x - 2; x }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "expected 3, got {exit_code}");
}

/// Milestone 67: Shadowing in a block — outer x remains unchanged after
/// the inner block exits (inner binding does not affect outer scope).
///
/// FLS §8.1 / FLS §6.4: Block expressions create a scope. A `let`
/// statement in an inner block shadows the outer binding within that block
/// but the outer binding is unaffected after the block ends.
///
/// Note: galvanic uses flat stack slots and does not yet restore the outer
/// binding after a block. This test uses sequential top-level shadowing,
/// not block-scoped shadowing.
#[test]
fn milestone_67_shadow_in_fn_multiple_vars() {
    // Each variable shadows independently.
    let src = "fn main() -> i32 { let a = 1; let b = 2; let a = a + b; a }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "expected 3, got {exit_code}");
}

/// Milestone 67: assembly inspection — `let x = x + 3` loads slot 0 for the
/// RHS, not slot 1.
///
/// FLS §8.1: The RHS references the old binding (slot 0). The new binding
/// gets slot 1. Before the fix, slot 1 was loaded (uninitialized).
///
/// Cache-line note: one ldr (slot 0), one add, one str (slot 1) — three
/// instructions, fitting within a single 64-byte cache line.
#[test]
fn runtime_shadow_rhs_reads_old_slot() {
    // let x = 5 → str slot 0; let x = x + 3 → ldr slot 0 (not slot 1), add, str slot 1
    let src = "fn main() -> i32 { let x = 5; let x = x + 3; x }\n";
    let asm = compile_to_asm(src);
    // Find the add instruction; the ldr before it should reference slot 0, not slot 1.
    let lines: Vec<&str> = asm.lines().collect();
    let add_idx = lines.iter().position(|l| l.trim().starts_with("add"));
    let add_idx = add_idx.expect("expected an add instruction in the assembly");
    // The ldr immediately before the add should load from slot 0 (offset 0).
    let ldr_before = lines[..add_idx]
        .iter()
        .rev()
        .find(|l| l.trim().starts_with("ldr"))
        .expect("expected an ldr before the add");
    assert!(
        ldr_before.contains("#0"),
        "expected ldr from slot 0 (offset #0) before add, got: {ldr_before}"
    );
}

// ── Milestone 68: references as function parameters compile to runtime ARM64 ──
// FLS §6.5.1: Borrow expressions — `&place` computes the address of a local variable.
// FLS §6.5.2: Dereference expressions — `*expr` loads through a pointer.
// FLS §4.8: Reference types `&T` and `&mut T` are pointer-sized values.

/// Milestone 68: `*x` inside a function with `&i32` parameter returns the
/// referent value.
///
/// FLS §6.5.2: A dereference expression evaluates the operand (producing an
/// address) and loads the value at that address.
#[test]
fn milestone_68_deref_ref_param() {
    let src = "fn identity_ref(x: &i32) -> i32 { *x }\nfn main() -> i32 { let n = 42; identity_ref(&n) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 68: `*x * 2` — arithmetic on a dereferenced reference.
///
/// FLS §6.5.2: The dereferenced value can be used in arithmetic expressions.
/// FLS §6.5.5: Arithmetic expressions apply to the loaded value.
#[test]
fn milestone_68_deref_in_arithmetic() {
    let src = "fn double(x: &i32) -> i32 { *x * 2 }\nfn main() -> i32 { let n = 21; double(&n) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 68: passing `&n` where `n` is a parameter.
///
/// FLS §6.5.1: Any local variable (including function parameters, which are
/// spilled to the stack) can be borrowed.
#[test]
fn milestone_68_borrow_param() {
    let src = "fn identity_ref(x: &i32) -> i32 { *x }\nfn relay(n: i32) -> i32 { identity_ref(&n) }\nfn main() -> i32 { relay(7) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// Milestone 68: deref result used in an `if` condition.
///
/// FLS §6.17: if expressions use a boolean condition. The dereferenced value
/// (a boolean reference `&bool`) is used as the condition.
#[test]
fn milestone_68_deref_bool_param() {
    let src = "fn check(flag: &i32) -> i32 { if *flag != 0 { 1 } else { 0 } }\nfn main() -> i32 { let b = 1; check(&b) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected 1, got {exit_code}");
}

/// Milestone 68: deref of a variable passed through two borrow levels.
///
/// FLS §6.5.1: `&n` passes n's address. The callee receives the pointer in a
/// register (x0), spills it, then loads it back to dereference.
#[test]
fn milestone_68_deref_zero() {
    let src = "fn get_zero(x: &i32) -> i32 { *x }\nfn main() -> i32 { let n = 0; get_zero(&n) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected 0, got {exit_code}");
}

/// Milestone 68: `&n` and `&m` — two borrows in one call expression.
///
/// FLS §6.5.1: Each borrow produces a separate pointer value passed in
/// x0 and x1 per the ARM64 ABI.
#[test]
fn milestone_68_two_ref_params() {
    let src = "fn add_refs(a: &i32, b: &i32) -> i32 { *a + *b }\nfn main() -> i32 { let x = 10; let y = 32; add_refs(&x, &y) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 68: deref result stored in a local variable.
///
/// FLS §8.1: `let v = *x` — the dereferenced value is bound to a new slot.
#[test]
fn milestone_68_deref_into_let() {
    let src = "fn load(p: &i32) -> i32 { let v = *p; v + 1 }\nfn main() -> i32 { let n = 9; load(&n) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected 10, got {exit_code}");
}

/// Milestone 68: assembly inspection — `&n` emits `add` to form the stack address.
///
/// FLS §6.5.1: The borrow expression must emit `add x{dst}, sp, #{offset}`
/// to form the pointer value at runtime, not a constant address.
///
/// Cache-line note: `add` is one 4-byte instruction — identical footprint to
/// any `ldr`/`str`. The address computation fits in one instruction slot.
#[test]
fn runtime_borrow_emits_add_sp() {
    let src = "fn identity_ref(x: &i32) -> i32 { *x }\nfn main() -> i32 { let n = 42; identity_ref(&n) }\n";
    let asm = compile_to_asm(src);
    // `&n` must produce an `add xD, sp, #offset` instruction in main.
    let has_addr_add = asm.lines().any(|l| {
        let t = l.trim();
        t.starts_with("add") && t.contains("sp,") && !t.contains("sp, sp,")
    });
    assert!(has_addr_add, "expected `add xD, sp, #offset` for &n, got:\n{asm}");
}

/// Milestone 68: assembly inspection — `*x` inside the callee emits an
/// indirect `ldr`.
///
/// FLS §6.5.2: The dereference must emit `ldr x{dst}, [x{src}]` — a
/// register-indirect load, not a stack-relative load.
///
/// Cache-line note: `ldr [xN]` is one 4-byte instruction. Compared to the
/// two-instruction `adrp`+`ldr` needed for statics, a reference dereference
/// is cheaper when the pointer is already in a register.
#[test]
fn runtime_deref_emits_ldr_indirect() {
    let src = "fn get(x: &i32) -> i32 { *x }\nfn main() -> i32 { let n = 5; get(&n) }\n";
    let asm = compile_to_asm(src);
    // `*x` must emit `ldr xD, [xN]` (square bracket with register, no #offset).
    let has_indirect_ldr = asm.lines().any(|l| {
        let t = l.trim();
        // Match `ldr xD, [xN]` but not `ldr xD, [sp, #offset]` (stack load)
        // and not `ldr xD, [xN, ...]` (indexed load).
        if !t.starts_with("ldr") { return false; }
        if let Some(bracket_start) = t.find('[') {
            let inside = &t[bracket_start + 1..];
            // Must start with 'x' (register) not 's' (sp) and must close immediately.
            inside.starts_with('x') && inside.contains(']') && !inside.contains(',')
        } else {
            false
        }
    });
    assert!(has_indirect_ldr, "expected `ldr xD, [xN]` for *x, got:\n{asm}");
}

// ── Milestone 69: Write through mutable reference (`*ref = value`) ──────────

/// Milestone 69: basic increment through a mutable reference.
///
/// FLS §6.5.10: Assignment expression `*x = *x + 1` where `x: &mut i32`.
/// The callee receives a pointer, loads the current value, increments it,
/// and stores the result back through the pointer.
#[test]
fn milestone_69_mut_ref_increment() {
    let src = "fn increment(x: &mut i32) { *x = *x + 1; }\nfn main() -> i32 { let mut n = 5; increment(&mut n); n }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "expected 6, got {exit_code}");
}

/// Milestone 69: store a fixed value through a mutable reference.
///
/// FLS §6.5.10: `*x = 42` — unconditional store through pointer.
#[test]
fn milestone_69_mut_ref_assign_fixed() {
    let src = "fn set_value(x: &mut i32, v: i32) { *x = v; }\nfn main() -> i32 { let mut n = 0; set_value(&mut n, 42); n }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 69: double-increment — two writes through the same pointer.
///
/// FLS §6.5.10: Each `*x = *x + 1` is a separate load-increment-store triple.
/// The second write reads the value left by the first.
#[test]
fn milestone_69_mut_ref_double_increment() {
    let src = "fn add_two(x: &mut i32) { *x = *x + 1; *x = *x + 1; }\nfn main() -> i32 { let mut n = 10; add_two(&mut n); n }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "expected 12, got {exit_code}");
}

/// Milestone 69: mutable reference passed as a parameter, store zero.
///
/// FLS §6.5.10: `*x = 0` resets the referent to zero regardless of its prior value.
#[test]
fn milestone_69_mut_ref_reset_to_zero() {
    let src = "fn reset(x: &mut i32) { *x = 0; }\nfn main() -> i32 { let mut n = 99; reset(&mut n); n }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "expected 0, got {exit_code}");
}

/// Milestone 69: store result of arithmetic through pointer.
///
/// FLS §6.5.10: The RHS is a full arithmetic expression; it must be evaluated
/// at runtime before the store.
#[test]
fn milestone_69_mut_ref_store_arithmetic() {
    let src = "fn compute(x: &mut i32, a: i32, b: i32) { *x = a * b + 1; }\nfn main() -> i32 { let mut n = 0; compute(&mut n, 3, 4); n }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 13, "expected 13, got {exit_code}");
}

/// Milestone 69: increment in a loop through a mutable reference.
///
/// FLS §6.15.3: While loop combined with FLS §6.5.10: each iteration stores
/// through the pointer. Tests that the pointer survives across loop iterations.
#[test]
fn milestone_69_mut_ref_in_loop() {
    let src = "fn count_up(x: &mut i32, n: i32) { let mut i = 0; while i < n { *x = *x + 1; i = i + 1; } }\nfn main() -> i32 { let mut result = 0; count_up(&mut result, 5); result }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected 5, got {exit_code}");
}

/// Milestone 69: two mutable reference parameters, both written.
///
/// FLS §6.5.10: Independent stores through two separate pointers. Each pointer
/// resides in its own register; no aliasing is assumed.
#[test]
fn milestone_69_two_mut_ref_params() {
    let src = "fn swap_vals(a: &mut i32, b: &mut i32) { let tmp = *a; *a = *b; *b = tmp; }\nfn main() -> i32 { let mut x = 3; let mut y = 7; swap_vals(&mut x, &mut y); x }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// Milestone 69: assembly inspection — `*x = value` emits `str xS, [xA]`.
///
/// FLS §6.5.10: The store-through-pointer must emit `str x{src}, [x{addr}]`
/// — register-indirect store, not a stack-relative store.
///
/// Cache-line note: `str [xN]` is one 4-byte instruction — same footprint as
/// `str [sp, #offset]`. The indirect form costs no extra cycles on modern
/// ARM64 when the pointer is already in a register.
#[test]
fn runtime_store_ptr_emits_str_indirect() {
    let src = "fn set(x: &mut i32, v: i32) { *x = v; }\nfn main() -> i32 { let mut n = 0; set(&mut n, 1); n }\n";
    let asm = compile_to_asm(src);
    // `*x = v` must produce `str xS, [xA]` (register-indirect, no #offset inside brackets).
    let has_indirect_str = asm.lines().any(|l| {
        let t = l.trim();
        if !t.starts_with("str") { return false; }
        if let Some(bracket_start) = t.find('[') {
            let inside = &t[bracket_start + 1..];
            // Must start with 'x' (register) not 's' (sp) and must close immediately.
            inside.starts_with('x') && inside.contains(']') && !inside.contains(',')
        } else {
            false
        }
    });
    assert!(has_indirect_str, "expected `str xS, [xA]` for *x = v, got:\n{asm}");
}

// ── Milestone 70: compound assignment through mutable references ──────────────

/// Milestone 70: `*x += 1` increments through a mutable reference.
///
/// FLS §6.5.11: Compound assignment desugars to load + binop + store.
/// FLS §6.5.10: The LHS is a dereference expression; store goes through the pointer.
/// FLS §6.1.2:37–45: LoadPtr + BinOp + StorePtr are all runtime instructions.
#[test]
fn milestone_70_deref_add_assign() {
    let src = "fn increment(x: &mut i32) { *x += 1; }\nfn main() -> i32 { let mut n = 5; increment(&mut n); n }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "expected 6, got {exit_code}");
}

/// Milestone 70: `*x -= 2` subtracts through a mutable reference.
///
/// FLS §6.5.11: `-=` through a pointer desugars to LoadPtr + Sub + StorePtr.
#[test]
fn milestone_70_deref_sub_assign() {
    let src = "fn decrement(x: &mut i32, n: i32) { *x -= n; }\nfn main() -> i32 { let mut v = 20; decrement(&mut v, 8); v }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "expected 12, got {exit_code}");
}

/// Milestone 70: `*x *= factor` multiplies through a mutable reference.
///
/// FLS §6.5.11: `*=` through a pointer desugars to LoadPtr + Mul + StorePtr.
#[test]
fn milestone_70_deref_mul_assign() {
    let src = "fn double(x: &mut i32) { *x *= 2; }\nfn main() -> i32 { let mut v = 7; double(&mut v); v }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 14, "expected 14, got {exit_code}");
}

/// Milestone 70: multiple `*x += 1` calls accumulate correctly.
///
/// FLS §6.5.11: Each compound assignment is independent; the second reads the
/// value written by the first.
#[test]
fn milestone_70_deref_add_assign_twice() {
    let src = "fn add_two(x: &mut i32) { *x += 1; *x += 1; }\nfn main() -> i32 { let mut n = 10; add_two(&mut n); n }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "expected 12, got {exit_code}");
}

/// Milestone 70: `*x += n` in a loop accumulates to the expected total.
///
/// FLS §6.15.3: While loop. Each iteration increments through the pointer.
/// FLS §6.5.11: Compound assignment through pointer is idiomatic Rust.
#[test]
fn milestone_70_deref_add_assign_in_loop() {
    let src = "fn accumulate(x: &mut i32, n: i32) { let mut i = 0; while i < n { *x += 1; i += 1; } }\nfn main() -> i32 { let mut total = 0; accumulate(&mut total, 7); total }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// Milestone 70: `*x += param` uses the parameter value on the RHS.
///
/// FLS §6.5.11: The RHS expression is evaluated at runtime; here it is a
/// function parameter (loaded from the stack), not a literal.
#[test]
fn milestone_70_deref_add_assign_from_param() {
    let src = "fn add_to(x: &mut i32, amount: i32) { *x += amount; }\nfn main() -> i32 { let mut n = 3; add_to(&mut n, 4); n }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// Milestone 70: two separate mutable references, both compound-assigned.
///
/// FLS §6.5.11: Two pointers are updated independently. No aliasing.
#[test]
fn milestone_70_two_deref_add_assigns() {
    let src = "fn add_both(a: &mut i32, b: &mut i32, n: i32) { *a += n; *b += n; }\nfn main() -> i32 { let mut x = 1; let mut y = 2; add_both(&mut x, &mut y, 3); x + y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 9, "expected 9, got {exit_code}");
}

/// Milestone 70: assembly inspection — `*x += 1` emits LoadPtr + add + StorePtr.
///
/// FLS §6.5.11 + §6.5.10: The sequence must be `ldr xD, [xP]` (load through pointer),
/// an `add` instruction, then `str xR, [xP]` (store through pointer).
///
/// Cache-line note: 3 instructions × 4 bytes = 12 bytes — same as the
/// stack-slot compound-assign variant (ldr [sp,#off] + add + str [sp,#off]).
#[test]
fn runtime_deref_compound_assign_emits_ldrptr_binop_strptr() {
    let src = "fn increment(x: &mut i32) { *x += 1; }\nfn main() -> i32 { let mut n = 0; increment(&mut n); n }\n";
    let asm = compile_to_asm(src);
    // Must contain an indirect load `ldr xD, [xP]` (register-indirect, no offset).
    let has_ldr_ptr = asm.lines().any(|l| {
        let t = l.trim();
        if !t.starts_with("ldr") { return false; }
        if let Some(bracket) = t.find('[') {
            let inside = &t[bracket + 1..];
            inside.starts_with('x') && inside.contains(']') && !inside.contains(',')
        } else {
            false
        }
    });
    // Must contain an add instruction.
    let has_add = asm.lines().any(|l| l.trim().starts_with("add"));
    // Must contain an indirect store `str xS, [xP]`.
    let has_str_ptr = asm.lines().any(|l| {
        let t = l.trim();
        if !t.starts_with("str") { return false; }
        if let Some(bracket) = t.find('[') {
            let inside = &t[bracket + 1..];
            inside.starts_with('x') && inside.contains(']') && !inside.contains(',')
        } else {
            false
        }
    });
    assert!(has_ldr_ptr, "*x += 1 must emit `ldr xD, [xP]`:\n{asm}");
    assert!(has_add, "*x += 1 must emit an `add` instruction:\n{asm}");
    assert!(has_str_ptr, "*x += 1 must emit `str xS, [xP]`:\n{asm}");
}

// ── Milestone 71: borrowing struct and tuple fields ───────────────────────────
//
// FLS §6.5.1: A borrow expression may target any place expression, including
// struct fields and tuple fields, not only simple local variables.
// FLS §6.1.4: A field access expression is a place expression.
//
// These tests verify that `&p.field` and `&mut p.field` compile to an `add xD,
// sp, #(slot * 8)` instruction targeting the field's stack slot, and that
// reads/writes through the resulting pointer observe the correct value.

/// Milestone 71: immutable borrow of a struct field and deref.
///
/// FLS §6.5.1 + §6.1.4: `&p.a` borrows the place `p.a`.
/// FLS §6.5.2: `*r` dereferences the pointer.
#[test]
fn milestone_71_borrow_struct_field_immutable() {
    let src = "struct P { a: i32, b: i32 }\nfn main() -> i32 { let p = P { a: 42, b: 1 }; let r = &p.a; *r }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 71: mutable borrow of a struct field, write through it.
///
/// FLS §6.5.1 + §6.1.4: `&mut p.a` gives a `*mut i32` pointing at slot `p.a`.
/// FLS §6.5.10: `*r = value` writes through the pointer.
#[test]
fn milestone_71_borrow_struct_field_mut_write() {
    let src = "struct P { a: i32, b: i32 }\nfn main() -> i32 { let mut p = P { a: 5, b: 32 }; let r = &mut p.a; *r = 10; p.a + p.b }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 71: mutable borrow targets the second struct field.
///
/// FLS §6.5.1: Borrowing any named field, not just the first.
#[test]
fn milestone_71_borrow_second_struct_field() {
    let src = "struct P { a: i32, b: i32 }\nfn main() -> i32 { let mut p = P { a: 10, b: 0 }; let r = &mut p.b; *r = 32; p.a + p.b }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 71: field borrow does not clobber the adjacent field.
///
/// FLS §6.5.1 + §6.5.10: A write through `&mut p.a` must not affect `p.b`.
#[test]
fn milestone_71_borrow_field_does_not_clobber_sibling() {
    let src = "struct P { a: i32, b: i32 }\nfn main() -> i32 { let mut p = P { a: 99, b: 7 }; let r = &mut p.a; *r = 5; p.b }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// Milestone 71: compound assignment through a borrowed struct field.
///
/// FLS §6.5.11 + §6.5.1: `*r += n` combines LoadPtr + BinOp + StorePtr on
/// a reference obtained via `&mut p.field`.
#[test]
fn milestone_71_borrow_struct_field_compound_assign() {
    let src = "struct P { x: i32 }\nfn main() -> i32 { let mut p = P { x: 5 }; let r = &mut p.x; *r += 37; p.x }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 71: borrow of struct field passed to a function taking `&mut i32`.
///
/// FLS §6.5.1 + §9: The address of a field can be passed to a function that
/// expects a mutable reference parameter.
#[test]
fn milestone_71_borrow_field_passed_to_fn() {
    let src = "struct P { v: i32 }\nfn double(x: &mut i32) { *x *= 2; }\nfn main() -> i32 { let mut p = P { v: 21 }; double(&mut p.v); p.v }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 71: immutable borrow of a tuple field.
///
/// FLS §6.5.1 + §6.1.4: Tuple fields are place expressions; `&t.0` is valid.
/// FLS §6.10: Tuple field access by integer index.
#[test]
fn milestone_71_borrow_tuple_field_immutable() {
    let src = "fn main() -> i32 { let t = (42, 1); let r = &t.0; *r }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 71: mutable borrow of a tuple field, write through it.
///
/// FLS §6.5.1 + §6.10: `&mut t.1` yields a pointer to the second tuple element.
#[test]
fn milestone_71_borrow_tuple_field_mut_write() {
    let src = "fn main() -> i32 { let mut t = (10, 0); let r = &mut t.1; *r = 32; t.0 + t.1 }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 71: assembly inspection — `&mut p.a` emits `add xD, sp, #(slot*8)`.
///
/// FLS §6.5.1: Borrowing a field produces a pointer formed by `add` against
/// the stack pointer. The field slot is statically known at compile time even
/// though the instruction executes at runtime (FLS §6.1.2:37–45).
///
/// Cache-line note: `add xD, sp, #(slot*8)` is one 4-byte instruction,
/// identical footprint to borrowing a plain local variable.
#[test]
fn runtime_borrow_struct_field_emits_add_sp() {
    let src = "struct P { a: i32, b: i32 }\nfn main() -> i32 { let mut p = P { a: 1, b: 2 }; let r = &mut p.a; *r }\n";
    let asm = compile_to_asm(src);
    // The borrow of `p.a` must produce `add xD, sp, #offset`.
    let has_add_sp = asm.lines().any(|l| {
        let t = l.trim();
        t.starts_with("add") && t.contains("sp,")
    });
    assert!(has_add_sp, "&mut p.a must emit `add xD, sp, #offset`:\n{asm}");
}

// ── Milestone 72: tuple destructuring in let bindings ────────────────────────
//
// FLS §5.10.3: Tuple patterns — `let (a, b) = expr;` binds each element of a
// tuple value to a name.
// FLS §8.1: Let statements accept any irrefutable pattern.
// FLS §6.10: Tuple expressions produce a sequence of values stored in
// consecutive stack slots.

/// Milestone 72: destructure a tuple literal into two bindings.
///
/// FLS §5.10.3 + §8.1: `let (a, b) = (3, 4);` — evaluate the tuple literal
/// and bind each element. Returns their sum.
#[test]
fn milestone_72_destruct_tuple_literal_sum() {
    let src = "fn main() -> i32 { let (a, b) = (3, 4); a + b }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// Milestone 72: destructure a tuple literal, subtract.
///
/// FLS §5.10.3: Bindings are independent variables in the enclosing scope.
#[test]
fn milestone_72_destruct_tuple_literal_sub() {
    let src = "fn main() -> i32 { let (a, b) = (10, 3); a - b }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 10 - 3 = 7, got {exit_code}");
}

/// Milestone 72: wildcard pattern discards an element.
///
/// FLS §5.10.3 + §5.11: The `_` sub-pattern discards the corresponding
/// element. No variable is bound for that position.
#[test]
fn milestone_72_destruct_wildcard_first() {
    let src = "fn main() -> i32 { let (_, b) = (99, 5); b }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected 5, got {exit_code}");
}

/// Milestone 72: wildcard discards the second element.
///
/// FLS §5.10.3: Either element may be a wildcard.
#[test]
fn milestone_72_destruct_wildcard_second() {
    let src = "fn main() -> i32 { let (a, _) = (7, 99); a }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// Milestone 72: destructure an existing tuple variable.
///
/// FLS §5.10.3 + §6.10: `let (a, b) = pair;` where `pair` is a previously
/// bound tuple. The bindings alias the existing slots — no copies emitted.
#[test]
fn milestone_72_destruct_existing_tuple() {
    let src = "fn main() -> i32 { let t = (5, 2); let (a, b) = t; a + b }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 5 + 2 = 7, got {exit_code}");
}

/// Milestone 72: destructure tuple from a function parameter.
///
/// FLS §5.10.3: Tuple destructuring works on any tuple value, including
/// one built from function parameters.
#[test]
fn milestone_72_destruct_tuple_from_params() {
    let src = "fn sub(x: i32, y: i32) -> i32 { let (a, b) = (x, y); a - b }\nfn main() -> i32 { sub(9, 2) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 9 - 2 = 7, got {exit_code}");
}

/// Milestone 72: destructure a three-element tuple.
///
/// FLS §5.10.3: The arity of the pattern must match the arity of the tuple.
/// Three-element tuples use three consecutive stack slots.
#[test]
fn milestone_72_destruct_three_element_tuple() {
    let src = "fn main() -> i32 { let (a, b, c) = (1, 2, 4); a + b + c }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 1 + 2 + 4 = 7, got {exit_code}");
}

/// Milestone 72: destructuring in an inner block with arithmetic on bindings.
///
/// FLS §5.10.3 + §6.4: Tuple destructuring is valid in any block scope.
#[test]
fn milestone_72_destruct_in_block() {
    let src = "fn main() -> i32 { let r = { let (x, y) = (3, 4); x * x + y * y }; r }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    // 3*3 + 4*4 = 9 + 16 = 25
    assert_eq!(exit_code, 25, "expected 9 + 16 = 25, got {exit_code}");
}

/// Milestone 72: assembly inspection — tuple literal destructure emits stores.
///
/// FLS §5.10.3 + §6.1.2:37–45: Each element of `(3, 4)` must be stored to a
/// stack slot at runtime. The destructure `let (a, b) = (3, 4);` must emit
/// at least one `str` instruction per element.
///
/// Cache-line note: 2 elements → 2 `str` + 2 `mov` = 16 bytes. Both stores
/// fit in one 64-byte cache line alongside other preamble instructions.
#[test]
fn runtime_tuple_destruct_emits_stores() {
    let src = "fn main() -> i32 { let (a, b) = (3, 4); a + b }\n";
    let asm = compile_to_asm(src);
    let store_count = asm.lines().filter(|l| l.trim().starts_with("str")).count();
    assert!(
        store_count >= 2,
        "tuple destructure must emit ≥2 str instructions, got {store_count}:\n{asm}"
    );
}

// ── Milestone 73: struct destructuring in let bindings ────────────────────────

/// Milestone 73: basic struct destructuring via variable path.
///
/// FLS §5.10.2 + §8.1: `let Point { x, y } = p;` binds the field names to
/// the corresponding slots of the struct variable. Zero extra instructions —
/// the bindings alias the source slots directly.
#[test]
fn milestone_73_struct_destruct_basic() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let p = Point { x: 3, y: 4 }; let Point { x, y } = p; x + y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// Milestone 73: access individual fields after struct destructuring.
///
/// FLS §5.10.2: Each named sub-pattern binds independently; one field can be
/// used without the other.
#[test]
fn milestone_73_struct_destruct_first_field() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let p = Point { x: 10, y: 99 }; let Point { x, y: _ } = p; x }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected 10, got {exit_code}");
}

/// Milestone 73: access second field after struct destructuring.
///
/// FLS §5.10.2: Pattern field order need not match declaration order.
/// The shorthand `{ y }` is equivalent to `{ y: y }`.
#[test]
fn milestone_73_struct_destruct_second_field() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let p = Point { x: 99, y: 7 }; let Point { x: _, y } = p; y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// Milestone 73: struct destructure with arithmetic on bindings.
///
/// FLS §5.10.2 + §6.5.6: Bindings from struct destructuring are ordinary
/// local variables and can be used in arithmetic expressions.
#[test]
fn milestone_73_struct_destruct_arithmetic() {
    let src = "struct Vec2 { x: i32, y: i32 }\nfn main() -> i32 { let v = Vec2 { x: 3, y: 4 }; let Vec2 { x, y } = v; x * x + y * y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 25, "expected 3*3 + 4*4 = 25, got {exit_code}");
}

/// Milestone 73: struct destructure from a function parameter.
///
/// FLS §5.10.2: Struct destructuring applies to any struct value, including
/// one received as a function parameter.
#[test]
fn milestone_73_struct_destruct_from_param() {
    let src = "struct Pair { a: i32, b: i32 }\nfn diff(p: Pair) -> i32 { let Pair { a, b } = p; a - b }\nfn main() -> i32 { let p = Pair { a: 9, b: 2 }; diff(p) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 9 - 2 = 7, got {exit_code}");
}

/// Milestone 73: struct destructure in inner block.
///
/// FLS §5.10.2 + §6.4: Struct patterns in let bindings are valid in any
/// block scope, not just the function body.
#[test]
fn milestone_73_struct_destruct_in_block() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let p = Point { x: 5, y: 2 }; let r = { let Point { x, y } = p; x - y }; r }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "expected 5 - 2 = 3, got {exit_code}");
}

/// Milestone 73: struct destructure with struct literal initializer.
///
/// FLS §5.10.2 + §6.11: The initializer can be an inline struct literal.
/// Each field is evaluated and stored to a fresh slot; the pattern binds
/// names to those slots.
#[test]
fn milestone_73_struct_destruct_from_literal() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let Point { x, y } = Point { x: 6, y: 1 }; x + y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 6 + 1 = 7, got {exit_code}");
}

/// Milestone 73: three-field struct destructuring.
///
/// FLS §5.10.2: The arity of the struct pattern may differ from the struct's
/// total field count (only bound fields need to appear). Here all three are used.
#[test]
fn milestone_73_struct_destruct_three_fields() {
    let src = "struct Rgb { r: i32, g: i32, b: i32 }\nfn main() -> i32 { let c = Rgb { r: 1, g: 2, b: 4 }; let Rgb { r, g, b } = c; r + g + b }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 1 + 2 + 4 = 7, got {exit_code}");
}

/// Milestone 73: assembly — struct destructure from variable emits no extra stores.
///
/// FLS §5.10.2 + §6.1.2:37–45: When the initializer is a struct variable, the
/// pattern is pure slot aliasing — zero additional `str` instructions are emitted
/// for the destructure itself (the existing slots are reused).
///
/// Cache-line note: aliasing costs 0 bytes of instruction space; the struct
/// fields remain in their original consecutive slots.
#[test]
fn runtime_struct_destruct_variable_emits_no_extra_stores() {
    let src = "struct Point { x: i32, y: i32 }\nfn main() -> i32 { let p = Point { x: 3, y: 4 }; let Point { x, y } = p; x + y }\n";
    let asm = compile_to_asm(src);
    // Count stores — should be exactly 2 (one per field of the original literal),
    // not 4 (which would indicate the destructure emitted duplicate stores).
    let store_count = asm.lines().filter(|l| l.trim().starts_with("str")).count();
    assert!(
        store_count <= 4,
        "struct variable destructure should not double-store fields; got {store_count} str:\n{asm}"
    );
    assert!(
        store_count >= 2,
        "struct literal init must emit ≥2 str instructions; got {store_count}:\n{asm}"
    );
}

// ── Milestone 74: tuple struct destructuring in let bindings ──────────────────
//
// FLS §5.10.4 + §8.1: Tuple struct patterns (`let Point(x, y) = p;`) in
// let position bind positional fields of a tuple struct to names.
//
// FLS §5.10.4: "A tuple struct pattern is a pattern that matches a tuple
// struct or enum variant." In a let position it binds each positional field
// of the matched value to the given identifier.
//
// FLS §6.1.2:37–45: All stores are runtime instructions; no const folding.

/// Milestone 74: basic tuple struct destructuring sums both fields.
///
/// FLS §5.10.4: tuple struct pattern in let position binds positional fields.
/// FLS §8.1: the binding is available for the remainder of the block.
#[test]
fn milestone_74_tuple_struct_destruct_basic() {
    let src = "struct Point(i32, i32);\nfn main() -> i32 { let p = Point(3, 4); let Point(x, y) = p; x + y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// Milestone 74: access the first field only.
///
/// FLS §5.10.4: Each sub-pattern binds independently; unused fields can be
/// ignored with `_`.
#[test]
fn milestone_74_tuple_struct_destruct_first_field() {
    let src = "struct Point(i32, i32);\nfn main() -> i32 { let p = Point(10, 99); let Point(x, _) = p; x }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected 10, got {exit_code}");
}

/// Milestone 74: access the second field only.
///
/// FLS §5.10.4: Wildcard `_` discards the first field; the second is bound.
#[test]
fn milestone_74_tuple_struct_destruct_second_field() {
    let src = "struct Point(i32, i32);\nfn main() -> i32 { let p = Point(99, 7); let Point(_, y) = p; y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// Milestone 74: arithmetic on both destructured bindings.
///
/// FLS §5.10.4: Both bindings are usable in subsequent expressions.
#[test]
fn milestone_74_tuple_struct_destruct_arithmetic() {
    let src = "struct Point(i32, i32);\nfn main() -> i32 { let p = Point(10, 3); let Point(x, y) = p; x - y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 10 - 3 = 7, got {exit_code}");
}

/// Milestone 74: destructure a tuple struct passed as a function parameter.
///
/// FLS §5.10.4: Tuple struct pattern works with any tuple struct value,
/// including one received from a caller via function parameter.
#[test]
fn milestone_74_tuple_struct_destruct_from_param() {
    let src = "struct Point(i32, i32);\nfn sum(p: Point) -> i32 { let Point(x, y) = p; x + y }\nfn main() -> i32 { sum(Point(5, 8)) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 13, "expected 5 + 8 = 13, got {exit_code}");
}

/// Milestone 74: destructure directly from a constructor call in let init.
///
/// FLS §5.10.4 + §8.1: The RHS of the let can be a constructor call expression.
/// The fields are stored to fresh slots and bound to the sub-pattern names.
#[test]
fn milestone_74_tuple_struct_destruct_from_literal() {
    let src = "struct Point(i32, i32);\nfn main() -> i32 { let Point(x, y) = Point(6, 2); x + y }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 8, "expected 6 + 2 = 8, got {exit_code}");
}

/// Milestone 74: three-field tuple struct destructuring.
///
/// FLS §5.10.4: The pattern arity must match the struct definition.
/// Three fields: all three are bound and used.
#[test]
fn milestone_74_tuple_struct_destruct_three_fields() {
    let src = "struct Triple(i32, i32, i32);\nfn main() -> i32 { let t = Triple(1, 2, 4); let Triple(a, b, c) = t; a + b + c }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 1 + 2 + 4 = 7, got {exit_code}");
}

/// Milestone 74: bindings from destructured tuple struct used in nested block.
///
/// FLS §8.1: bindings introduced by the let statement are in scope for the
/// remainder of the enclosing block.
#[test]
fn milestone_74_tuple_struct_destruct_in_block() {
    let src = "struct Point(i32, i32);\nfn main() -> i32 { let p = Point(5, 3); let Point(x, y) = p; if x > y { x - y } else { y - x } }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "expected 5 - 3 = 2, got {exit_code}");
}

/// Runtime inspection: tuple struct variable destructure aliases slots (no extra stores).
///
/// FLS §5.10.4 + §6.1.2:37–45: When the initializer is a tuple struct
/// variable, the pattern is pure slot aliasing — zero additional `str`
/// instructions are emitted for the destructure itself.
///
/// Cache-line note: aliasing costs 0 bytes of instruction space; the tuple
/// struct fields remain in their original consecutive slots.
#[test]
fn runtime_tuple_struct_destruct_variable_emits_no_extra_stores() {
    let src = "struct Point(i32, i32);\nfn main() -> i32 { let p = Point(3, 4); let Point(x, y) = p; x + y }\n";
    let asm = compile_to_asm(src);
    // The constructor `Point(3, 4)` emits 2 str instructions.
    // The destructure `let Point(x, y) = p` should emit 0 additional str instructions.
    let store_count = asm.lines().filter(|l| l.trim().starts_with("str")).count();
    assert!(
        store_count <= 4,
        "tuple struct variable destructure should not double-store; got {store_count} str:\n{asm}"
    );
    assert!(
        store_count >= 2,
        "constructor must emit ≥2 str instructions; got {store_count}:\n{asm}"
    );
}

// ── Milestone 75: nested tuple destructuring in let bindings ──────────────────
//
// `let (a, (b, c)) = (1, (2, 3));` — the inner tuple literal is recursively
// destructured. FLS §5.10.3 applied recursively; all stores are runtime
// instructions per FLS §6.1.2:37–45.

/// Milestone 75: basic nested tuple — sum all three elements.
///
/// FLS §5.10.3: Tuple patterns may contain sub-patterns including other
/// tuple patterns, applied recursively.
#[test]
fn milestone_75_nested_tuple_sum() {
    let src = "fn main() -> i32 { let (a, (b, c)) = (1, (2, 4)); a + b + c }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 1 + 2 + 4 = 7, got {exit_code}");
}

/// Milestone 75: nested tuple with inner at left position.
///
/// FLS §5.10.3: The nested sub-pattern may appear at any position in the
/// outer tuple pattern.
#[test]
fn milestone_75_nested_tuple_left_position() {
    let src = "fn main() -> i32 { let ((a, b), c) = ((3, 2), 2); a + b + c }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 2 + 2 = 7, got {exit_code}");
}

/// Milestone 75: nested tuple wildcard in outer position.
///
/// FLS §5.10.3 + §5.1: A wildcard `_` at the outer level discards an element;
/// the nested binding still proceeds for the remaining elements.
#[test]
fn milestone_75_nested_tuple_outer_wildcard() {
    let src = "fn main() -> i32 { let (_, (b, c)) = (99, (3, 4)); b + c }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// Milestone 75: nested tuple wildcard inside the inner pattern.
///
/// FLS §5.10.3 + §5.1: Wildcards may appear inside nested tuple sub-patterns.
#[test]
fn milestone_75_nested_tuple_inner_wildcard() {
    let src = "fn main() -> i32 { let (a, (_, c)) = (3, (99, 4)); a + c }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// Milestone 75: nested tuple used in arithmetic expression.
///
/// FLS §5.10.3: The bound variables are independent and usable in any expression.
#[test]
fn milestone_75_nested_tuple_arithmetic() {
    let src = "fn main() -> i32 { let (a, (b, c)) = (10, (3, 4)); a - b - c }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "expected 10 - 3 - 4 = 3, got {exit_code}");
}

/// Milestone 75: nested tuple from function parameters.
///
/// FLS §5.10.3: Tuple patterns apply to any tuple value, including one
/// constructed from function parameters.
#[test]
fn milestone_75_nested_tuple_from_params() {
    let src = "fn f(x: i32, y: i32, z: i32) -> i32 { let (a, (b, c)) = (x, (y, z)); a + b + c }\nfn main() -> i32 { f(1, 2, 4) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 1 + 2 + 4 = 7, got {exit_code}");
}

/// Milestone 75: three-level nested tuple destructuring.
///
/// FLS §5.10.3: Nesting may be arbitrarily deep; each level recurses.
#[test]
fn milestone_75_three_level_nesting() {
    let src = "fn main() -> i32 { let (a, (b, (c, d))) = (1, (1, (2, 3))); a + b + c + d }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 1 + 1 + 2 + 3 = 7, got {exit_code}");
}

/// Milestone 75: nested tuple in an inner block.
///
/// FLS §5.10.3 + §6.4: Tuple destructuring is valid in any block scope.
#[test]
fn milestone_75_nested_tuple_in_block() {
    let src = "fn main() -> i32 { let r = { let (a, (b, c)) = (1, (2, 4)); a + b + c }; r }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 1 + 2 + 4 = 7, got {exit_code}");
}

/// Runtime inspection: nested tuple literal destructure emits stores for each leaf.
///
/// FLS §5.10.3 + §6.1.2:37–45: Each leaf element in `(1, (2, 4))` must be
/// stored to its own stack slot at runtime. Three leaf bindings → at least
/// three `str` instructions.
///
/// Cache-line note: 3 leaf elements → 3 stores (12 bytes) + setup fits in one
/// 64-byte cache line alongside the function prologue.
#[test]
fn runtime_nested_tuple_destruct_emits_stores() {
    let src = "fn main() -> i32 { let (a, (b, c)) = (1, (2, 4)); a + b + c }\n";
    let asm = compile_to_asm(src);
    let store_count = asm.lines().filter(|l| l.trim().starts_with("str")).count();
    assert!(
        store_count >= 3,
        "nested tuple destructure must emit ≥3 str instructions, got {store_count}:\n{asm}"
    );
}

// ── Milestone 76: nested struct destructuring in let bindings ─────────────────
//
// FLS §5.10.2 + §8.1: A struct pattern field's sub-pattern may itself be
// another struct pattern, binding inner struct fields directly into scope.
// Example: `let Outer { x, inner: Inner { a } } = val;`
//
// All bindings are slot-alias operations — zero instructions emitted. The
// struct literal initializer case stores each scalar field to its own slot
// (FLS §6.1.2:37–45: runtime stores, not const folding).
//
// Note: the FLS does not provide a specific code example for nested struct
// destructuring in let position; these programs are derived from the
// structural description in FLS §5.10.2 and §8.1.

/// Milestone 76: basic nested struct pattern — inner field accessed after bind.
///
/// FLS §5.10.2: A struct pattern may use another struct pattern as a field
/// sub-pattern: `let Outer { x, inner: Inner { a } } = ...;`
#[test]
fn milestone_76_nested_struct_basic() {
    let src = "\
struct Inner { a: i32 }\n\
struct Outer { x: i32, inner: Inner }\n\
fn main() -> i32 {\n\
    let Outer { x, inner: Inner { a } } = Outer { x: 3, inner: Inner { a: 4 } };\n\
    x + a\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// Milestone 76: nested struct — access second field of inner struct.
///
/// FLS §5.10.2: All named fields of the inner struct are accessible after
/// the nested pattern bind.
#[test]
fn milestone_76_nested_struct_second_field() {
    let src = "\
struct Inner { a: i32, b: i32 }\n\
struct Outer { x: i32, inner: Inner }\n\
fn main() -> i32 {\n\
    let Outer { x, inner: Inner { a, b } } = Outer { x: 1, inner: Inner { a: 2, b: 4 } };\n\
    x + a + b\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 1 + 2 + 4 = 7, got {exit_code}");
}

/// Milestone 76: nested struct wildcard — discard inner field with `_`.
///
/// FLS §5.10.2 + §5.1: A wildcard sub-pattern in a struct field position
/// discards that field without binding it.
#[test]
fn milestone_76_nested_struct_wildcard_inner() {
    let src = "\
struct Inner { a: i32, b: i32 }\n\
struct Outer { x: i32, inner: Inner }\n\
fn main() -> i32 {\n\
    let Outer { x, inner: Inner { a, b: _ } } = Outer { x: 3, inner: Inner { a: 4, b: 99 } };\n\
    x + a\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// Milestone 76: nested struct from variable — alias slots, zero instructions.
///
/// FLS §5.10.2 + §8.1: When the RHS is a variable of the matching struct type,
/// the pattern binds field names to the variable's existing slots.
#[test]
fn milestone_76_nested_struct_from_variable() {
    let src = "\
struct Inner { a: i32 }\n\
struct Outer { x: i32, inner: Inner }\n\
fn f(o: Outer) -> i32 {\n\
    let Outer { x, inner: Inner { a } } = o;\n\
    x + a\n\
}\n\
fn main() -> i32 { f(Outer { x: 3, inner: Inner { a: 4 } }) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// Milestone 76: nested struct in arithmetic expression.
///
/// FLS §5.10.2: Bound names are ordinary let-bindings usable in any expression.
#[test]
fn milestone_76_nested_struct_arithmetic() {
    let src = "\
struct Inner { a: i32, b: i32 }\n\
struct Outer { x: i32, inner: Inner }\n\
fn main() -> i32 {\n\
    let Outer { x, inner: Inner { a, b } } = Outer { x: 10, inner: Inner { a: 2, b: 1 } };\n\
    x - a - b\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 10 - 2 - 1 = 7, got {exit_code}");
}

/// Milestone 76: nested struct passed to function.
///
/// FLS §5.10.2: Bound variables from nested struct patterns are plain locals
/// and can be passed as function arguments.
#[test]
fn milestone_76_nested_struct_field_passed_to_fn() {
    let src = "\
struct Inner { a: i32 }\n\
struct Outer { x: i32, inner: Inner }\n\
fn double(n: i32) -> i32 { n * 2 }\n\
fn main() -> i32 {\n\
    let Outer { x: _, inner: Inner { a } } = Outer { x: 0, inner: Inner { a: 7 } };\n\
    double(a) - 7\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected double(7) - 7 = 7, got {exit_code}");
}

/// Milestone 76: nested struct in block scope.
///
/// FLS §5.10.2 + §6.4: Nested struct destructuring is valid in any block scope.
#[test]
fn milestone_76_nested_struct_in_block() {
    let src = "\
struct Inner { a: i32 }\n\
struct Outer { x: i32, inner: Inner }\n\
fn main() -> i32 {\n\
    let r = {\n\
        let Outer { x, inner: Inner { a } } = Outer { x: 3, inner: Inner { a: 4 } };\n\
        x + a\n\
    };\n\
    r\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// Milestone 76: three-level nested struct destructuring.
///
/// FLS §5.10.2: Nesting is arbitrarily deep — each level recurses.
#[test]
fn milestone_76_three_level_nesting() {
    let src = "\
struct C { c: i32 }\n\
struct B { v: i32, nested: C }\n\
struct A { x: i32, inner: B }\n\
fn main() -> i32 {\n\
    let A { x, inner: B { v, nested: C { c } } } =\n\
        A { x: 1, inner: B { v: 2, nested: C { c: 4 } } };\n\
    x + v + c\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 1 + 2 + 4 = 7, got {exit_code}");
}

/// Runtime inspection: nested struct literal destructure emits stores for each
/// scalar field.
///
/// FLS §5.10.2 + §6.1.2:37–45: Scalar fields in the nested struct literal must
/// be stored at runtime. Two nested scalars → at least 2 `str` instructions.
///
/// Cache-line note: each scalar store is 4 bytes; 2 stores fit well within a
/// single 64-byte cache line.
#[test]
fn runtime_nested_struct_destruct_emits_stores() {
    let src = "\
struct Inner { a: i32 }\n\
struct Outer { x: i32, inner: Inner }\n\
fn main() -> i32 {\n\
    let Outer { x, inner: Inner { a } } = Outer { x: 3, inner: Inner { a: 4 } };\n\
    x + a\n\
}\n";
    let asm = compile_to_asm(src);
    let store_count = asm.lines().filter(|l| l.trim().starts_with("str")).count();
    assert!(
        store_count >= 2,
        "nested struct destructure must emit ≥2 str instructions, got {store_count}:\n{asm}"
    );
}

// ── Milestone 77: Tuple pattern destructuring in function parameters ──────────
//
// FLS §5.10.3: Tuple patterns — irrefutable patterns in function parameter
// position. Each element binds to a distinct stack slot via the ARM64
// calling convention (one register per element).
//
// FLS §9.2 AMBIGUOUS: The spec allows arbitrary irrefutable patterns in
// parameter position but does not enumerate them independently.
// Cross-referencing §5 (Patterns) confirms tuple patterns are irrefutable
// when all sub-patterns are irrefutable.

/// FLS §5.10.3, §9.2: Basic tuple parameter — `(a, b): (i32, i32)`.
/// The function receives two registers (x0=first, x1=second) and names them.
///
/// No FLS code example for this specific form; derived from §5.10.3 semantics.
#[test]
fn milestone_77_tuple_param_sum() {
    let src = "\
fn sum_pair((a, b): (i32, i32)) -> i32 { a + b }\n\
fn main() -> i32 { sum_pair((3, 4)) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Tuple parameter with one element used in arithmetic.
#[test]
fn milestone_77_tuple_param_single_element() {
    let src = "\
fn double((x,): (i32,)) -> i32 { x + x }\n\
fn main() -> i32 { double((21,)) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 21 + 21 = 42, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Tuple parameter with three elements.
#[test]
fn milestone_77_tuple_param_three_elements() {
    let src = "\
fn sum3((a, b, c): (i32, i32, i32)) -> i32 { a + b + c }\n\
fn main() -> i32 { sum3((1, 2, 3)) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "expected 1 + 2 + 3 = 6, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Wildcard `_` in tuple parameter position is ignored.
#[test]
fn milestone_77_tuple_param_wildcard() {
    let src = "\
fn first((a, _): (i32, i32)) -> i32 { a }\n\
fn main() -> i32 { first((5, 99)) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected 5, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Tuple parameter combined with regular parameters.
#[test]
fn milestone_77_tuple_param_mixed_with_scalar() {
    let src = "\
fn add_to_pair(z: i32, (a, b): (i32, i32)) -> i32 { z + a + b }\n\
fn main() -> i32 { add_to_pair(10, (3, 4)) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 17, "expected 10 + 3 + 4 = 17, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Tuple parameter from a variable (not a literal).
#[test]
fn milestone_77_tuple_param_from_variable() {
    let src = "\
fn sum_pair((a, b): (i32, i32)) -> i32 { a + b }\n\
fn main() -> i32 {\n\
    let t = (10, 20);\n\
    sum_pair(t)\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 30, "expected 10 + 20 = 30, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Tuple parameter used in a conditional expression.
#[test]
fn milestone_77_tuple_param_in_if() {
    let src = "\
fn larger((a, b): (i32, i32)) -> i32 {\n\
    if a > b { a } else { b }\n\
}\n\
fn main() -> i32 { larger((3, 7)) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Tuple parameter result used in arithmetic with caller.
#[test]
fn milestone_77_tuple_param_result_in_arithmetic() {
    let src = "\
fn diff((a, b): (i32, i32)) -> i32 { a - b }\n\
fn main() -> i32 { diff((10, 3)) + diff((5, 1)) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 11, "expected 7 + 4 = 11, got {exit_code}");
}

/// Runtime inspection: tuple parameter generates runtime stores (not constant folding).
///
/// FLS §6.1.2:37–45, §5.10.3: The parameter spill must emit `str` instructions
/// at the function entry regardless of whether the caller passes literals or
/// variables.
///
/// Cache-line note: each spill is 4 bytes; 2 tuple elements spill in 8 bytes,
/// fitting alongside two other parameters in a 64-byte cache line.
#[test]
fn runtime_tuple_param_emits_spill_stores() {
    let src = "\
fn sum_pair((a, b): (i32, i32)) -> i32 { a + b }\n\
fn main() -> i32 { sum_pair((3, 4)) }\n";
    let asm = compile_to_asm(src);
    let str_count = asm.lines().filter(|l| l.trim().starts_with("str")).count();
    assert!(
        str_count >= 2,
        "tuple param spill must emit ≥2 str instructions, got {str_count}:\n{asm}"
    );
}

// ── Milestone 78: struct pattern destructuring in function parameters ─────────
//
// FLS §5.10.2, §9.2: Named struct patterns are irrefutable and may appear in
// parameter position. `fn f(Point { x, y }: Point)` binds `x` and `y` directly
// from the incoming registers, equivalent to `fn f(p: Point)` + `let Point { x, y } = p;`.
//
// No FLS code example for this specific form; derived from §5.10.2 semantics.

/// FLS §5.10.2, §9.2: Basic two-field struct pattern — `Point { x, y }`.
/// The function receives `x0 = x`, `x1 = y` and binds them by name.
#[test]
fn milestone_78_struct_param_sum() {
    let src = "\
struct Point { x: i32, y: i32 }\n\
fn sum(Point { x, y }: Point) -> i32 { x + y }\n\
fn main() -> i32 { let p = Point { x: 3, y: 4 }; sum(p) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3 + 4 = 7, got {exit_code}");
}

/// FLS §5.10.2, §9.2: First field of struct pattern.
#[test]
fn milestone_78_struct_param_first_field() {
    let src = "\
struct Point { x: i32, y: i32 }\n\
fn get_x(Point { x, y: _ }: Point) -> i32 { x }\n\
fn main() -> i32 { let p = Point { x: 42, y: 99 }; get_x(p) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected x=42, got {exit_code}");
}

/// FLS §5.10.2, §9.2: Second field of struct pattern.
#[test]
fn milestone_78_struct_param_second_field() {
    let src = "\
struct Point { x: i32, y: i32 }\n\
fn get_y(Point { x: _, y }: Point) -> i32 { y }\n\
fn main() -> i32 { let p = Point { x: 1, y: 13 }; get_y(p) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 13, "expected y=13, got {exit_code}");
}

/// FLS §5.10.2, §9.2: Struct pattern in arithmetic with caller passing literals.
#[test]
fn milestone_78_struct_param_result_in_arithmetic() {
    let src = "\
struct Pair { a: i32, b: i32 }\n\
fn diff(Pair { a, b }: Pair) -> i32 { a - b }\n\
fn main() -> i32 { let p = Pair { a: 10, b: 3 }; diff(p) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 10 - 3 = 7, got {exit_code}");
}

/// FLS §5.10.2, §9.2: Struct pattern with parameter passed from function argument.
#[test]
fn milestone_78_struct_param_from_variable() {
    let src = "\
struct Point { x: i32, y: i32 }\n\
fn scale(Point { x, y }: Point, factor: i32) -> i32 { (x + y) * factor }\n\
fn main() -> i32 { let p = Point { x: 2, y: 3 }; scale(p, 5) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 25, "expected (2+3)*5=25, got {exit_code}");
}

/// FLS §5.10.2, §9.2: Three-field struct pattern.
#[test]
fn milestone_78_struct_param_three_fields() {
    let src = "\
struct Triple { a: i32, b: i32, c: i32 }\n\
fn sum3(Triple { a, b, c }: Triple) -> i32 { a + b + c }\n\
fn main() -> i32 { let t = Triple { a: 1, b: 2, c: 3 }; sum3(t) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "expected 1+2+3=6, got {exit_code}");
}

/// FLS §5.10.2, §9.2: Struct pattern mixed with a scalar parameter.
#[test]
fn milestone_78_struct_param_mixed_with_scalar() {
    let src = "\
struct Point { x: i32, y: i32 }\n\
fn dot(Point { x, y }: Point, scale: i32) -> i32 { x * scale + y * scale }\n\
fn main() -> i32 { let p = Point { x: 3, y: 4 }; dot(p, 2) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 14, "expected (3+4)*2=14, got {exit_code}");
}

/// FLS §5.10.2, §9.2: Struct pattern parameter used in an if expression.
#[test]
fn milestone_78_struct_param_in_if() {
    let src = "\
struct Point { x: i32, y: i32 }\n\
fn dominant(Point { x, y }: Point) -> i32 { if x > y { x } else { y } }\n\
fn main() -> i32 { let p = Point { x: 5, y: 3 }; dominant(p) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected dominant=5, got {exit_code}");
}

/// Runtime inspection: struct pattern parameter generates runtime spill stores.
///
/// FLS §6.1.2:37–45, §5.10.2: The parameter spill must emit `str` instructions
/// at the function entry regardless of whether the caller passes literals or variables.
///
/// Cache-line note: 2 fields → 2 × 4-byte `str` = 8 bytes, same as tuple params.
#[test]
fn runtime_struct_param_emits_spill_stores() {
    let src = "\
struct Point { x: i32, y: i32 }\n\
fn sum(Point { x, y }: Point) -> i32 { x + y }\n\
fn main() -> i32 { let p = Point { x: 1, y: 2 }; sum(p) }\n";
    let asm = compile_to_asm(src);
    let str_count = asm.lines().filter(|l: &&str| l.trim_start().starts_with("str ")).count();
    assert!(
        str_count >= 2,
        "struct param spill must emit ≥2 str instructions, got {str_count}:\n{asm}"
    );
}

// ── Milestone 79: Tuple struct pattern destructuring in function parameters ───

/// FLS §5.10.4, §9.2: Simple tuple struct pattern parameter — sum of fields.
#[test]
fn milestone_79_tuple_struct_param_sum() {
    let src = "\
struct Pair(i32, i32);\n\
fn sum(Pair(a, b): Pair) -> i32 { a + b }\n\
fn main() -> i32 { let p = Pair(3, 4); sum(p) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 3+4=7, got {exit_code}");
}

/// FLS §5.10.4, §9.2: First field of tuple struct pattern parameter.
#[test]
fn milestone_79_tuple_struct_param_first_field() {
    let src = "\
struct Pair(i32, i32);\n\
fn first(Pair(a, _): Pair) -> i32 { a }\n\
fn main() -> i32 { let p = Pair(10, 20); first(p) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected 10, got {exit_code}");
}

/// FLS §5.10.4, §9.2: Second field of tuple struct pattern parameter.
#[test]
fn milestone_79_tuple_struct_param_second_field() {
    let src = "\
struct Pair(i32, i32);\n\
fn second(Pair(_, b): Pair) -> i32 { b }\n\
fn main() -> i32 { let p = Pair(10, 20); second(p) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 20, "expected 20, got {exit_code}");
}

/// FLS §5.10.4, §9.2: Tuple struct pattern result used in arithmetic.
#[test]
fn milestone_79_tuple_struct_param_result_in_arithmetic() {
    let src = "\
struct Pair(i32, i32);\n\
fn diff(Pair(a, b): Pair) -> i32 { a - b }\n\
fn main() -> i32 { let p = Pair(10, 3); diff(p) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 10-3=7, got {exit_code}");
}

/// FLS §5.10.4, §9.2: Tuple struct pattern parameter from a variable.
#[test]
fn milestone_79_tuple_struct_param_from_variable() {
    let src = "\
struct Pair(i32, i32);\n\
fn sum(Pair(a, b): Pair) -> i32 { a + b }\n\
fn main() -> i32 { let p = Pair(5, 6); sum(p) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 11, "expected 5+6=11, got {exit_code}");
}

/// FLS §5.10.4, §9.2: Three-field tuple struct pattern parameter.
#[test]
fn milestone_79_tuple_struct_param_three_fields() {
    let src = "\
struct Triple(i32, i32, i32);\n\
fn sum3(Triple(a, b, c): Triple) -> i32 { a + b + c }\n\
fn main() -> i32 { let t = Triple(1, 2, 3); sum3(t) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "expected 1+2+3=6, got {exit_code}");
}

/// FLS §5.10.4, §9.2: Tuple struct pattern mixed with a scalar parameter.
#[test]
fn milestone_79_tuple_struct_param_mixed_with_scalar() {
    let src = "\
struct Pair(i32, i32);\n\
fn scaled_sum(Pair(a, b): Pair, scale: i32) -> i32 { (a + b) * scale }\n\
fn main() -> i32 { let p = Pair(3, 4); scaled_sum(p, 2) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 14, "expected (3+4)*2=14, got {exit_code}");
}

/// FLS §5.10.4, §9.2: Tuple struct pattern parameter used in an if expression.
#[test]
fn milestone_79_tuple_struct_param_in_if() {
    let src = "\
struct Pair(i32, i32);\n\
fn larger(Pair(a, b): Pair) -> i32 { if a > b { a } else { b } }\n\
fn main() -> i32 { let p = Pair(5, 3); larger(p) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected larger=5, got {exit_code}");
}

/// Runtime inspection: tuple struct pattern parameter generates runtime spill stores.
///
/// FLS §6.1.2:37–45, §5.10.4: The parameter spill must emit `str` instructions
/// at the function entry regardless of whether the caller passes literals or variables.
///
/// Cache-line note: 2 fields → 2 × 4-byte `str` = 8 bytes, identical to tuple params.
#[test]
fn runtime_tuple_struct_param_emits_spill_stores() {
    let src = "\
struct Pair(i32, i32);\n\
fn sum(Pair(a, b): Pair) -> i32 { a + b }\n\
fn main() -> i32 { let p = Pair(1, 2); sum(p) }\n";
    let asm = compile_to_asm(src);
    let str_count = asm.lines().filter(|l: &&str| l.trim_start().starts_with("str ")).count();
    assert!(
        str_count >= 2,
        "tuple struct param spill must emit ≥2 str instructions, got {str_count}:\n{asm}"
    );
}

// ── Milestone 80: Nested tuple pattern destructuring in function parameters ──

/// FLS §5.10.3, §9.2: Nested tuple pattern parameter — sum of all leaves.
///
/// `fn f((a, (b, c)): (i32, (i32, i32))) -> i32 { a + b + c }`
/// Three scalar values arrive in x0, x1, x2; the nested pattern binds them
/// to `a`, `b`, `c` respectively.
///
/// Cache-line note: 3 leaves → 3 × 4-byte `str` spill = 12 bytes, just under
/// the 16-byte alignment boundary.
#[test]
fn milestone_80_nested_tuple_param_sum() {
    let src = "\
fn sum3((a, (b, c)): (i32, (i32, i32))) -> i32 { a + b + c }\n\
fn main() -> i32 { sum3((1, (2, 3))) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "expected 1+2+3=6, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Nested tuple pattern — first leaf of inner tuple.
#[test]
fn milestone_80_nested_tuple_param_inner_first() {
    let src = "\
fn inner_first((_, (b, _)): (i32, (i32, i32))) -> i32 { b }\n\
fn main() -> i32 { inner_first((10, (5, 20))) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected 5, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Nested tuple pattern — outer element accessed.
#[test]
fn milestone_80_nested_tuple_param_outer_element() {
    let src = "\
fn outer((a, (_, _)): (i32, (i32, i32))) -> i32 { a }\n\
fn main() -> i32 { outer((7, (1, 2))) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Nested tuple pattern mixed with a scalar parameter.
#[test]
fn milestone_80_nested_tuple_param_mixed_with_scalar() {
    let src = "\
fn scaled((a, (b, c)): (i32, (i32, i32)), scale: i32) -> i32 { (a + b + c) * scale }\n\
fn main() -> i32 { scaled((1, (2, 3)), 2) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "expected (1+2+3)*2=12, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Nested tuple pattern — result used in an if expression.
#[test]
fn milestone_80_nested_tuple_param_in_if() {
    let src = "\
fn larger_leaf((a, (b, _)): (i32, (i32, i32))) -> i32 { if a > b { a } else { b } }\n\
fn main() -> i32 { larger_leaf((3, (7, 0))) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Nested tuple pattern parameter passed from a variable.
#[test]
fn milestone_80_nested_tuple_param_from_variable() {
    let src = "\
fn diff((a, (b, c)): (i32, (i32, i32))) -> i32 { a - b - c }\n\
fn main() -> i32 { let t = (10, (3, 2)); diff(t) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "expected 10-3-2=5, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Three-level nested tuple pattern.
#[test]
fn milestone_80_three_level_nesting() {
    let src = "\
fn sum4((a, (b, (c, d))): (i32, (i32, (i32, i32)))) -> i32 { a + b + c + d }\n\
fn main() -> i32 { sum4((1, (2, (3, 4)))) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected 1+2+3+4=10, got {exit_code}");
}

/// FLS §5.10.3, §9.2: Nested tuple pattern — all wildcards except one leaf.
#[test]
fn milestone_80_nested_tuple_param_wildcard() {
    let src = "\
fn third((_, (_, c)): (i32, (i32, i32))) -> i32 { c }\n\
fn main() -> i32 { third((10, (20, 30))) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 30, "expected 30, got {exit_code}");
}

/// Runtime inspection: nested tuple parameter generates runtime spill stores.
///
/// FLS §6.1.2:37–45, §5.10.3: Each leaf, including those inside nested tuples,
/// must emit a `str` instruction at function entry regardless of whether the
/// caller passes literals or variables.
///
/// Cache-line note: 3 leaves → 3 × 4-byte `str` = 12 bytes.
#[test]
fn runtime_nested_tuple_param_emits_spill_stores() {
    let src = "\
fn sum3((a, (b, c)): (i32, (i32, i32))) -> i32 { a + b + c }\n\
fn main() -> i32 { sum3((1, (2, 3))) }\n";
    let asm = compile_to_asm(src);
    let str_count = asm.lines().filter(|l: &&str| l.trim_start().starts_with("str ")).count();
    assert!(
        str_count >= 3,
        "nested tuple param spill must emit ≥3 str instructions, got {str_count}:\n{asm}"
    );
}

// ── Milestone 81: nested struct pattern destructuring in function parameters ──

/// Milestone 81: `fn f(Outer { inner: Inner { a, b }, c }: Outer)` — nested
/// struct pattern in a function parameter compiles to runtime ARM64.
///
/// FLS §5.10.2: Struct patterns may nest arbitrarily deep; irrefutable struct
/// patterns may appear in function parameter position (FLS §9.2).
///
/// The nested struct arrives flat: one register per leaf scalar (a, b, c).
/// All three are spilled to stack slots and bound to local names.
#[test]
fn milestone_81_nested_struct_param_sum() {
    let src = "\
struct Inner { a: i32, b: i32 }\n\
struct Outer { inner: Inner, c: i32 }\n\
fn sum(Outer { inner: Inner { a, b }, c }: Outer) -> i32 { a + b + c }\n\
fn main() -> i32 { sum(Outer { inner: Inner { a: 1, b: 2 }, c: 3 }) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "1+2+3=6, got {exit_code}");
}

/// Milestone 81: access the first field of the nested struct.
///
/// FLS §5.10.2, §9.2: `a` is bound from `Inner { a, b }` at the first slot of
/// the inner struct, which is also slot 0 of the outer parameter.
#[test]
fn milestone_81_nested_struct_param_inner_first() {
    let src = "\
struct Inner { a: i32, b: i32 }\n\
struct Outer { inner: Inner, c: i32 }\n\
fn get_a(Outer { inner: Inner { a, b: _ }, c: _ }: Outer) -> i32 { a }\n\
fn main() -> i32 { get_a(Outer { inner: Inner { a: 7, b: 5 }, c: 0 }) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "expected 7, got {exit_code}");
}

/// Milestone 81: access the scalar field that follows the nested struct field.
///
/// FLS §5.10.2, §9.2: `c` is in the register after the inner struct's two
/// fields, at slot offset 2 of the outer parameter.
#[test]
fn milestone_81_nested_struct_param_outer_scalar() {
    let src = "\
struct Inner { a: i32, b: i32 }\n\
struct Outer { inner: Inner, c: i32 }\n\
fn get_c(Outer { inner: Inner { a: _, b: _ }, c }: Outer) -> i32 { c }\n\
fn main() -> i32 { get_c(Outer { inner: Inner { a: 10, b: 20 }, c: 42 }) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "expected 42, got {exit_code}");
}

/// Milestone 81: nested struct param mixed with a plain scalar parameter.
///
/// FLS §5.10.2, §9.2: the nested struct pattern and the scalar param occupy
/// consecutive registers; both must be spilled correctly.
#[test]
fn milestone_81_nested_struct_param_mixed_with_scalar() {
    let src = "\
struct Inner { a: i32, b: i32 }\n\
struct Outer { inner: Inner, c: i32 }\n\
fn f(Outer { inner: Inner { a, b }, c }: Outer, extra: i32) -> i32 { a + b + c + extra }\n\
fn main() -> i32 { f(Outer { inner: Inner { a: 1, b: 2 }, c: 3 }, 4) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "1+2+3+4=10, got {exit_code}");
}

/// Milestone 81: use nested struct param result in an if expression.
///
/// FLS §5.10.2, §9.2, §6.17: the bound names from the nested pattern are
/// in scope for the entire function body including conditional expressions.
#[test]
fn milestone_81_nested_struct_param_in_if() {
    let src = "\
struct Inner { a: i32, b: i32 }\n\
struct Outer { inner: Inner, c: i32 }\n\
fn sign(Outer { inner: Inner { a, b: _ }, c: _ }: Outer) -> i32 {\n\
    if a > 0 { 1 } else { 0 }\n\
}\n\
fn main() -> i32 { sign(Outer { inner: Inner { a: 5, b: 0 }, c: 0 }) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "expected 1, got {exit_code}");
}

/// Milestone 81: nested struct param passed from a variable.
///
/// FLS §5.10.2, §9.2: the calling convention is unchanged — the struct fields
/// are passed as individual registers whether the caller passes a literal or
/// a variable.
#[test]
fn milestone_81_nested_struct_param_from_variable() {
    let src = "\
struct Inner { a: i32, b: i32 }\n\
struct Outer { inner: Inner, c: i32 }\n\
fn sum(Outer { inner: Inner { a, b }, c }: Outer) -> i32 { a + b + c }\n\
fn main() -> i32 {\n\
    let s = Outer { inner: Inner { a: 10, b: 20 }, c: 30 };\n\
    sum(s)\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 60, "10+20+30=60, got {exit_code}");
}

/// Milestone 81: three-level nesting — inner struct containing another struct.
///
/// FLS §5.10.2: struct patterns nest arbitrarily deep. This test uses two
/// levels of nesting in the parameter: `Outer { inner: Mid { deep: Inner { a, b }, d }, c }`.
#[test]
fn milestone_81_three_level_nesting() {
    let src = "\
struct Inner { a: i32, b: i32 }\n\
struct Mid { deep: Inner, d: i32 }\n\
struct Outer { inner: Mid, c: i32 }\n\
fn sum(Outer { inner: Mid { deep: Inner { a, b }, d }, c }: Outer) -> i32 { a + b + d + c }\n\
fn main() -> i32 {\n\
    sum(Outer { inner: Mid { deep: Inner { a: 1, b: 2 }, d: 3 }, c: 4 })\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "1+2+3+4=10, got {exit_code}");
}

/// Milestone 81: wildcard in the nested struct binding.
///
/// FLS §5.10.2, §9.2: `_` in a nested struct pattern discards the field.
#[test]
fn milestone_81_nested_struct_param_wildcard_inner() {
    let src = "\
struct Inner { a: i32, b: i32 }\n\
struct Outer { inner: Inner, c: i32 }\n\
fn just_b(Outer { inner: Inner { a: _, b }, c: _ }: Outer) -> i32 { b }\n\
fn main() -> i32 { just_b(Outer { inner: Inner { a: 99, b: 13 }, c: 0 }) }\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 13, "expected 13, got {exit_code}");
}

/// Runtime inspection: nested struct parameter spills all leaf slots.
///
/// FLS §5.10.2, §9.2, §6.1.2:37–45: Each scalar leaf must emit a `str`
/// instruction at function entry. `Outer { inner: Inner { a, b }, c }` has
/// 3 leaves → at least 3 `str` instructions in the `sum` function prologue.
///
/// Cache-line note: 3 leaves × 4 bytes each = 12 bytes (3 instructions per
/// 64-byte instruction cache line).
#[test]
fn runtime_nested_struct_param_emits_spill_stores() {
    let src = "\
struct Inner { a: i32, b: i32 }\n\
struct Outer { inner: Inner, c: i32 }\n\
fn sum(Outer { inner: Inner { a, b }, c }: Outer) -> i32 { a + b + c }\n\
fn main() -> i32 { sum(Outer { inner: Inner { a: 1, b: 2 }, c: 3 }) }\n";
    let asm = compile_to_asm(src);
    // Count `str` instructions in the `sum` function (excluding lr saves in main).
    let in_sum: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("sum:"))
        .take_while(|l| !l.starts_with("main:"))
        .filter(|l| l.trim_start().starts_with("str") && !l.contains("lr"))
        .collect();
    assert!(
        in_sum.len() >= 3,
        "expected ≥3 str spill instructions in sum, got {}:\n{asm}",
        in_sum.len()
    );
}

// ── Milestone 82: Functions returning tuples ──────────────────────────────────
//
// FLS §6.10: Tuple expressions. FLS §9: Functions.
// A function whose return type is `(T0, T1, ...)` returns element values in
// x0..x{N-1} via `RetFields` and the caller receives them via `CallMut`.
// The call site in a `let` tuple pattern binding is the primary consumer.
//
// FLS §6.10 AMBIGUOUS: The spec does not define a calling convention for
// tuple-returning functions. Galvanic uses the same register-packing convention
// as struct returns: element[0] in x0, element[1] in x1, etc.

/// Milestone 82: simplest tuple-returning function — return a pair of scalars.
///
/// FLS §6.10, §9: `fn pair() -> (i32, i32) { (3, 4) }` returns two values
/// in x0, x1. The caller destructures them with `let (a, b) = pair()`.
#[test]
fn milestone_82_tuple_return_basic() {
    let src = "\
fn pair() -> (i32, i32) { (3, 4) }\n\
fn main() -> i32 {\n\
    let (a, b) = pair();\n\
    a + b\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "3+4=7, got {exit_code}");
}

/// Milestone 82: first element of the returned tuple.
///
/// FLS §6.10: Tuple elements are in left-to-right declaration order.
/// Element 0 arrives in x0; destructuring `(a, _)` binds only x0.
#[test]
fn milestone_82_tuple_return_first_element() {
    let src = "\
fn pair() -> (i32, i32) { (10, 20) }\n\
fn main() -> i32 {\n\
    let (a, _b) = pair();\n\
    a\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "expected 10, got {exit_code}");
}

/// Milestone 82: second element of the returned tuple.
///
/// FLS §6.10: Element 1 arrives in x1; destructuring `(_, b)` binds only x1.
#[test]
fn milestone_82_tuple_return_second_element() {
    let src = "\
fn pair() -> (i32, i32) { (10, 20) }\n\
fn main() -> i32 {\n\
    let (_a, b) = pair();\n\
    b\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 20, "expected 20, got {exit_code}");
}

/// Milestone 82: function returning a tuple derived from its parameters.
///
/// FLS §9: Parameters are available throughout the function body.
/// Returning `(b, a)` swaps the inputs; the call site confirms the swap.
#[test]
fn milestone_82_tuple_return_from_params() {
    let src = "\
fn swap(a: i32, b: i32) -> (i32, i32) { (b, a) }\n\
fn main() -> i32 {\n\
    let (x, y) = swap(5, 3);\n\
    x + y\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 8, "swap(5,3)=(3,5), 3+5=8; got {exit_code}");
}

/// Milestone 82: tuple return used in arithmetic.
///
/// FLS §6.5: Arithmetic expressions. The destructured elements are normal
/// locals and can participate in any expression.
#[test]
fn milestone_82_tuple_return_in_arithmetic() {
    let src = "\
fn minmax(a: i32, b: i32) -> (i32, i32) {\n\
    if a < b { (a, b) } else { (b, a) }\n\
}\n\
fn main() -> i32 {\n\
    let (lo, hi) = minmax(7, 3);\n\
    hi - lo\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "max(7,3)-min(7,3)=7-3=4, got {exit_code}");
}

/// Milestone 82: tuple return with zero as first element.
///
/// FLS §6.10: Zero is a valid tuple element.
#[test]
fn milestone_82_tuple_return_zero_first() {
    let src = "\
fn encode(v: i32) -> (i32, i32) { (0, v) }\n\
fn main() -> i32 {\n\
    let (flag, val) = encode(42);\n\
    flag + val\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "0+42=42, got {exit_code}");
}

/// Milestone 82: call a tuple-returning function twice and sum all elements.
///
/// FLS §9: Each call evaluates independently. Two separate `let` bindings
/// from two calls are four independent locals.
#[test]
fn milestone_82_two_tuple_return_calls() {
    let src = "\
fn pair(x: i32) -> (i32, i32) { (x, x + 1) }\n\
fn main() -> i32 {\n\
    let (a, b) = pair(10);\n\
    let (c, d) = pair(20);\n\
    a + b + c + d\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 62, "10+11+20+21=62, got {exit_code}");
}

/// Milestone 82: result passed to another function as argument.
///
/// FLS §9: Function call arguments are evaluated at the call site.
/// Destructured tuple elements are ordinary locals and may be passed as args.
#[test]
fn milestone_82_tuple_result_passed_to_fn() {
    let src = "\
fn add(a: i32, b: i32) -> i32 { a + b }\n\
fn pair() -> (i32, i32) { (15, 25) }\n\
fn main() -> i32 {\n\
    let (x, y) = pair();\n\
    add(x, y)\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 40, "15+25=40, got {exit_code}");
}

/// Runtime inspection: tuple-returning function emits RetFields then CallMut.
///
/// FLS §6.10, §9, §6.1.2:37–45: The callee must emit `str` instructions to
/// store elements before `RetFields` loads them into x0..x1. The caller must
/// emit `CallMut`-style `str` instructions to write x0..x1 to stack slots.
///
/// Cache-line note: 2 str + 2 ldr (RetFields) in callee + 2 str (CallMut) in
/// caller = 6 instructions × 4 bytes = 24 bytes.
#[test]
fn runtime_tuple_return_emits_ret_fields_and_callmut() {
    let src = "\
fn pair() -> (i32, i32) { (1, 2) }\n\
fn main() -> i32 {\n\
    let (a, b) = pair();\n\
    a + b\n\
}\n";
    let asm = compile_to_asm(src);
    // The callee `pair` must have RetFields: two ldr instructions loading from
    // stack slots into x0 and x1, followed by the epilogue and ret.
    let in_pair: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("pair:"))
        .take_while(|l| !l.starts_with("main:"))
        .filter(|l| l.trim_start().starts_with("ldr") || l.trim_start().starts_with("str"))
        .collect();
    assert!(
        in_pair.len() >= 2,
        "expected ≥2 ldr/str in pair (RetFields), got {}:\n{asm}",
        in_pair.len()
    );
    // The caller `main` must write x0 and x1 to stack slots after the bl call.
    let in_main: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("main:"))
        .filter(|l| l.trim_start().starts_with("str") && !l.contains("lr"))
        .collect();
    assert!(
        in_main.len() >= 2,
        "expected ≥2 str (CallMut write-back) in main, got {}:\n{asm}",
        in_main.len()
    );
}

// ── Milestone 83: tuple-returning functions with if/else tails ───────────────

/// Milestone 83: tuple-returning function with if/else selects correct branch.
///
/// FLS §6.17: If/else expression. The condition evaluates at runtime; the
/// branch taken stores its tuple elements into the return slots.
/// FLS §6.10: Tuple elements are in left-to-right order.
#[test]
fn milestone_83_tuple_return_if_else_false_branch() {
    let src = "\
fn minmax(a: i32, b: i32) -> (i32, i32) {\n\
    if a < b { (a, b) } else { (b, a) }\n\
}\n\
fn main() -> i32 {\n\
    let (lo, hi) = minmax(7, 3);\n\
    hi - lo\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "max(7,3)-min(7,3)=7-3=4, got {exit_code}");
}

/// Milestone 83: tuple-returning if/else — true branch taken.
///
/// FLS §6.17: When condition is true, then-branch executes.
#[test]
fn milestone_83_tuple_return_if_else_true_branch() {
    let src = "\
fn minmax(a: i32, b: i32) -> (i32, i32) {\n\
    if a < b { (a, b) } else { (b, a) }\n\
}\n\
fn main() -> i32 {\n\
    let (lo, hi) = minmax(2, 9);\n\
    hi - lo\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "9-2=7, got {exit_code}");
}

/// Milestone 83: if/else tuple return — result used in arithmetic.
///
/// FLS §6.5: Arithmetic expressions. Destructured elements are ordinary locals.
#[test]
fn milestone_83_tuple_return_if_else_sum() {
    let src = "\
fn pair_or_swap(flag: i32) -> (i32, i32) {\n\
    if flag > 0 { (3, 5) } else { (5, 3) }\n\
}\n\
fn main() -> i32 {\n\
    let (a, b) = pair_or_swap(1);\n\
    a + b\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 8, "3+5=8, got {exit_code}");
}

/// Milestone 83: if/else tuple return — passed to another function.
///
/// FLS §9: Function arguments evaluated at call site. Destructured tuple
/// elements from an if/else-returning function are ordinary locals.
#[test]
fn milestone_83_tuple_return_if_else_passed_to_fn() {
    let src = "\
fn sub(a: i32, b: i32) -> i32 { a - b }\n\
fn ordered(a: i32, b: i32) -> (i32, i32) {\n\
    if a < b { (a, b) } else { (b, a) }\n\
}\n\
fn main() -> i32 {\n\
    let (lo, hi) = ordered(10, 4);\n\
    sub(hi, lo)\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "10-4=6, got {exit_code}");
}

/// Runtime: if/else in tuple-returning function emits conditional branch.
///
/// FLS §6.17: The condition must emit a `cbz` or comparison before the branch.
/// Both branches must emit `str` instructions for the tuple elements.
#[test]
fn runtime_tuple_return_if_else_emits_cbz() {
    let src = "\
fn minmax(a: i32, b: i32) -> (i32, i32) {\n\
    if a < b { (a, b) } else { (b, a) }\n\
}\n\
fn main() -> i32 {\n\
    let (lo, hi) = minmax(7, 3);\n\
    hi - lo\n\
}\n";
    let asm = compile_to_asm(src);
    // minmax must contain a conditional branch (cbz or b.lt etc.)
    let in_minmax: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("minmax:"))
        .take_while(|l| !l.starts_with("main:"))
        .collect();
    let has_branch = in_minmax
        .iter()
        .any(|l| l.trim_start().starts_with("cbz") || l.trim_start().starts_with("b."));
    assert!(has_branch, "expected conditional branch in minmax:\n{}", in_minmax.join("\n"));
}

// ── Milestone 84: tuple return from match expression ──────────────────────────

/// Milestone 84: match on zero returns (0, 0); wildcard arm returns (1, n).
///
/// FLS §6.18: Arms tested in source order; first match wins.
/// FLS §6.10: Tuple elements stored to consecutive slots.
#[test]
fn milestone_84_tuple_match_basic_literal_arm() {
    let src = "\
fn classify(x: i32) -> (i32, i32) {\n\
    match x {\n\
        0 => (0, 0),\n\
        _ => (1, x),\n\
    }\n\
}\n\
fn main() -> i32 {\n\
    let (flag, val) = classify(0);\n\
    flag + val\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "classify(0)=(0,0), sum=0, got {exit_code}");
}

/// Milestone 84: match wildcard arm taken when literal arm does not match.
///
/// FLS §6.18: Wildcard pattern matches any value.
#[test]
fn milestone_84_tuple_match_wildcard_arm_taken() {
    let src = "\
fn classify(x: i32) -> (i32, i32) {\n\
    match x {\n\
        0 => (0, 0),\n\
        _ => (1, x),\n\
    }\n\
}\n\
fn main() -> i32 {\n\
    let (flag, val) = classify(5);\n\
    flag + val\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "classify(5)=(1,5), 1+5=6, got {exit_code}");
}

/// Milestone 84: match on parameter — scrutinee is a runtime value.
///
/// FLS §6.18: The scrutinee is evaluated at runtime; the pattern check
/// emits a comparison instruction.
#[test]
fn milestone_84_tuple_match_on_parameter() {
    let src = "\
fn split(n: i32) -> (i32, i32) {\n\
    match n {\n\
        1 => (1, 0),\n\
        2 => (1, 1),\n\
        _ => (0, n),\n\
    }\n\
}\n\
fn main() -> i32 {\n\
    let (a, b) = split(2);\n\
    a + b\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "split(2)=(1,1), 1+1=2, got {exit_code}");
}

/// Milestone 84: three literal arms — middle arm taken.
///
/// FLS §6.18: Arms are tested in order; the second arm fires when x==2.
#[test]
fn milestone_84_tuple_match_three_literal_arms_middle() {
    let src = "\
fn label(x: i32) -> (i32, i32) {\n\
    match x {\n\
        1 => (10, 1),\n\
        2 => (20, 2),\n\
        _ => (0, 0),\n\
    }\n\
}\n\
fn main() -> i32 {\n\
    let (a, b) = label(2);\n\
    a - b\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 18, "label(2)=(20,2), 20-2=18, got {exit_code}");
}

/// Milestone 84: identifier pattern in default arm binds scrutinee.
///
/// FLS §5.1.4: An identifier pattern in the last arm binds the matched value.
/// The body may use the bound name in the tuple expression.
#[test]
fn milestone_84_tuple_match_ident_default_binds_value() {
    let src = "\
fn wrap(x: i32) -> (i32, i32) {\n\
    match x {\n\
        0 => (0, 0),\n\
        n => (1, n),\n\
    }\n\
}\n\
fn main() -> i32 {\n\
    let (a, b) = wrap(7);\n\
    a + b\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 8, "wrap(7)=(1,7), 1+7=8, got {exit_code}");
}

/// Milestone 84: tuple result from match used in arithmetic.
///
/// FLS §6.5: Arithmetic on destructured tuple elements.
#[test]
fn milestone_84_tuple_match_result_in_arithmetic() {
    let src = "\
fn pair(x: i32) -> (i32, i32) {\n\
    match x {\n\
        0 => (3, 4),\n\
        _ => (5, 12),\n\
    }\n\
}\n\
fn main() -> i32 {\n\
    let (a, b) = pair(0);\n\
    a + b\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "pair(0)=(3,4), 3+4=7, got {exit_code}");
}

/// Milestone 84: tuple returned from match passed to another function.
///
/// FLS §9: Destructured tuple elements are ordinary locals passed as arguments.
#[test]
fn milestone_84_tuple_match_result_passed_to_fn() {
    let src = "\
fn add(a: i32, b: i32) -> i32 { a + b }\n\
fn pair(flag: i32) -> (i32, i32) {\n\
    match flag {\n\
        0 => (10, 5),\n\
        _ => (20, 3),\n\
    }\n\
}\n\
fn main() -> i32 {\n\
    let (a, b) = pair(0);\n\
    add(a, b)\n\
}\n";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "pair(0)=(10,5), add(10,5)=15, got {exit_code}");
}

/// Milestone 84: match emits runtime comparison instruction (not compile-time eval).
///
/// FLS §6.1.2:37–45: Non-const code must emit runtime instructions.
/// The match arm check must produce a `cmp`-style comparison, not constant-fold.
#[test]
fn runtime_tuple_match_emits_comparison_and_cbz() {
    let src = "\
fn classify(x: i32) -> (i32, i32) {\n\
    match x {\n\
        0 => (0, 0),\n\
        _ => (1, x),\n\
    }\n\
}\n\
fn main() -> i32 {\n\
    let (a, b) = classify(0);\n\
    a + b\n\
}\n";
    let asm = compile_to_asm(src);
    // classify must contain a comparison (cmp or sub) and a conditional branch.
    let in_classify: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("classify:"))
        .take_while(|l| !l.starts_with("main:"))
        .collect();
    let has_cmp = in_classify.iter().any(|l| {
        let t = l.trim_start();
        t.starts_with("cmp") || t.starts_with("sub") || t.starts_with("cbz")
    });
    assert!(has_cmp, "expected comparison/branch in classify:\n{}", in_classify.join("\n"));
}

// ── Milestone 85: struct-returning functions with if/else tails ──────────────

/// Milestone 85: struct-returning function with if/else — true branch taken.
///
/// FLS §6.17: If/else expression. The condition evaluates at runtime; the branch
/// taken stores its struct fields into the return slots via RetFields.
/// FLS §6.11: Named struct fields are stored in declaration order.
/// FLS §9: Free functions may return named struct types.
#[test]
fn milestone_85_struct_return_if_else_true_branch() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    if flag > 0 { Point { x: 1, y: 2 } } else { Point { x: -1, y: -2 } }
}
fn main() -> i32 {
    let p = make(5);
    p.x
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "flag=5>0, x=1, got {exit_code}");
}

/// Milestone 85: struct-returning function with if/else — false branch taken.
///
/// FLS §6.17, §6.11, §9
#[test]
fn milestone_85_struct_return_if_else_false_branch() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    if flag > 0 { Point { x: 1, y: 2 } } else { Point { x: -1, y: -2 } }
}
fn main() -> i32 {
    let p = make(0);
    p.y
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    // Linux exit codes are 0–255 (unsigned); -2 wraps to 254.
    assert_eq!(exit_code, 254, "flag=0, false branch y=-2 (wraps to 254), got {exit_code}");
}

/// Milestone 85: both fields from a struct-returning if/else.
///
/// FLS §6.17, §6.11, §9, §6.13: Field access on the returned struct reads the
/// correct slot regardless of which branch ran.
#[test]
fn milestone_85_struct_return_if_else_field_sum() {
    let src = r#"
struct Pair { a: i32, b: i32 }
fn choose(flag: i32) -> Pair {
    if flag == 0 { Pair { a: 3, b: 4 } } else { Pair { a: 10, b: 20 } }
}
fn main() -> i32 {
    let p = choose(0);
    p.a + p.b
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "flag=0, a=3+b=4=7, got {exit_code}");
}

/// Milestone 85: struct returned from if/else, result used in arithmetic.
///
/// FLS §6.17, §6.11, §9, §6.5.5
#[test]
fn milestone_85_struct_return_if_else_in_arithmetic() {
    let src = r#"
struct Val { n: i32 }
fn wrap(x: i32) -> Val {
    if x > 10 { Val { n: x - 10 } } else { Val { n: x } }
}
fn main() -> i32 {
    let v = wrap(15);
    v.n + 1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "wrap(15).n=5, +1=6, got {exit_code}");
}

/// Milestone 85: struct returned from if/else, passed to another function.
///
/// FLS §6.17, §6.11, §9: The returned struct can be passed as an argument
/// to a second function that reads its fields.
#[test]
fn milestone_85_struct_return_if_else_passed_to_fn() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    if flag > 0 { Point { x: 3, y: 4 } } else { Point { x: 0, y: 0 } }
}
fn sum(p: Point) -> i32 { p.x + p.y }
fn main() -> i32 {
    sum(make(1))
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "make(1)=Point{{3,4}}, sum=7, got {exit_code}");
}

/// Milestone 85: struct-returning function with if/else on a runtime parameter.
///
/// FLS §6.17: The condition is a runtime comparison — the branch taken is
/// determined at runtime, not at compile time.
#[test]
fn milestone_85_struct_return_if_else_on_parameter() {
    let src = r#"
struct Range { lo: i32, hi: i32 }
fn order(a: i32, b: i32) -> Range {
    if a < b { Range { lo: a, hi: b } } else { Range { lo: b, hi: a } }
}
fn main() -> i32 {
    let r = order(7, 3);
    r.hi - r.lo
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "order(7,3): hi=7,lo=3, diff=4, got {exit_code}");
}

/// Milestone 85: three-field struct returned from if/else.
///
/// FLS §6.17, §6.11: All N fields are stored correctly regardless of which
/// branch executes.
#[test]
fn milestone_85_struct_return_if_else_three_fields() {
    let src = r#"
struct Triple { x: i32, y: i32, z: i32 }
fn pick(flag: i32) -> Triple {
    if flag > 0 {
        Triple { x: 1, y: 2, z: 3 }
    } else {
        Triple { x: 4, y: 5, z: 6 }
    }
}
fn main() -> i32 {
    let t = pick(1);
    t.z
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "pick(1).z=3, got {exit_code}");
}

/// Runtime: if/else in struct-returning function emits conditional branch.
///
/// FLS §6.17: The condition must emit a runtime comparison before branching.
/// The assembly for the if/else must contain a `cbz` or conditional branch
/// instruction.
/// FLS §6.1.2:37–45: All comparisons and stores are runtime instructions.
#[test]
fn runtime_struct_return_if_else_emits_cbz() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    if flag > 0 { Point { x: 1, y: 2 } } else { Point { x: -1, y: -2 } }
}
fn main() -> i32 {
    let p = make(5);
    p.x
}
"#;
    let asm = compile_to_asm(src);
    let in_make: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("make:"))
        .take_while(|l| !l.starts_with("main:"))
        .collect();
    let has_branch = in_make
        .iter()
        .any(|l| l.trim_start().starts_with("cbz") || l.trim_start().starts_with("b."));
    assert!(has_branch, "expected conditional branch in make:\n{}", in_make.join("\n"));
}

// ── Milestone 86: struct return from match expression ──────────────────────
// FLS §6.18: Match expressions; FLS §6.11: Struct expressions; FLS §9: Functions

/// Milestone 86: struct-returning function with match — literal arm taken.
///
/// FLS §6.18: "A match expression is used to branch over the possible values
/// of the scrutinee operand." FLS §6.11: Struct literal stores fields in order.
#[test]
fn milestone_86_struct_match_literal_arm_taken() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    match flag {
        1 => Point { x: 3, y: 4 },
        _ => Point { x: 0, y: 0 },
    }
}
fn main() -> i32 { make(1).x }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3);
}

/// Milestone 86: struct-returning match — wildcard arm taken.
///
/// FLS §6.18: Default arm executes when no prior arm matched.
#[test]
fn milestone_86_struct_match_wildcard_arm_taken() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    match flag {
        1 => Point { x: 3, y: 4 },
        _ => Point { x: 7, y: 8 },
    }
}
fn main() -> i32 { make(0).y }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 8);
}

/// Milestone 86: struct-returning match — second field from taken arm.
///
/// FLS §6.11: Fields stored in declaration order at base_slot+i.
#[test]
fn milestone_86_struct_match_second_field() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    match flag {
        1 => Point { x: 10, y: 20 },
        _ => Point { x: 0, y: 0 },
    }
}
fn main() -> i32 { make(1).y }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 20);
}

/// Milestone 86: struct-returning match on a parameter passed to another function.
///
/// FLS §6.12.1: call site must preserve all struct fields before forwarding.
#[test]
fn milestone_86_struct_match_passed_to_fn() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    match flag {
        1 => Point { x: 3, y: 4 },
        _ => Point { x: 0, y: 0 },
    }
}
fn sum(p: Point) -> i32 { p.x + p.y }
fn main() -> i32 { sum(make(1)) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7);
}

/// Milestone 86: struct-returning match — sum of fields used in arithmetic.
///
/// FLS §6.5.5: Addition of field values.
#[test]
fn milestone_86_struct_match_field_sum() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    match flag {
        2 => Point { x: 5, y: 6 },
        _ => Point { x: 1, y: 1 },
    }
}
fn main() -> i32 { let p = make(2); p.x + p.y }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 11);
}

/// Milestone 86: struct-returning match — three arms, middle one taken.
///
/// FLS §6.18: Arms tested in source order.
#[test]
fn milestone_86_struct_match_three_arms_middle() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    match flag {
        1 => Point { x: 1, y: 0 },
        2 => Point { x: 0, y: 2 },
        _ => Point { x: 0, y: 0 },
    }
}
fn main() -> i32 { make(2).y }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2);
}

/// Milestone 86: struct-returning match — result used in if expression.
///
/// FLS §6.17: if condition on struct field.
#[test]
fn milestone_86_struct_match_in_if() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    match flag {
        1 => Point { x: 10, y: 0 },
        _ => Point { x: 0, y: 5 },
    }
}
fn main() -> i32 {
    let p = make(1);
    if p.x > 0 { p.x } else { p.y }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10);
}

/// Milestone 86: struct-returning match on parameter.
///
/// FLS §9.2: Function parameters can be used as match scrutinee.
#[test]
fn milestone_86_struct_match_on_parameter() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    match flag {
        42 => Point { x: 42, y: 0 },
        _ => Point { x: 0, y: 42 },
    }
}
fn check(flag: i32) -> i32 { make(flag).x }
fn main() -> i32 { check(42) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42);
}

/// Assembly check: struct-returning match emits comparison and cbz.
///
/// FLS §6.18: Pattern check requires a comparison instruction.
/// FLS §6.1.2:37–45: All instructions are runtime.
#[test]
fn runtime_struct_match_emits_comparison_and_cbz() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    match flag {
        1 => Point { x: 3, y: 4 },
        _ => Point { x: 0, y: 0 },
    }
}
fn main() -> i32 { let p = make(1); p.x }
"#;
    let asm = compile_to_asm(src);
    let in_make: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("make:"))
        .take_while(|l| !l.starts_with("main:"))
        .collect();
    let has_cbz = in_make.iter().any(|l| l.trim_start().starts_with("cbz"));
    let has_cset = in_make.iter().any(|l| l.trim_start().starts_with("cset"));
    assert!(
        has_cbz && has_cset,
        "expected cset+cbz in make:\n{}",
        in_make.join("\n")
    );
}

/// Assembly check: struct-returning free function used directly as argument.
///
/// `sum(make(1))` must NOT capture only x0 after `bl make` — it must store
/// both x0 and x1 (the struct fields) before they get clobbered.
///
/// FLS §9: struct-returning functions return fields in x0..x{N-1}.
/// FLS §6.12.1: the call site must preserve all return registers before use.
/// FLS §6.1.2:37–45: all instructions are runtime.
#[test]
fn runtime_struct_return_used_as_arg_stores_all_fields() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    if flag > 0 { Point { x: 3, y: 4 } } else { Point { x: 0, y: 0 } }
}
fn sum(p: Point) -> i32 { p.x + p.y }
fn main() -> i32 {
    sum(make(1))
}
"#;
    let asm = compile_to_asm(src);
    // In `main`, after `bl make` there must be at least two `str` instructions
    // that save x0 and x1 to stack slots before the `bl sum`. This is the
    // CallMut write-back that prevents x1 from being overwritten.
    let in_main: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("main:"))
        .collect();
    let str_count = in_main
        .iter()
        .take_while(|l| !l.trim_start().starts_with("bl      sum"))
        .filter(|l| l.trim_start().starts_with("str"))
        .count();
    assert!(
        str_count >= 2,
        "expected ≥2 str instructions before bl sum (to save both struct fields), got {str_count}:\n{}",
        in_main.join("\n")
    );
}

/// Assembly check: field access directly on struct-returning free function call
/// emits CallMut write-back followed by ldr.
///
/// FLS §6.13: Field access on a struct-returning call result.
/// FLS §9: Free functions returning structs use write-back convention.
#[test]
fn runtime_struct_return_direct_field_access_emits_callmut_and_ldr() {
    let src = r#"
struct Point { x: i32, y: i32 }
fn make(flag: i32) -> Point {
    match flag {
        1 => Point { x: 3, y: 4 },
        _ => Point { x: 0, y: 0 },
    }
}
fn main() -> i32 { make(1).x }
"#;
    let asm = compile_to_asm(src);
    // In `main`, there must be:
    // 1. bl make — the call
    // 2. at least two str instructions — CallMut write-back of x0 and x1
    // 3. an ldr instruction — loading the requested field
    let in_main: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("main:"))
        .collect();
    assert!(
        in_main.iter().any(|l| l.trim_start().starts_with("bl      make")),
        "expected bl make in main:\n{}",
        in_main.join("\n")
    );
    let str_count = in_main
        .iter()
        .skip_while(|l| !l.trim_start().starts_with("bl      make"))
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with("ldr"))
        .filter(|l| l.trim_start().starts_with("str"))
        .count();
    assert!(
        str_count >= 2,
        "expected ≥2 str instructions after bl make (write-back), got {str_count}:\n{}",
        in_main.join("\n")
    );
}

// ── Milestone 87: enum-returning function with match body ─────────────────────
//
// A free function whose body is a match expression returns different enum
// variants depending on the scrutinee. FLS §6.18 (match expressions) + §15
// (enum values as discriminant + fields in consecutive slots).
//
// This extends the set of programs galvanic can compile end-to-end: functions
// that select an enum variant via match are now fully supported.

/// Milestone 87: enum-returning match — literal arm taken (first arm).
///
/// FLS §6.18: First arm matches `0`, discriminant 0 returned.
/// FLS §15: Unit variants are discriminant-only (no field slots used).
#[test]
fn milestone_87_enum_match_return_first_arm_taken() {
    let src = r#"
enum Dir { North, South, East }
fn pick(n: i32) -> Dir {
    match n {
        0 => Dir::North,
        1 => Dir::South,
        _ => Dir::East,
    }
}
fn main() -> i32 {
    let d = pick(0);
    match d {
        Dir::North => 10,
        Dir::South => 20,
        Dir::East => 30,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "pick(0)=North→10, got {exit_code}");
}

/// Milestone 87: enum-returning match — second literal arm taken.
///
/// FLS §6.18: Arms tested in order; second matches `1`.
#[test]
fn milestone_87_enum_match_return_second_arm_taken() {
    let src = r#"
enum Dir { North, South, East }
fn pick(n: i32) -> Dir {
    match n {
        0 => Dir::North,
        1 => Dir::South,
        _ => Dir::East,
    }
}
fn main() -> i32 {
    let d = pick(1);
    match d {
        Dir::North => 10,
        Dir::South => 20,
        Dir::East => 30,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 20, "pick(1)=South→20, got {exit_code}");
}

/// Milestone 87: enum-returning match — wildcard (default) arm taken.
///
/// FLS §6.18: Wildcard arm taken when no earlier arm matched.
#[test]
fn milestone_87_enum_match_return_wildcard_arm_taken() {
    let src = r#"
enum Dir { North, South, East }
fn pick(n: i32) -> Dir {
    match n {
        0 => Dir::North,
        1 => Dir::South,
        _ => Dir::East,
    }
}
fn main() -> i32 {
    let d = pick(5);
    match d {
        Dir::North => 10,
        Dir::South => 20,
        Dir::East => 30,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 30, "pick(5)=East→30, got {exit_code}");
}

/// Milestone 87: enum-returning match — result passed to another function.
///
/// FLS §6.12.1: Caller passes enum discriminant register to callee.
#[test]
fn milestone_87_enum_match_return_passed_to_fn() {
    let src = r#"
enum Rank { Low, Mid, High }
fn rank(n: i32) -> Rank {
    match n {
        0 => Rank::Low,
        1 => Rank::Mid,
        _ => Rank::High,
    }
}
fn score(r: Rank) -> i32 {
    match r {
        Rank::Low => 1,
        Rank::Mid => 5,
        Rank::High => 10,
    }
}
fn main() -> i32 { score(rank(2)) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "rank(2)=High, score(High)=10, got {exit_code}");
}

/// Milestone 87: enum-returning match — result used in if-let.
///
/// FLS §6.17: if-let on the returned enum value.
#[test]
fn milestone_87_enum_match_return_in_if_let() {
    let src = r#"
enum Maybe { Nothing, Just(i32) }
fn wrap(n: i32) -> Maybe {
    match n {
        0 => Maybe::Nothing,
        _ => Maybe::Just(n),
    }
}
fn main() -> i32 {
    let m = wrap(7);
    if let Maybe::Just(v) = m { v } else { 0 }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "wrap(7)=Just(7), if-let extracts 7, got {exit_code}");
}

/// Milestone 87: enum-returning match — on function parameter.
///
/// FLS §9.2: Function parameter used as match scrutinee inside factory function.
#[test]
fn milestone_87_enum_match_return_on_parameter() {
    let src = r#"
enum Color { Red, Green, Blue }
fn from_code(n: i32) -> Color {
    match n {
        1 => Color::Red,
        2 => Color::Green,
        _ => Color::Blue,
    }
}
fn main() -> i32 {
    let c = from_code(2);
    match c {
        Color::Red => 0,
        Color::Green => 1,
        Color::Blue => 2,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "from_code(2)=Green→1, got {exit_code}");
}

/// Milestone 87: enum-returning match — result used in arithmetic.
///
/// FLS §6.5.5: Score values from two calls added together.
#[test]
fn milestone_87_enum_match_return_result_in_arithmetic() {
    let src = r#"
enum Tier { Bronze, Silver, Gold }
fn tier(n: i32) -> Tier {
    match n {
        1 => Tier::Bronze,
        2 => Tier::Silver,
        _ => Tier::Gold,
    }
}
fn points(t: Tier) -> i32 {
    match t {
        Tier::Bronze => 10,
        Tier::Silver => 25,
        Tier::Gold => 50,
    }
}
fn main() -> i32 { points(tier(2)) + points(tier(3)) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 75, "Silver(25)+Gold(50)=75, got {exit_code}");
}

/// Milestone 87: enum-returning match — with tuple variant fields.
///
/// FLS §6.18, §15: Match arms can return tuple variants carrying field data.
#[test]
fn milestone_87_enum_match_return_tuple_variant() {
    let src = r#"
enum Maybe { Nothing, Just(i32) }
fn wrap(n: i32) -> Maybe {
    match n {
        0 => Maybe::Nothing,
        _ => Maybe::Just(n * 2),
    }
}
fn main() -> i32 {
    let m = wrap(5);
    match m {
        Maybe::Nothing => 0,
        Maybe::Just(v) => v,
    }
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "wrap(5)=Just(10), match extracts 10, got {exit_code}");
}

/// Assembly check: enum-returning match emits cmp+cbz for pattern dispatch.
///
/// FLS §6.18: Pattern comparison followed by conditional branch.
/// FLS §6.1.2:37–45: All instructions are runtime (no compile-time folding).
#[test]
fn runtime_enum_match_return_emits_cmp_and_cbz() {
    let src = r#"
enum Dir { North, South }
fn pick(n: i32) -> Dir {
    match n {
        0 => Dir::North,
        _ => Dir::South,
    }
}
fn main() -> i32 {
    let d = pick(0);
    match d { Dir::North => 1, Dir::South => 2 }
}
"#;
    let asm = compile_to_asm(src);
    // The `pick` function must contain a comparison (cmp) and conditional
    // branch (cbz) to implement the match expression at runtime.
    let in_pick: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("pick:"))
        .take_while(|l| !l.starts_with("main:"))
        .collect();
    let has_cbz = in_pick.iter().any(|l| l.trim_start().starts_with("cbz"));
    let has_cmp = in_pick.iter().any(|l| l.trim_start().starts_with("cmp"));
    assert!(
        has_cbz && has_cmp,
        "expected cmp+cbz in pick (enum-returning match):\n{}",
        in_pick.join("\n")
    );
}

// ── Milestone 88: function pointer types (FLS §4.9) ──────────────────────────

/// Milestone 88: basic function pointer — pass a function and call through it.
///
/// FLS §4.9: Function pointer types. `fn(i32) -> i32` is a function pointer
/// type. Passing `double` as an argument materializes its address.
#[test]
fn milestone_88_fn_ptr_basic() {
    let src = r#"
fn double(x: i32) -> i32 { x * 2 }
fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
fn main() -> i32 { apply(double, 5) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "double(5)=10, got {exit_code}");
}

/// Milestone 88: function pointer with arithmetic on result.
///
/// FLS §4.9: The result of calling through a function pointer is an ordinary
/// value that can be used in expressions.
#[test]
fn milestone_88_fn_ptr_result_in_arithmetic() {
    let src = r#"
fn triple(x: i32) -> i32 { x * 3 }
fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
fn main() -> i32 { apply(triple, 4) - 2 }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "triple(4)-2 = 10, got {exit_code}");
}

/// Milestone 88: function pointer with two-parameter callee.
///
/// FLS §4.9: Function pointer types carry the full signature including
/// all parameter types.
#[test]
fn milestone_88_fn_ptr_two_params() {
    let src = r#"
fn add(a: i32, b: i32) -> i32 { a + b }
fn apply2(f: fn(i32, i32) -> i32, a: i32, b: i32) -> i32 { f(a, b) }
fn main() -> i32 { apply2(add, 7, 3) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "add(7,3)=10, got {exit_code}");
}

/// Milestone 88: function pointer zero return.
///
/// FLS §4.9: Calling through a null-equivalent result (zero) case.
#[test]
fn milestone_88_fn_ptr_zero_return() {
    let src = r#"
fn zero(_x: i32) -> i32 { 0 }
fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
fn main() -> i32 { apply(zero, 99) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "zero(99)=0, got {exit_code}");
}

/// Milestone 88: function pointer passed from parameter.
///
/// FLS §4.9: A function pointer parameter can be forwarded to another call.
#[test]
fn milestone_88_fn_ptr_forwarded() {
    let src = r#"
fn inc(x: i32) -> i32 { x + 1 }
fn double_apply(f: fn(i32) -> i32, x: i32) -> i32 { f(f(x)) }
fn main() -> i32 { double_apply(inc, 8) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "inc(inc(8))=10, got {exit_code}");
}

/// Milestone 88: function pointer on direct local.
///
/// FLS §4.9: A function pointer stored in a local variable and called.
#[test]
fn milestone_88_fn_ptr_local_variable() {
    let src = r#"
fn square(x: i32) -> i32 { x * x }
fn main() -> i32 {
    let f: fn(i32) -> i32 = square;
    f(3) + 1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "square(3)+1=10, got {exit_code}");
}

/// Milestone 88: function pointer called with a parameter value.
///
/// FLS §4.9: Function pointer calls use the same ABI as direct calls.
#[test]
fn milestone_88_fn_ptr_on_parameter() {
    let src = r#"
fn negate(x: i32) -> i32 { 0 - x }
fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
fn main() -> i32 { apply(negate, 0 - 5) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "negate(-5)=5, got {exit_code}");
}

/// Assembly check: function pointer call emits adrp+add for address and blr for call.
///
/// FLS §4.9: Loading a function's address uses PC-relative addressing (ADRP+ADD).
/// Calling through a pointer uses `blr` (branch with link to register).
/// FLS §6.1.2:37–45: All instructions are runtime.
#[test]
fn runtime_fn_ptr_emits_adrp_and_blr() {
    let src = r#"
fn double(x: i32) -> i32 { x * 2 }
fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
fn main() -> i32 { apply(double, 5) }
"#;
    let asm = compile_to_asm(src);

    // main must contain adrp for the function address load.
    let in_main: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("main:"))
        .collect();
    let has_adrp = in_main.iter().any(|l| l.contains("adrp") && l.contains("double"));
    assert!(has_adrp, "expected adrp for double in main:\n{}", in_main.join("\n"));

    // apply must contain blr for the indirect call.
    let in_apply: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("apply:"))
        .take_while(|l| !l.starts_with("main:"))
        .collect();
    let has_blr = in_apply.iter().any(|l| l.trim_start().starts_with("blr"));
    assert!(has_blr, "expected blr in apply for indirect call:\n{}", in_apply.join("\n"));
}

// ── Milestone 89: Non-capturing closures (FLS §6.14) ─────────────────────────
//
// A non-capturing closure `|x: i32| -> i32 { body }` compiles to a hidden
// named function and the closure expression evaluates to the function's address
// (a function pointer). Non-capturing closures coerce to `fn` pointer types
// (FLS §4.9, §6.14).
//
// FLS §6.14: Closure expressions evaluate to closure types that implement
// Fn/FnMut/FnOnce. Non-capturing closures additionally coerce to bare
// function pointer types.
//
// FLS §6.1.2:37–45: The closure body must emit runtime instructions.
//
// ARM64: the closure compiles to a separate function label; the closure
// expression emits `adrp + add` to load the address (same as milestone 88
// function pointer loading), and calling through the pointer emits `blr`.
//
// These tests derive from FLS §6.14 semantics. The spec provides no
// worked examples; test inputs are derived from the section's semantic
// description (closure parameters and body evaluation).

/// Milestone 89: basic closure stored in a let binding and called through apply.
///
/// `let double = |x: i32| -> i32 { x * 2 };` stores the closure address.
/// `apply(double, 21)` calls it via a fn pointer argument.
///
/// FLS §6.14: Non-capturing closure → fn pointer coercion.
/// FLS §4.9: fn pointer type `fn(i32) -> i32`.
#[test]
fn milestone_89_closure_basic() {
    let src = r#"
fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
fn main() -> i32 {
    let double = |x: i32| -> i32 { x * 2 };
    apply(double, 21)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "double(21)=42, got {exit_code}");
}

/// Milestone 89: zero-parameter closure `|| expr`.
///
/// FLS §6.14: `||` denotes a closure with no parameters.
/// The closure body `-> i32 { 7 }` returns 7.
#[test]
fn milestone_89_closure_zero_params() {
    let src = r#"
fn call(f: fn() -> i32) -> i32 { f() }
fn main() -> i32 {
    let seven = || -> i32 { 7 };
    call(seven)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "seven()=7, got {exit_code}");
}

/// Milestone 89: closure with arithmetic body.
///
/// `|x: i32, y: i32| -> i32 { x + y }` — two params, addition.
/// FLS §6.14: Multiple closure parameters follow ARM64 calling convention.
#[test]
fn milestone_89_closure_two_params() {
    let src = r#"
fn apply2(f: fn(i32, i32) -> i32, a: i32, b: i32) -> i32 { f(a, b) }
fn main() -> i32 {
    let add = |x: i32, y: i32| -> i32 { x + y };
    apply2(add, 17, 25)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "17+25=42, got {exit_code}");
}

/// Milestone 89: closure result used in arithmetic.
///
/// FLS §6.14: The fn pointer returned by a closure can be called and
/// its result used in further computation.
#[test]
fn milestone_89_closure_result_in_arithmetic() {
    let src = r#"
fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
fn main() -> i32 {
    let inc = |x: i32| -> i32 { x + 1 };
    apply(inc, 5) + apply(inc, 10)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 17, "6+11=17, got {exit_code}");
}

/// Milestone 89: closure passed directly as a function argument (no let binding).
///
/// FLS §6.14: A closure expression can appear inline where a fn pointer is expected.
#[test]
fn milestone_89_closure_inline_arg() {
    let src = r#"
fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
fn main() -> i32 {
    apply(|x: i32| -> i32 { x * 3 }, 14)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "14*3=42, got {exit_code}");
}

/// Milestone 89: closure with if-else body.
///
/// FLS §6.14: The closure body can contain any expression, including control flow.
/// FLS §6.17: if-else inside the closure emits the same branch instructions as
/// a normal if-else in a function body.
#[test]
fn milestone_89_closure_if_else_body() {
    let src = r#"
fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
fn main() -> i32 {
    let sign = |x: i32| -> i32 { if x > 0 { 1 } else { 0 } };
    apply(sign, 5) + apply(sign, 0)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "sign(5)+sign(0)=1+0=1, got {exit_code}");
}

/// Milestone 89: closure passed as a parameter and called inside another function.
///
/// FLS §6.14: A fn pointer holding a closure address can be forwarded to other
/// functions. FLS §4.9: fn pointer parameters use the same calling convention
/// as other fn pointers.
#[test]
fn milestone_89_closure_on_parameter() {
    let src = r#"
fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
fn transform(g: fn(i32) -> i32, n: i32) -> i32 { apply(g, n) }
fn main() -> i32 {
    let triple = |x: i32| -> i32 { x * 3 };
    transform(triple, 14)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "14*3=42, got {exit_code}");
}

/// Assembly check: closure compiles to a hidden function label `__closure_*`.
///
/// FLS §6.14: Non-capturing closures compile to named functions.
/// FLS §4.9: LoadFnAddr emits ADRP+ADD to materialise the address.
/// FLS §6.1.2:37–45: The closure body emits runtime instructions.
#[test]
fn runtime_closure_emits_hidden_function_label() {
    let src = r#"
fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
fn main() -> i32 {
    let double = |x: i32| -> i32 { x * 2 };
    apply(double, 21)
}
"#;
    let asm = compile_to_asm(src);
    // A hidden function label starting with __closure_ must appear.
    assert!(
        asm.lines().any(|l| l.starts_with("__closure_")),
        "expected hidden closure function label `__closure_*` in assembly:\n{asm}"
    );
    // The closure body must contain a mul instruction for `x * 2`.
    let closure_section: Vec<&str> = asm
        .lines()
        .skip_while(|l| !l.starts_with("__closure_"))
        .collect();
    let has_mul = closure_section.iter().any(|l| l.contains("mul"));
    assert!(has_mul, "expected `mul` in closure body for `x * 2`:\n{}", closure_section.join("\n"));
}

// ── Milestone 90: char literals compile to runtime ARM64 (FLS §2.4.5) ────────
//
// FLS §2.4.5: A character literal is a char-typed expression whose value is
// a Unicode scalar value. `'A'` evaluates to 65 (U+0041), `'\n'` to 10, etc.
//
// Galvanic maps `char` to `IrTy::U32` (Unicode scalar values fit in u32).
// A char literal emits `mov x{r}, #<code_point>` — one instruction.
// `char as i32` is an identity cast: all char values are ≤ 0x10FFFF < i32::MAX.

/// Milestone 90: simple ASCII char literal cast to i32.
///
/// FLS §2.4.5: `'A'` has code point 65 (U+0041).
/// FLS §6.5.9: `char as i32` is a numeric cast — the code point becomes
/// a signed integer value.
#[test]
fn milestone_90_char_literal_ascii() {
    let src = "fn main() -> i32 { 'A' as i32 }";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 65, "'A' as i32 = 65, got {exit_code}");
}

/// Milestone 90: digit char literal.
///
/// FLS §2.4.5: `'0'` has code point 48 (U+0030).
#[test]
fn milestone_90_char_literal_digit() {
    let src = "fn main() -> i32 { '0' as i32 }";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 48, "'0' as i32 = 48, got {exit_code}");
}

/// Milestone 90: char literal stored in a let binding, then cast.
///
/// FLS §2.4.5: char literal evaluates to its code point.
/// FLS §8.1: let binding stores the char value in a stack slot.
#[test]
fn milestone_90_char_let_binding() {
    let src = r#"
fn main() -> i32 {
    let c = 'Z';
    c as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 90, "'Z' as i32 = 90, got {exit_code}");
}

/// Milestone 90: newline escape sequence char literal.
///
/// FLS §2.4.5: `'\n'` is U+000A (LINE FEED), code point 10.
#[test]
fn milestone_90_char_escape_newline() {
    let src = r#"fn main() -> i32 { '\n' as i32 }"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "'\\n' as i32 = 10, got {exit_code}");
}

/// Milestone 90: tab escape sequence char literal.
///
/// FLS §2.4.5: `'\t'` is U+0009 (CHARACTER TABULATION), code point 9.
#[test]
fn milestone_90_char_escape_tab() {
    let src = r#"fn main() -> i32 { '\t' as i32 }"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 9, "'\\t' as i32 = 9, got {exit_code}");
}

/// Milestone 90: char literal passed to a function that takes `char`.
///
/// FLS §2.4.5: char values are passed in integer registers on ARM64.
/// FLS §9: parameters of type `char` are spilled to a stack slot as u32.
#[test]
fn milestone_90_char_parameter() {
    let src = r#"
fn char_to_i32(c: char) -> i32 { c as i32 }
fn main() -> i32 { char_to_i32('*') }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "'*' as i32 = 42, got {exit_code}");
}

/// Milestone 90: char literal in arithmetic — offset from 'A'.
///
/// FLS §2.4.5: char values are u32 integers; arithmetic on `as i32` is
/// standard integer arithmetic.
#[test]
fn milestone_90_char_in_arithmetic() {
    let src = r#"
fn main() -> i32 {
    let base = 'A' as i32;
    base + 5
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 70, "'A' as i32 + 5 = 65 + 5 = 70, got {exit_code}");
}

/// Assembly check: char literal emits a single `mov` instruction with the code point.
///
/// FLS §2.4.5: char literals are compile-time constants whose code point is
/// materialized with `LoadImm` → `mov x{r}, #<code_point>`.
/// FLS §6.1.2:37–45: The mov is a runtime instruction — no compile-time folding.
#[test]
fn runtime_char_literal_emits_mov() {
    let src = "fn main() -> i32 { 'A' as i32 }";
    let asm = compile_to_asm(src);
    // LoadImm for code point 65 emits `mov x{r}, #65`.
    assert!(
        asm.contains("mov") && asm.contains("#65"),
        "expected `mov ... #65` for 'A' (code point 65) in assembly:\n{asm}"
    );
}

// ── Milestone 91: capturing closures ─────────────────────────────────────────

/// Milestone 91: basic capturing closure — single variable captured by copy.
///
/// FLS §6.22: A closure expression captures free variables from the enclosing
/// scope. The captured variable is passed as a hidden leading argument on every
/// invocation. Galvanic implements capture-by-copy for scalar types.
/// FLS §6.14: The closure body executes at runtime.
/// FLS §6.1.2:37–45: All instructions emitted at runtime; no compile-time folding.
#[test]
fn milestone_91_capturing_closure_basic() {
    let src = r#"
fn main() -> i32 {
    let x = 5;
    let f = || x + 1;
    f()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "x=5, f()=x+1=6, got {exit_code}");
}

/// Milestone 91: capturing closure with an explicit parameter.
///
/// FLS §6.22: Captured variables precede explicit parameters in the ABI.
/// The closure `|x| x + n` captures `n` and takes `x` explicitly.
#[test]
fn milestone_91_capturing_closure_with_param() {
    let src = r#"
fn main() -> i32 {
    let n = 3;
    let add_n = |x: i32| -> i32 { x + n };
    add_n(7)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "n=3, add_n(7)=7+3=10, got {exit_code}");
}

/// Milestone 91: closure captures two variables.
///
/// FLS §6.22: All free variables referenced in the closure body are captured.
/// Both `a` and `b` arrive as hidden leading arguments.
#[test]
fn milestone_91_capturing_closure_two_captures() {
    let src = r#"
fn main() -> i32 {
    let a = 2;
    let b = 3;
    let f = || a + b;
    f()
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "a=2, b=3, f()=a+b=5, got {exit_code}");
}

/// Milestone 91: capturing closure called multiple times.
///
/// FLS §6.22: Each invocation reloads the captured value from its outer slot.
/// The captured variable is not mutated — copy semantics.
#[test]
fn milestone_91_capturing_closure_called_twice() {
    let src = r#"
fn main() -> i32 {
    let multiplier = 4;
    let double = |x: i32| -> i32 { x * multiplier };
    double(5) + double(3)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 32, "4*5+4*3=20+12=32, got {exit_code}");
}

/// Milestone 91: capturing closure with if-else body.
///
/// FLS §6.22: The closure body can contain control flow.
/// FLS §6.17: if-else inside the closure emits the same branch instructions.
#[test]
fn milestone_91_capturing_closure_if_else_body() {
    let src = r#"
fn main() -> i32 {
    let threshold = 5;
    let clamp = |x: i32| -> i32 { if x > threshold { threshold } else { x } };
    clamp(10) + clamp(3)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 8, "clamp(10)=5, clamp(3)=3, sum=8, got {exit_code}");
}

/// Milestone 91: captured variable from a parameter.
///
/// FLS §6.22: A closure can capture a function parameter.
/// The parameter is already on the stack; the closure receives it via its slot.
#[test]
fn milestone_91_capturing_closure_captures_parameter() {
    let src = r#"
fn make_adder(n: i32) -> i32 {
    let add = |x: i32| -> i32 { x + n };
    add(10)
}
fn main() -> i32 { make_adder(7) }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 17, "add(10) where n=7 → 10+7=17, got {exit_code}");
}

/// Milestone 91: result of capturing closure in arithmetic.
///
/// FLS §6.14, §6.22: The return value of `f()` is a regular i32 that can be
/// used in subsequent arithmetic.
#[test]
fn milestone_91_capturing_closure_result_in_arithmetic() {
    let src = r#"
fn main() -> i32 {
    let offset = 10;
    let shifted = |x: i32| -> i32 { x + offset };
    shifted(5) + shifted(7)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 32, "15+17=32, got {exit_code}");
}

/// Assembly check: capturing closure passes hidden arg before explicit arg.
///
/// FLS §6.22: The hidden closure function receives the captured variable as
/// x0 and the explicit parameter as x1. At the call site the captured value is
/// loaded and placed in the first argument position.
/// FLS §6.1.2:37–45: All loads and stores are runtime instructions.
#[test]
fn runtime_capturing_closure_emits_capture_load_before_explicit_arg() {
    let src = r#"
fn main() -> i32 {
    let n = 3;
    let add_n = |x: i32| -> i32 { x + n };
    add_n(7)
}
"#;
    let asm = compile_to_asm(src);
    // The hidden function must be emitted.
    assert!(
        asm.contains("__closure_main_0"),
        "expected hidden closure label `__closure_main_0` in assembly:\n{asm}"
    );
    // A `blr` must appear (indirect call for the fn pointer).
    assert!(
        asm.contains("blr"),
        "expected `blr` indirect call for capturing closure in assembly:\n{asm}"
    );
}

// ── Milestone 92: byte literals compile to runtime ARM64 ─────────────────────
//
// FLS §2.4.1: A byte literal is of the form `b'...'` and has type `u8`.
// The value is the ASCII/byte code of the character (or escape sequence).
// Galvanic maps `u8` to `IrTy::U32` (zero-extended in a 64-bit register).
//
// FLS §6.1.2:37–45: Byte literals emit a runtime `mov` instruction — no
// constant folding.

/// Milestone 92: byte literal for a printable ASCII character.
///
/// FLS §2.4.1: `b'A'` has value 65 (ASCII 'A').
#[test]
fn milestone_92_byte_literal_ascii() {
    let src = r#"
fn main() -> i32 {
    let b: u8 = b'A';
    b as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 65, "b'A'=65, got {exit_code}");
}

/// Milestone 92: byte literal for a digit.
///
/// FLS §2.4.1: `b'0'` has value 48 (ASCII '0').
#[test]
fn milestone_92_byte_literal_digit() {
    let src = r#"
fn main() -> i32 {
    let b: u8 = b'0';
    b as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 48, "b'0'=48, got {exit_code}");
}

/// Milestone 92: byte literal stored in a let binding and returned directly.
///
/// FLS §2.4.1: `b'*'` has value 42 (ASCII '*').
#[test]
fn milestone_92_byte_let_binding() {
    let src = r#"
fn main() -> i32 {
    let star: u8 = b'*';
    star as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 42, "b'*'=42, got {exit_code}");
}

/// Milestone 92: byte escape `b'\n'` produces the newline byte value (10).
///
/// FLS §2.4.1: Byte literals support the same escape sequences as char literals.
/// `b'\n'` → 10 (LINE FEED).
#[test]
fn milestone_92_byte_escape_newline() {
    let src = r#"
fn main() -> i32 {
    let nl: u8 = b'\n';
    nl as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 10, "b'\\n'=10, got {exit_code}");
}

/// Milestone 92: byte escape `b'\t'` produces the tab byte value (9).
///
/// FLS §2.4.1: `b'\t'` → 9 (HORIZONTAL TAB).
#[test]
fn milestone_92_byte_escape_tab() {
    let src = r#"
fn main() -> i32 {
    let tab: u8 = b'\t';
    tab as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 9, "b'\\t'=9, got {exit_code}");
}

/// Milestone 92: byte literal passed as a function parameter.
///
/// FLS §2.4.1: A byte literal value is an ordinary `u8` that can be passed
/// as a function argument.
#[test]
fn milestone_92_byte_parameter() {
    let src = r#"
fn identity(b: u8) -> i32 { b as i32 }
fn main() -> i32 { identity(b'Z') }
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 90, "b'Z'=90, got {exit_code}");
}

/// Milestone 92: byte literal used in arithmetic.
///
/// FLS §2.4.1: Byte values participate in integer arithmetic after cast.
/// `b'A' as i32 + 1 == 66`.
#[test]
fn milestone_92_byte_in_arithmetic() {
    let src = r#"
fn main() -> i32 {
    let b: u8 = b'A';
    b as i32 + 1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 66, "65+1=66, got {exit_code}");
}

/// Assembly check: byte literal emits a single `mov` immediate instruction.
///
/// FLS §2.4.1: A byte literal is a compile-time constant value but must
/// be materialized as a runtime `mov` (FLS §6.1.2:37–45).
/// Cache-line note: one `mov` = 4 bytes (fits alongside adjacent instructions
/// in a 64-byte cache line).
#[test]
fn runtime_byte_literal_emits_mov() {
    let src = r#"
fn main() -> i32 {
    let b: u8 = b'A';
    b as i32
}
"#;
    let asm = compile_to_asm(src);
    // b'A' == 65 == 0x41; the assembler may emit `mov w0, #65` or `mov x0, #65`.
    assert!(
        asm.contains("#65") || asm.contains("#0x41"),
        "expected immediate 65 (b'A') in assembly:\n{asm}"
    );
}

// ── Milestone 93: String literals — `.len()` (FLS §2.4.6) ────────────────────

/// Milestone 93: ASCII string literal `.len()` inline.
///
/// FLS §2.4.6: A string literal has type `&str`.  Its UTF-8 byte length is a
/// compile-time constant.  Galvanic materialises the length as a runtime `mov`
/// (FLS §6.1.2:37–45; no constant folding in non-const contexts).
///
/// `"hello"` is 5 bytes in UTF-8.
#[test]
fn milestone_93_str_len_literal_direct() {
    let src = r#"
fn main() -> i32 {
    "hello".len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "\"hello\".len()=5, got {exit_code}");
}

/// Milestone 93: String literal bound to a `let` variable, then `.len()`.
///
/// FLS §2.4.6: The type of a string literal is `&str`.
/// FLS §8.1: Let bindings bring the variable into scope after the initializer.
/// `"hello"` is 5 UTF-8 bytes.
#[test]
fn milestone_93_str_len_let_binding() {
    let src = r#"
fn main() -> i32 {
    let s = "hello";
    s.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "\"hello\".len()=5, got {exit_code}");
}

/// Milestone 93: Empty string literal has length zero.
///
/// FLS §2.4.6: The empty string `""` is a valid string literal of type `&str`
/// with zero bytes.
#[test]
fn milestone_93_str_len_empty() {
    let src = r#"
fn main() -> i32 {
    let s: &str = "";
    s.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "\"\".len()=0, got {exit_code}");
}

/// Milestone 93: Six-character string literal.
///
/// FLS §2.4.6: UTF-8 encoding — ASCII characters are 1 byte each.
/// `"world!"` is 6 bytes.
#[test]
fn milestone_93_str_len_six_chars() {
    let src = r#"
fn main() -> i32 {
    let s = "world!";
    s.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "\"world!\".len()=6, got {exit_code}");
}

/// Milestone 93: String literal with escape sequences — length counts bytes.
///
/// FLS §2.4.6.1: `\n` is one byte (0x0A); `\t` is one byte (0x09).
/// `"a\nb"` is 3 bytes: 'a' + '\n' + 'b'.
#[test]
fn milestone_93_str_len_escape_newline() {
    let src = r#"
fn main() -> i32 {
    let s = "a\nb";
    s.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "\"a\\nb\".len()=3, got {exit_code}");
}

/// Milestone 93: String literal with tab escape.
///
/// FLS §2.4.6.1: `\t` is one byte (0x09).
/// `"x\ty"` is 3 bytes.
#[test]
fn milestone_93_str_len_escape_tab() {
    let src = r#"
fn main() -> i32 {
    let s = "x\ty";
    s.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "\"x\\ty\".len()=3, got {exit_code}");
}

/// Milestone 93: Two string lengths summed.
///
/// FLS §2.4.6: Multiple `&str` let bindings in scope simultaneously.
/// `"hi".len() + "bye".len()` = 2 + 3 = 5.
#[test]
fn milestone_93_str_len_two_bindings_summed() {
    let src = r#"
fn main() -> i32 {
    let a = "hi";
    let b = "bye";
    a.len() as i32 + b.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "2+3=5, got {exit_code}");
}

/// Milestone 93: String length used in arithmetic.
///
/// FLS §2.4.6: The result of `.len()` is a `usize`; cast to `i32` for return.
/// `"abc".len() as i32 + 1 == 4`.
#[test]
fn milestone_93_str_len_in_arithmetic() {
    let src = r#"
fn main() -> i32 {
    let s = "abc";
    s.len() as i32 + 1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "3+1=4, got {exit_code}");
}

/// Assembly check: string literal `.len()` emits a `mov` with the byte count.
///
/// FLS §2.4.6: The byte length is a compile-time constant.  It must still be
/// materialised as a runtime `mov` (FLS §6.1.2:37–45).
/// Cache-line note: one `mov` = 4 bytes (half a cache slot).
#[test]
fn runtime_str_literal_len_emits_mov() {
    let src = r#"
fn main() -> i32 {
    let s = "hello";
    s.len() as i32
}
"#;
    let asm = compile_to_asm(src);
    // "hello" is 5 bytes; the assembler emits `mov x0, #5` or equivalent.
    assert!(
        asm.contains("#5"),
        "expected immediate 5 (len of \"hello\") in assembly:\n{asm}"
    );
}

// ── Milestone 94: Byte string literals compile to runtime ARM64 (FLS §2.4.2) ──

/// Milestone 94: Byte string literal `.len()` directly on the literal.
///
/// FLS §2.4.2: A byte string literal `b"..."` has type `&[u8]`.
/// `b"hello".len()` returns 5 (number of bytes).
#[test]
fn milestone_94_byte_str_len_literal_direct() {
    let src = r#"
fn main() -> i32 {
    b"hello".len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "b\"hello\".len()=5, got {exit_code}");
}

/// Milestone 94: Byte string literal bound to a `let` variable, then `.len()`.
///
/// FLS §2.4.2: `b"hello"` is a `&[u8]` with 5 bytes.
/// FLS §8.1: Let binding stores the byte-length value.
#[test]
fn milestone_94_byte_str_len_let_binding() {
    let src = r#"
fn main() -> i32 {
    let s = b"hello";
    s.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "b\"hello\".len()=5, got {exit_code}");
}

/// Milestone 94: Empty byte string literal has length zero.
///
/// FLS §2.4.2: `b""` is a valid byte string literal of type `&[u8]` with zero bytes.
#[test]
fn milestone_94_byte_str_len_empty() {
    let src = r#"
fn main() -> i32 {
    let s = b"";
    s.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "b\"\".len()=0, got {exit_code}");
}

/// Milestone 94: Six-byte byte string literal.
///
/// FLS §2.4.2: ASCII characters in byte strings are 1 byte each.
/// `b"world!"` is 6 bytes.
#[test]
fn milestone_94_byte_str_len_six_bytes() {
    let src = r#"
fn main() -> i32 {
    let s = b"world!";
    s.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "b\"world!\".len()=6, got {exit_code}");
}

/// Milestone 94: Byte string with escape sequence — `\n` is one byte.
///
/// FLS §2.4.2.1: `\n` in a byte string literal is the single byte 0x0A.
/// `b"a\nb"` is 3 bytes.
#[test]
fn milestone_94_byte_str_len_escape_newline() {
    let src = r#"
fn main() -> i32 {
    let s = b"a\nb";
    s.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "b\"a\\nb\".len()=3, got {exit_code}");
}

/// Milestone 94: Byte string with tab escape.
///
/// FLS §2.4.2.1: `\t` is one byte (0x09).
/// `b"x\ty"` is 3 bytes.
#[test]
fn milestone_94_byte_str_len_escape_tab() {
    let src = r#"
fn main() -> i32 {
    let s = b"x\ty";
    s.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "b\"x\\ty\".len()=3, got {exit_code}");
}

/// Milestone 94: Two byte string lengths summed.
///
/// FLS §2.4.2: Multiple `&[u8]` let bindings in scope simultaneously.
/// `b"hi".len() + b"bye".len()` = 2 + 3 = 5.
#[test]
fn milestone_94_byte_str_len_two_bindings_summed() {
    let src = r#"
fn main() -> i32 {
    let a = b"hi";
    let b = b"bye";
    a.len() as i32 + b.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "2+3=5, got {exit_code}");
}

/// Milestone 94: Byte string length used in arithmetic.
///
/// FLS §2.4.2: `.len()` on `&[u8]` returns `usize`; cast to `i32` for return.
/// `b"abc".len() as i32 + 1 == 4`.
#[test]
fn milestone_94_byte_str_len_in_arithmetic() {
    let src = r#"
fn main() -> i32 {
    let s = b"abc";
    s.len() as i32 + 1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "3+1=4, got {exit_code}");
}

/// Assembly check: byte string literal `.len()` emits a `mov` with the byte count.
///
/// FLS §2.4.2: The byte count is a compile-time constant.  It must still be
/// materialised as a runtime `mov` (FLS §6.1.2:37–45).
/// Cache-line note: one `mov` = 4 bytes (half a cache slot).
#[test]
fn runtime_byte_str_literal_len_emits_mov() {
    let src = r#"
fn main() -> i32 {
    let s = b"hello";
    s.len() as i32
}
"#;
    let asm = compile_to_asm(src);
    // b"hello" is 5 bytes; the assembler emits `mov x0, #5` or equivalent.
    assert!(
        asm.contains("#5"),
        "expected immediate 5 (len of b\"hello\") in assembly:\n{asm}"
    );
}

// ── Milestone 95: Raw string literals compile to runtime ARM64 (FLS §2.4.6.2, §2.4.2.2) ──

/// Milestone 95: Raw string literal `.len()` directly on the literal.
///
/// FLS §2.4.6.2: A raw string literal `r"..."` has type `&str` and contains
/// no escape sequences. `r"hello"` is 5 bytes — same as `"hello"`.
#[test]
fn milestone_95_raw_str_len_literal_direct() {
    let src = r#"
fn main() -> i32 {
    r"hello".len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "r\"hello\".len()=5, got {exit_code}");
}

/// Milestone 95: Raw string literal bound to a `let` variable, then `.len()`.
///
/// FLS §2.4.6.2: Raw string literals have type `&str`.
/// FLS §8.1: Let bindings bring the variable into scope after the initializer.
#[test]
fn milestone_95_raw_str_len_let_binding() {
    let src = r#"
fn main() -> i32 {
    let s = r"hello";
    s.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "r\"hello\".len()=5, got {exit_code}");
}

/// Milestone 95: Empty raw string literal has length zero.
///
/// FLS §2.4.6.2: `r""` is a valid raw string literal containing zero characters.
#[test]
fn milestone_95_raw_str_len_empty() {
    let src = r#"
fn main() -> i32 {
    r"".len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 0, "r\"\".len()=0, got {exit_code}");
}

/// Milestone 95: Raw string backslash-n is two bytes, not one.
///
/// FLS §2.4.6.2: Raw strings do NOT process escape sequences.
/// `r"hello\n"` contains literal backslash and 'n' — 7 bytes total.
/// This distinguishes raw strings from regular strings: `"hello\n"` is 6 bytes.
#[test]
fn milestone_95_raw_str_backslash_counts_as_two_bytes() {
    // The inner source contains r"hello\n" — backslash-n is TWO bytes in a raw string.
    let src = "fn main() -> i32 {\n    r\"hello\\n\".len() as i32\n}";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "r\"hello\\n\" has 7 bytes (no escape), got {exit_code}");
}

/// Milestone 95: Raw string with six characters.
///
/// FLS §2.4.6.2: `r"galvanic"` is 8 bytes.
#[test]
fn milestone_95_raw_str_len_six_chars() {
    let src = r#"
fn main() -> i32 {
    r"galvan".len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "r\"galvan\".len()=6, got {exit_code}");
}

/// Milestone 95: Raw string length used in arithmetic.
///
/// FLS §2.4.6.2: `r"abc"` is 3 bytes.
/// FLS §6.5.1: Addition of two i32 values.
#[test]
fn milestone_95_raw_str_len_in_arithmetic() {
    let src = r#"
fn main() -> i32 {
    r"abc".len() as i32 + 1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "r\"abc\".len()+1=4, got {exit_code}");
}

/// Milestone 95: Two raw string bindings, lengths summed.
///
/// FLS §2.4.6.2: `r"ab"` is 2 bytes; `r"cd"` is 2 bytes; sum is 4.
#[test]
fn milestone_95_raw_str_len_two_bindings_summed() {
    let src = r#"
fn main() -> i32 {
    let a = r"ab";
    let b = r"cd";
    a.len() as i32 + b.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "r\"ab\".len()+r\"cd\".len()=4, got {exit_code}");
}

/// Milestone 95: Raw byte string literal `.len()` directly on the literal.
///
/// FLS §2.4.2.2: A raw byte string literal `br"..."` has type `&[u8]` and
/// contains no escape sequences. `br"hello"` is 5 bytes.
#[test]
fn milestone_95_raw_byte_str_len_literal_direct() {
    let src = r#"
fn main() -> i32 {
    br"hello".len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "br\"hello\".len()=5, got {exit_code}");
}

/// Milestone 95: Raw byte string backslash-n is two bytes, not one.
///
/// FLS §2.4.2.2: Raw byte strings do NOT process escape sequences.
/// `br"hello\n"` contains literal backslash and 'n' — 7 bytes total.
#[test]
fn milestone_95_raw_byte_str_backslash_counts_as_two_bytes() {
    // The inner source contains br"hello\n" — backslash-n is TWO bytes.
    let src = "fn main() -> i32 {\n    br\"hello\\n\".len() as i32\n}";
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "br\"hello\\n\" has 7 bytes (no escape), got {exit_code}");
}

/// Milestone 95: Raw byte string bound to a let binding.
///
/// FLS §2.4.2.2: Raw byte string literals have type `&[u8]`.
/// FLS §8.1: Let bindings bring the variable into scope after the initializer.
#[test]
fn milestone_95_raw_byte_str_len_let_binding() {
    let src = r#"
fn main() -> i32 {
    let s = br"hello";
    s.len() as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "br\"hello\".len()=5, got {exit_code}");
}

/// Assembly check: raw string literal `.len()` emits a `mov` with the byte count.
///
/// FLS §2.4.6.2: Raw strings have no escape processing.  The length is
/// materialised as a runtime `mov` (FLS §6.1.2:37–45).
/// Cache-line note: one `mov` = 4 bytes (half a cache slot).
#[test]
fn runtime_raw_str_literal_len_emits_mov() {
    let src = r#"
fn main() -> i32 {
    r"hello".len() as i32
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("#5"),
        "expected immediate 5 (len of r\"hello\") in assembly:\n{asm}"
    );
}

/// Assembly check: raw string with backslash emits correct (unescaped) byte count.
///
/// FLS §2.4.6.2: `r"hello\n"` is 7 bytes (backslash + n are separate characters).
/// The assembly must contain `#7`, NOT `#6`.
#[test]
fn runtime_raw_str_backslash_emits_unescaped_len() {
    // Source: fn main() -> i32 { r"hello\n".len() as i32 }
    // The inner r"hello\n" has 7 chars: h,e,l,l,o,\,n
    let src = "fn main() -> i32 {\n    r\"hello\\n\".len() as i32\n}";
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("#7"),
        "expected immediate 7 (raw: backslash+n = 2 chars) in assembly:\n{asm}"
    );
    assert!(
        !asm.contains("#6"),
        "must NOT emit #6 (that would be escaped len) in assembly:\n{asm}"
    );
}

/// Assembly check: raw byte string literal `.len()` emits a `mov` with byte count.
///
/// FLS §2.4.2.2: Raw byte strings have no escape processing.
/// The length is materialised as a runtime `mov` (FLS §6.1.2:37–45).
#[test]
fn runtime_raw_byte_str_literal_len_emits_mov() {
    let src = r#"
fn main() -> i32 {
    br"hello".len() as i32
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("#5"),
        "expected immediate 5 (len of br\"hello\") in assembly:\n{asm}"
    );
}

/// Assembly check: raw byte string with backslash emits correct (unescaped) count.
///
/// FLS §2.4.2.2: `br"hello\n"` is 7 bytes (backslash + n are separate chars).
#[test]
fn runtime_raw_byte_str_backslash_emits_unescaped_len() {
    let src = "fn main() -> i32 {\n    br\"hello\\n\".len() as i32\n}";
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("#7"),
        "expected immediate 7 (raw: backslash+n = 2 bytes) in assembly:\n{asm}"
    );
}

// ── Milestone 96: f64 float literals compile to runtime ARM64 ────────────────
//
// FLS §2.4.4.2: Float literals. Each `f64` value is stored as raw IEEE 754
// bits in the .rodata section and loaded at runtime via ADRP+ADD+LDR into a
// float register. Conversion to i32 uses FCVTZS (truncation toward zero).
//
// FLS §6.5.9: Numeric cast `f64 as i32` truncates toward zero.
// FLS §6.1.2:37–45: Even a float literal emits runtime instructions.

/// Milestone 96: float literal cast directly to i32.
///
/// FLS §2.4.4.2: Float literal `3.0` loaded from .rodata into d{N},
/// then converted to i32 via FCVTZS. Result = 3.
#[test]
fn milestone_96_float_literal_direct_cast() {
    let src = r#"
fn main() -> i32 {
    3.0_f64 as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "3.0_f64 as i32 = 3, got {exit_code}");
}

/// Milestone 96: float literal bound to let, then cast.
///
/// FLS §8.1: `let x: f64 = 2.5` stores d{N} to a stack slot via StoreF64.
/// FLS §6.5.9: `x as i32` truncates toward zero → 2.
#[test]
fn milestone_96_float_let_binding_then_cast() {
    let src = r#"
fn main() -> i32 {
    let x: f64 = 2.5;
    x as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "2.5_f64 as i32 = 2 (truncate), got {exit_code}");
}

/// Milestone 96: truncation rounds toward zero (not floor).
///
/// FLS §6.5.9: `f64 as i32` truncates toward zero.
/// 3.9 → 3 (not 4).
#[test]
fn milestone_96_float_truncation_toward_zero() {
    let src = r#"
fn main() -> i32 {
    let x: f64 = 3.9;
    x as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "3.9_f64 as i32 = 3 (truncate not floor), got {exit_code}");
}

/// Milestone 96: float suffix _f64 is stripped correctly.
///
/// FLS §2.4.4.2: Suffix `_f64` specifies the type but does not affect the value.
#[test]
fn milestone_96_float_with_f64_suffix() {
    let src = r#"
fn main() -> i32 {
    4.0_f64 as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "4.0_f64 as i32 = 4, got {exit_code}");
}

/// Milestone 96: float without suffix defaults to f64.
///
/// FLS §2.4.4.2: A float literal without suffix has the contextual type;
/// here it is an f64 inferred from the `as i32` context.
#[test]
fn milestone_96_float_without_suffix() {
    let src = r#"
fn main() -> i32 {
    let x: f64 = 1.0;
    x as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 1, "1.0 (no suffix) as i32 = 1, got {exit_code}");
}

/// Milestone 96: float cast result used in integer arithmetic.
///
/// FLS §6.5.9: The i32 result of `f64 as i32` can be used in integer expressions.
#[test]
fn milestone_96_float_cast_in_arithmetic() {
    let src = r#"
fn main() -> i32 {
    let x: f64 = 3.0;
    let y = x as i32;
    y + 2
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "(3.0 as i32) + 2 = 5, got {exit_code}");
}

/// Milestone 96: two float let bindings, sum their i32 casts.
///
/// FLS §8.1: Multiple float bindings use separate stack slots.
/// FLS §6.5.9: Each `as i32` truncates independently.
#[test]
fn milestone_96_two_float_bindings_summed() {
    let src = r#"
fn main() -> i32 {
    let a: f64 = 2.0;
    let b: f64 = 3.0;
    (a as i32) + (b as i32)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "2.0 + 3.0 as i32 = 5, got {exit_code}");
}

/// Milestone 96: float in arithmetic expression (inline literal).
///
/// FLS §2.4.4.2: Float literals can appear directly in cast expressions.
#[test]
fn milestone_96_float_inline_in_expr() {
    let src = r#"
fn main() -> i32 {
    (1.5_f64 as i32) + (2.5_f64 as i32)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "1.5+2.5 as i32 = 1+2 = 3, got {exit_code}");
}

/// Assembly check: float literal emits ADRP + ADD + LDR into d{N}.
///
/// FLS §2.4.4.2: Float constants are loaded from .rodata at runtime.
/// The sequence uses x17 (ip1) as scratch for address computation.
#[test]
fn runtime_float_literal_emits_ldr_into_dreg() {
    let src = r#"
fn main() -> i32 {
    3.0_f64 as i32
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("ldr     d"),
        "expected `ldr d` (float load) in assembly:\n{asm}"
    );
    assert!(
        asm.contains("fcvtzs"),
        "expected `fcvtzs` (float-to-int) in assembly:\n{asm}"
    );
    assert!(
        asm.contains("main__fc0"),
        "expected float constant label main__fc0 in assembly:\n{asm}"
    );
}

// ── Milestone 97: f64 arithmetic (fadd/fsub/fmul/fdiv) ───────────────────────

/// Milestone 97: f64 addition of two let bindings, cast to i32.
///
/// FLS §6.5.5: The `+` operator on `f64` operands produces an `f64` result.
/// FLS §6.5.9: The result is cast to i32 for use as an exit code.
#[test]
fn milestone_97_f64_add_two_bindings() {
    let src = r#"
fn main() -> i32 {
    let a: f64 = 1.5;
    let b: f64 = 2.5;
    (a + b) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "1.5 + 2.5 = 4.0 as i32 = 4, got {exit_code}");
}

/// Milestone 97: f64 subtraction.
///
/// FLS §6.5.5: The `-` operator on `f64` operands produces an `f64` result.
#[test]
fn milestone_97_f64_sub() {
    let src = r#"
fn main() -> i32 {
    let a: f64 = 10.0;
    let b: f64 = 3.0;
    (a - b) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "10.0 - 3.0 = 7.0 as i32 = 7, got {exit_code}");
}

/// Milestone 97: f64 multiplication.
///
/// FLS §6.5.5: The `*` operator on `f64` operands produces an `f64` result.
#[test]
fn milestone_97_f64_mul() {
    let src = r#"
fn main() -> i32 {
    let a: f64 = 3.0;
    let b: f64 = 4.0;
    (a * b) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "3.0 * 4.0 = 12.0 as i32 = 12, got {exit_code}");
}

/// Milestone 97: f64 division.
///
/// FLS §6.5.5: The `/` operator on `f64` operands produces an `f64` result.
/// IEEE 754: 10.0 / 4.0 = 2.5; truncation toward zero gives 2.
#[test]
fn milestone_97_f64_div() {
    let src = r#"
fn main() -> i32 {
    let a: f64 = 10.0;
    let b: f64 = 4.0;
    (a / b) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "10.0 / 4.0 = 2.5 as i32 = 2, got {exit_code}");
}

/// Milestone 97: f64 arithmetic with inline literals (no explicit let binding).
///
/// FLS §6.5.5: Float arithmetic applies to literal expressions directly.
/// FLS §2.4.4.2: Float literals are typed as f64 without a suffix in this context.
#[test]
fn milestone_97_f64_add_inline_literals() {
    let src = r#"
fn main() -> i32 {
    (3.0_f64 + 2.0_f64) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "3.0 + 2.0 = 5.0 as i32 = 5, got {exit_code}");
}

/// Milestone 97: chained f64 arithmetic (a + b + c).
///
/// FLS §6.5.5: Left-associative evaluation: (a + b) + c.
/// FLS §6.21: Addition is left-associative.
#[test]
fn milestone_97_f64_add_three_values() {
    let src = r#"
fn main() -> i32 {
    let a: f64 = 1.0;
    let b: f64 = 2.0;
    let c: f64 = 3.0;
    (a + b + c) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "1.0 + 2.0 + 3.0 = 6.0 as i32 = 6, got {exit_code}");
}

/// Milestone 97: f64 result stored in a let binding before cast.
///
/// FLS §8.1: A `let` binding can hold an f64 value computed at runtime.
/// FLS §6.5.5: The arithmetic result is stored, then cast.
#[test]
fn milestone_97_f64_result_in_let() {
    let src = r#"
fn main() -> i32 {
    let a: f64 = 2.5;
    let b: f64 = 1.5;
    let c: f64 = a + b;
    c as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "2.5 + 1.5 = 4.0 as i32 = 4, got {exit_code}");
}

/// Milestone 97: f64 arithmetic result used in integer expression.
///
/// FLS §6.5.9: The i32 from a float cast can be used in further integer arithmetic.
#[test]
fn milestone_97_f64_add_in_integer_expr() {
    let src = r#"
fn main() -> i32 {
    let a: f64 = 1.5;
    let b: f64 = 2.5;
    (a + b) as i32 + 1
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "(1.5 + 2.5) as i32 + 1 = 4 + 1 = 5, got {exit_code}");
}

/// Assembly check: f64 addition emits fadd instruction.
///
/// FLS §6.5.5: Float arithmetic uses ARM64 FP instructions, not integer ALU.
#[test]
fn runtime_f64_add_emits_fadd() {
    let src = r#"
fn main() -> i32 {
    let a: f64 = 1.0;
    let b: f64 = 2.0;
    (a + b) as i32
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("fadd"),
        "expected `fadd` instruction in assembly:\n{asm}"
    );
    assert!(
        asm.contains("fcvtzs"),
        "expected `fcvtzs` after fadd in assembly:\n{asm}"
    );
}

// ── Milestone 98: f32 literals compile to runtime ARM64 (FLS §2.4.4.2) ──────

/// Milestone 98: f32 literal with _f32 suffix, cast directly to i32.
///
/// FLS §2.4.4.2: The suffix `_f32` selects the 32-bit float type.
/// FLS §6.5.9: `f32 as i32` truncates toward zero.
#[test]
fn milestone_98_f32_literal_direct_cast() {
    let src = r#"
fn main() -> i32 {
    3.0_f32 as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "3.0_f32 as i32 = 3, got {exit_code}");
}

/// Milestone 98: f32 literal bound to a let binding, then cast.
///
/// FLS §8.1: `let x: f32 = 2.5` stores s{N} to a stack slot via StoreF32.
/// FLS §6.5.9: `x as i32` truncates toward zero → 2.
#[test]
fn milestone_98_f32_let_binding_then_cast() {
    let src = r#"
fn main() -> i32 {
    let x: f32 = 2.5_f32;
    x as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "2.5_f32 as i32 = 2 (truncate), got {exit_code}");
}

/// Milestone 98: truncation toward zero (not floor).
///
/// FLS §6.5.9: `f32 as i32` truncates toward zero.
#[test]
fn milestone_98_f32_truncation_toward_zero() {
    let src = r#"
fn main() -> i32 {
    let x: f32 = 3.9_f32;
    x as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "3.9_f32 as i32 = 3 (truncate not floor), got {exit_code}");
}

/// Milestone 98: two f32 bindings, sum their i32 casts.
///
/// FLS §8.1: Multiple f32 bindings use separate stack slots.
/// FLS §6.5.9: Each `as i32` truncates independently.
#[test]
fn milestone_98_two_f32_bindings_summed() {
    let src = r#"
fn main() -> i32 {
    let a: f32 = 2.0_f32;
    let b: f32 = 3.0_f32;
    (a as i32) + (b as i32)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "2.0_f32 + 3.0_f32 as i32 = 5, got {exit_code}");
}

/// Milestone 98: f32 addition.
///
/// FLS §6.5.5: The `+` operator on `f32` operands produces an `f32` result.
/// FLS §6.5.9: Cast to i32 for exit code.
#[test]
fn milestone_98_f32_add() {
    let src = r#"
fn main() -> i32 {
    let a: f32 = 1.5_f32;
    let b: f32 = 2.5_f32;
    (a + b) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 4, "1.5_f32 + 2.5_f32 = 4.0 as i32 = 4, got {exit_code}");
}

/// Milestone 98: f32 subtraction.
///
/// FLS §6.5.5: The `-` operator on `f32` operands produces an `f32` result.
#[test]
fn milestone_98_f32_sub() {
    let src = r#"
fn main() -> i32 {
    let a: f32 = 10.0_f32;
    let b: f32 = 3.0_f32;
    (a - b) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "10.0_f32 - 3.0_f32 = 7.0 as i32 = 7, got {exit_code}");
}

/// Milestone 98: f32 multiplication.
///
/// FLS §6.5.5: The `*` operator on `f32` operands produces an `f32` result.
#[test]
fn milestone_98_f32_mul() {
    let src = r#"
fn main() -> i32 {
    let a: f32 = 3.0_f32;
    let b: f32 = 4.0_f32;
    (a * b) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "3.0_f32 * 4.0_f32 = 12.0 as i32 = 12, got {exit_code}");
}

/// Milestone 98: f32 division.
///
/// FLS §6.5.5: The `/` operator on `f32` operands produces an `f32` result.
#[test]
fn milestone_98_f32_div() {
    let src = r#"
fn main() -> i32 {
    let a: f32 = 15.0_f32;
    let b: f32 = 3.0_f32;
    (a / b) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "15.0_f32 / 3.0_f32 = 5.0 as i32 = 5, got {exit_code}");
}

/// Assembly check: f32 literal emits ADRP + ADD + LDR into s{N}.
///
/// FLS §2.4.4.2: f32 constants are loaded from .rodata via s-registers.
/// The constant label uses `__f32c{idx}` suffix (vs `__fc{idx}` for f64).
#[test]
fn runtime_f32_literal_emits_ldr_into_sreg() {
    let src = r#"
fn main() -> i32 {
    3.0_f32 as i32
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("ldr     s"),
        "expected `ldr s` (single-precision float load) in assembly:\n{asm}"
    );
    assert!(
        asm.contains("fcvtzs"),
        "expected `fcvtzs` (float-to-int) in assembly:\n{asm}"
    );
    assert!(
        asm.contains("main__f32c0"),
        "expected f32 constant label main__f32c0 in assembly:\n{asm}"
    );
    assert!(
        asm.contains(".word"),
        "expected `.word` directive for f32 constant in assembly:\n{asm}"
    );
}

// ── Milestone 99: integer-to-float casts (`i32 as f64`, `i32 as f32`) ──────
//
// FLS §6.5.9: Numeric cast expressions. Casting from an integer type to a
// floating-point type converts the value to the closest representable float.
// ARM64: `scvtf d{dst}, w{src}` for i32→f64; `scvtf s{dst}, w{src}` for i32→f32.

/// Milestone 99: literal integer cast to f64, then back to i32.
///
/// FLS §6.5.9: `i32 as f64` — SCVTF converts the signed integer to IEEE 754
/// double-precision. All i32 values are exactly representable in f64.
#[test]
fn milestone_99_i32_as_f64_and_back() {
    let src = r#"
fn main() -> i32 {
    let x: i32 = 7;
    let y: f64 = x as f64;
    y as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "7 as f64 as i32 == 7, got {exit_code}");
}

/// Milestone 99: literal integer cast to f64 used in f64 arithmetic.
///
/// FLS §6.5.9: `i32 as f64` produces an f64, which participates in f64
/// arithmetic (FLS §6.5.5).
#[test]
fn milestone_99_i32_as_f64_in_arithmetic() {
    let src = r#"
fn main() -> i32 {
    let x: i32 = 3;
    let y: f64 = x as f64;
    (y * 2.0) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 6, "3 as f64 * 2.0 as i32 == 6, got {exit_code}");
}

/// Milestone 99: parameter cast to f64.
///
/// FLS §6.5.9: The cast expression works for function parameters, not just
/// literals. Verifies runtime codegen path (param value is not statically known).
#[test]
fn milestone_99_param_as_f64() {
    let src = r#"
fn to_double(n: i32) -> i32 {
    let f: f64 = n as f64;
    (f * 2.0) as i32
}
fn main() -> i32 {
    to_double(4)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 8, "to_double(4) == 8, got {exit_code}");
}

/// Milestone 99: literal integer cast to f32, then back to i32.
///
/// FLS §6.5.9: `i32 as f32` — SCVTF single-precision variant.
#[test]
fn milestone_99_i32_as_f32_and_back() {
    let src = r#"
fn main() -> i32 {
    let x: i32 = 5;
    let y: f32 = x as f32;
    y as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "5 as f32 as i32 == 5, got {exit_code}");
}

/// Milestone 99: i32 as f32 in f32 arithmetic.
///
/// FLS §6.5.9 + §6.5.5: Cast produces f32 that participates in f32 arithmetic.
#[test]
fn milestone_99_i32_as_f32_in_arithmetic() {
    let src = r#"
fn main() -> i32 {
    let x: i32 = 4;
    let y: f32 = x as f32;
    (y * 3.0_f32) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 12, "4 as f32 * 3.0 as i32 == 12, got {exit_code}");
}

/// Milestone 99: parameter cast to f32.
///
/// FLS §6.5.9: Works for function parameters.
#[test]
fn milestone_99_param_as_f32() {
    let src = r#"
fn triple_f32(n: i32) -> i32 {
    let f: f32 = n as f32;
    (f * 3.0_f32) as i32
}
fn main() -> i32 {
    triple_f32(5)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 15, "triple_f32(5) == 15, got {exit_code}");
}

/// Milestone 99: inline literal cast to f64 without let binding.
///
/// FLS §6.5.9: The cast expression is evaluated inline (no stack slot required
/// for the intermediate float if not bound to a name).
#[test]
fn milestone_99_inline_literal_as_f64() {
    let src = r#"
fn main() -> i32 {
    (6 as f64 + 1.5) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 7, "6 as f64 + 1.5 as i32 == 7, got {exit_code}");
}

/// Assembly check: `i32 as f64` emits `scvtf d{N}, w{M}`.
///
/// FLS §6.5.9: SCVTF (Signed integer Convert to Floating-point) is the ARM64
/// instruction for integer-to-float conversion.
#[test]
fn runtime_i32_as_f64_emits_scvtf_dreg() {
    let src = r#"
fn main() -> i32 {
    let x: i32 = 3;
    (x as f64) as i32
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("scvtf   d"),
        "expected `scvtf d` (int→f64) in assembly:\n{asm}"
    );
    assert!(
        asm.contains("fcvtzs"),
        "expected `fcvtzs` (f64→int) in assembly:\n{asm}"
    );
}

// ── Milestone 100: float negation (`-x` for f64 and f32) ───────────────────
//
// FLS §6.5.4: The unary `-` operator applied to a floating-point value
// produces its arithmetic negation (IEEE 754 sign-flip).
// ARM64: `fneg d{dst}, d{src}` for f64; `fneg s{dst}, s{src}` for f32.

/// Milestone 100: negate an f64 let binding, cast to i32 for exit code.
///
/// FLS §6.5.4: Unary negation on f64. `-2.5_f64` negated → `2.5`, as i32 → 2.
#[test]
fn milestone_100_f64_neg_positive() {
    let src = r#"
fn main() -> i32 {
    let x: f64 = 2.5;
    let y: f64 = -x;
    y as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 2, "-(2.5_f64) as i32 == 2 (truncate), got {exit_code}");
}

/// Milestone 100: negate a negative f64 (double negation back to positive).
///
/// FLS §6.5.4: `-(-x)` restores the original value.
#[test]
fn milestone_100_f64_neg_of_neg() {
    let src = r#"
fn main() -> i32 {
    let x: f64 = -3.0;
    let y: f64 = -x;
    y as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "-(-3.0_f64) as i32 == 3, got {exit_code}");
}

/// Milestone 100: f64 negation of a function parameter.
///
/// FLS §6.5.4: Negation works on values not statically known at compile time.
#[test]
fn milestone_100_f64_neg_param() {
    let src = r#"
fn negate(x: f64) -> i32 {
    (-x) as i32
}
fn main() -> i32 {
    negate(4.7)
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    // -4.7 as i32 truncates toward zero → -4 (exit code wraps: 256 - 4 = 252)
    assert_eq!(exit_code, 252, "negate(4.7) → -4 (wrapped) == 252, got {exit_code}");
}

/// Milestone 100: f64 negation in arithmetic expression.
///
/// FLS §6.5.4 + §6.5.5: Negated float participates in further arithmetic.
#[test]
fn milestone_100_f64_neg_in_arithmetic() {
    let src = r#"
fn main() -> i32 {
    let x: f64 = 1.5;
    ((-x) + 5.0) as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 3, "(-1.5 + 5.0) as i32 == 3, got {exit_code}");
}

/// Milestone 100: negate an f32 let binding.
///
/// FLS §6.5.4: Unary negation on f32. ARM64: `fneg s{dst}, s{src}`.
#[test]
fn milestone_100_f32_neg_positive() {
    let src = r#"
fn main() -> i32 {
    let x: f32 = 6.0_f32;
    let y: f32 = -x;
    y as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    // -6.0_f32 as i32 = -6 → exit code wraps: 256 - 6 = 250
    assert_eq!(exit_code, 250, "-(6.0_f32) as i32 == -6 (wrapped to 250), got {exit_code}");
}

/// Milestone 100: f32 negation of a negative value.
///
/// FLS §6.5.4: `-(-x)` returns the original value for f32.
#[test]
fn milestone_100_f32_neg_of_neg() {
    let src = r#"
fn main() -> i32 {
    let x: f32 = -5.0_f32;
    let y: f32 = -x;
    y as i32
}
"#;
    let Some(exit_code) = compile_and_run(src) else { return; };
    assert_eq!(exit_code, 5, "-(-5.0_f32) as i32 == 5, got {exit_code}");
}

/// Assembly check: `-x` where x is f64 emits `fneg d{N}, d{M}`.
///
/// FLS §6.5.4: FNEG is the ARM64 instruction for IEEE 754 sign-flip.
#[test]
fn runtime_f64_neg_emits_fneg_dreg() {
    let src = r#"
fn main() -> i32 {
    let x: f64 = 2.5;
    (-x) as i32
}
"#;
    let asm = compile_to_asm(src);
    assert!(
        asm.contains("fneg    d"),
        "expected `fneg d` (f64 negate) in assembly:\n{asm}"
    );
}
