# Testing — How Galvanic Tests Itself

This file exists to prevent a specific mistake: writing a milestone test that
checks only the exit code. An interpreter and a compiler can both produce exit
code 42 for `fn main() -> i32 { 42 }`. Assembly inspection is what separates them.

---

## Test Suite Structure

```
tests/
  smoke.rs          — binary behavior (runs galvanic as a subprocess)
  fls_fixtures.rs   — parse-only acceptance tests for FLS programs
  e2e.rs            — full pipeline tests (assembly inspection + compile-and-run)
  fixtures/
    fls_*.rs        — Rust programs derived from FLS examples
    fls_*.s         — expected ARM64 assembly output (golden files)
    milestone_1.rs  — first milestone program
```

---

## Three Kinds of Tests

### 1. Parse acceptance (`tests/fls_fixtures.rs`)

```rust
fn assert_galvanic_accepts(fixture: &str) { ... }

#[test]
fn fls_10_3_assoc_consts() {
    assert_galvanic_accepts("fls_10_3_assoc_consts.rs");
}
```

These verify galvanic can lex and parse a fixture file without error. They do
**not** verify codegen. A feature can appear here long before it appears in e2e.
When you add a new fixture (e.g., `fls_12_generics.rs`), add a test here first.

### 2. Assembly inspection (`compile_to_asm` in `tests/e2e.rs`)

```rust
fn compile_to_asm(source: &str) -> String { ... }

#[test]
fn runtime_add_emits_add_instruction() {
    let asm = compile_to_asm("fn main() -> i32 { 1 + 2 }\n");
    assert!(asm.contains("add"), "expected add instruction...");
    assert!(!asm.contains("mov     x0, #3"), "must not fold to constant...");
}
```

`compile_to_asm` runs lex → parse → lower → codegen and returns the assembly
string. It does **not** assemble or run the result. It works on macOS without
cross tools. This is the **primary correctness check** for new codegen features.

Every new operator, control flow construct, or IR instruction type needs:
- A positive assertion: the expected instruction is present in the assembly.
- A negative assertion: the constant is NOT folded into a `mov #N`.

### 3. Compile and run (`compile_and_run` in `tests/e2e.rs`)

```rust
fn compile_and_run(source: &str) -> Option<i32> { ... }

#[test]
fn milestone_1_main_returns_zero() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { 0 }\n") else {
        return; // skipped if cross tools unavailable
    };
    assert_eq!(exit_code, 0);
}
```

Runs the full pipeline including assembling, linking, and executing the ARM64
binary (via qemu-aarch64 on non-ARM64 hosts). Skips automatically when cross
tools are not installed. Always runs on CI (ubuntu-latest installs them).

This is the end-to-end truth check but it cannot distinguish "correct runtime
behavior" from "correct constant folded at compile time" by itself — that is
why assembly inspection is also required.

---

## Writing a Complete Milestone Test

A well-covered milestone has ALL THREE test layers:

```rust
// Layer 1: parse acceptance (in fls_fixtures.rs)
#[test]
fn fls_15_closures() {
    assert_galvanic_accepts("fls_15_closures.rs");
}

// Layer 2: assembly inspection (in e2e.rs) — REQUIRED
// Tests that the feature emits the right instruction AND doesn't constant-fold
#[test]
fn milestone_N_closure_captures_emit_load() {
    let asm = compile_to_asm("fn main() -> i32 { let x = 7; let f = || x; f() }\n");
    assert!(asm.contains("ldr"), "closure must emit a load from capture slot");
    assert!(!asm.contains("mov     x0, #7"), "must not constant-fold captured value");
}

// Layer 3: compile and run (in e2e.rs)
#[test]
fn milestone_N_closure_captures_correct_value() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { let x = 7; let f = || x; f() }\n") else {
        return;
    };
    assert_eq!(exit_code, 7);
}
```

If a milestone only has Layer 3, it is incompletely tested.

---

## Fixture Files (`tests/fixtures/`)

Fixture `.rs` files are Rust programs derived from FLS examples. Not invented —
taken from the spec. When the FLS provides an example, use it verbatim or
minimally adapted. If the spec provides no example, note that in the file.

Some fixtures have a corresponding `.s` file (expected ARM64 assembly). These
are golden files. If `emit_asm` output for a fixture changes, update the `.s`
file and explain the change in the changelog.

---

## Test Naming Convention

Follow the pattern already in `tests/e2e.rs`:

- `milestone_{N}_{description}` — numbered milestone tests
- `runtime_{op}_emits_{instruction}_instruction` — assembly inspection tests
- `fls_{section}_{description}` — FLS-derived acceptance tests

---

## What Makes a Test Adversarial

The easiest test is always: `fn main() -> i32 { 42 }`. This tests almost nothing.
A better test uses:

- **Parameters**: `fn f(x: i32) -> i32 { ... }` — prevents constant folding by making the input unknown at compile time.
- **Multiple operations**: chains of operations that would produce the wrong answer if any step was skipped.
- **Boundary values**: 0, negative numbers, i32::MAX, for types that have them.
- **Interaction between features**: a closure that captures a variable that was computed by a function call.

When writing a test for a new feature, ask: "would this test catch the bug where galvanic constant-folds instead of generating runtime code?" If not, add a negative assertion.
