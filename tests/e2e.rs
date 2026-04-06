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
