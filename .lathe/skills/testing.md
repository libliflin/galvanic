# Testing

*Why this exists: galvanic has three distinct test layers with different purposes and different tooling requirements. Mixing them up is the most common contributor mistake. This explains what each layer is for and how to add tests to each.*

---

## The Three Layers

### Layer 1: `tests/smoke.rs` — CLI Smoke Tests

Tests the binary as a black box. Uses `std::process::Command` to run `galvanic` binary, checks exit codes and stdout text.

**When to add here:** When testing CLI behavior — usage errors, file-not-found, the "galvanic: compiling" output, basic CLI flag parsing.

**Pattern:**
```rust
#[test]
fn empty_file_exits_zero() {
    let empty = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(empty.path())
        .output()
        .expect("failed to run galvanic");
    assert!(output.status.success());
}
```

### Layer 2: `tests/fls_fixtures.rs` — Parse Acceptance Tests

Tests that galvanic can lex and parse a fixture file. No lowering, no codegen. A passing test here means: "galvanic correctly accepts this FLS construct at the parse level."

**When to add here:** When adding a new `.rs` fixture file in `tests/fixtures/` for a FLS construct that requires only parse coverage. Useful for FLS features that the codegen doesn't yet support.

**Pattern:** Add a `fls_X_Y_description.rs` file to `tests/fixtures/`, then add:
```rust
#[test]
fn fls_X_Y_description() {
    assert_galvanic_accepts("fls_X_Y_description.rs");
}
```

**Important:** A passing `fls_fixtures` test does NOT mean the feature compiles. It only means it parses. Do not cite a `fls_fixtures` test as evidence that a FLS feature is implemented.

### Layer 3: `tests/e2e.rs` — Full Pipeline Tests

The main test suite. Over 1700 tests. Two sub-patterns within this file:

#### 3a. Assembly Inspection (no external tools required)

Uses `compile_to_asm(source)` which runs lex → parse → lower → codegen and returns the GAS text. Works on macOS and Linux. Does NOT require ARM64 cross tools or QEMU.

**Purpose:** Verify that galvanic emits the correct ARM64 instruction for a given construct. Critical for proving FLS §6.1.2:37–45 compliance (no const-folding of runtime code).

**When to add:** For every new arithmetic, comparison, bitwise, or branch-emitting feature. Always add alongside an exit-code test. An exit-code test alone cannot distinguish "compiled correctly" from "constant-folded."

**Pattern:**
```rust
#[test]
fn runtime_FEATURE_emits_INSTRUCTION() {
    let asm = compile_to_asm("fn main() -> i32 { EXPR }\n");
    assert!(
        asm.contains("INSTRUCTION"),
        "expected `INSTRUCTION` in assembly, got:\n{asm}"
    );
    // For arithmetic: also assert the constant-folded result is absent
    assert!(
        !asm.contains("mov     x0, #RESULT"),
        "must not const-fold at compile time:\n{asm}"
    );
}
```

#### 3b. Compile-and-Run (requires ARM64 cross tools + QEMU)

Uses `compile_and_run(source)` which assembles, links, and runs the binary. Requires `aarch64-linux-gnu-as`, `aarch64-linux-gnu-ld`, and either native ARM64 or `qemu-aarch64`. Tests self-skip (return early) when tools are absent.

**When to add:** For verifying correct runtime behavior — the right exit code, correct iteration counts, correct arithmetic results.

**Pattern:**
```rust
/// Milestone N: description.
///
/// FLS §X.Y: the relevant spec section.
#[test]
fn milestone_N_description() {
    let Some(exit_code) = compile_and_run("fn main() -> i32 { EXPR }\n") else {
        return; // tools not available — test self-skips
    };
    assert_eq!(exit_code, EXPECTED, "expected {EXPECTED}, got {exit_code}");
}
```

---

## Milestone Numbering and Section Headers

Each new milestone group in `e2e.rs` gets a section comment:
```rust
// ── Milestone N: description ─────────────────────────────────────────────────
//
// FLS §X.Y: relevant context
// FLS §6.1.2:37–45: always cite if runtime instruction emission is involved.
```

Use the next sequential milestone number. Do not reuse milestone numbers. If you add several related tests under one milestone header, that's fine — one header, multiple tests.

---

## FLS Citations in Tests

Every e2e test doc comment must cite the relevant FLS section(s):

```rust
/// Milestone N: short description.
///
/// FLS §X.Y: what this tests.
/// FLS §6.1.2:37–45: if this test verifies runtime instruction emission.
#[test]
fn milestone_N_name() { ... }
```

---

## Fixture File Conventions

- `tests/fixtures/fls_X_Y_description.rs` — source programs drawn from FLS §X.Y examples
- `tests/fixtures/fls_X_Y_description.s` — expected ARM64 assembly output (present for some milestones)
- Fixture programs should be minimal: just enough to exercise the construct, no more.

---

## What "New Milestone" Checklist Looks Like

When adding a new FLS milestone to `e2e.rs`:

1. Add a `// ── Milestone N: description` section header with FLS citation
2. Add at least one `compile_and_run` test with the correct expected exit code
3. If the feature involves a new runtime instruction (arithmetic, comparison, branch, memory access): add a `compile_to_asm` assembly inspection test that:
   - Asserts the new instruction appears in the assembly
   - Asserts the result is NOT constant-folded (for arithmetic)
4. Cite `FLS §X.Y` in every test's doc comment
5. If the feature requires a new fixture file, add it to `tests/fixtures/` with the right naming

---

## Common Mistakes

- **Only adding exit-code tests for arithmetic.** Always add assembly inspection too. The const-fold check is non-optional.
- **Adding to `fls_fixtures.rs` and claiming the feature is implemented.** Parse acceptance is not implementation.
- **Using `compile_and_run` for assembly inspection.** Use `compile_to_asm` instead — it's simpler, faster, and works on macOS.
- **Not self-guarding `compile_and_run` tests.** Always use `let Some(exit_code) = compile_and_run(...) else { return; }`.
