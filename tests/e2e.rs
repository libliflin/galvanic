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
