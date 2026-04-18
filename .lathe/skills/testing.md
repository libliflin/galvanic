# Testing Conventions — Galvanic

How this project tests. For the builder when adding or modifying tests.

---

## Test Tiers

### 1. Inline unit tests (`src/*.rs`)
Small tests embedded directly in source modules under `#[cfg(test)]`. Used for data structure invariants — token size, span size — and for testing lexer/parser internals.

Run with: `cargo test --lib`

Key tests to keep green:
- `lexer::tests::token_is_eight_bytes` — `Token` must be exactly 8 bytes (cache-line constraint)
- `lexer::tests::span_is_eight_bytes` — `Span` must be exactly 8 bytes

These are enforced in the bench CI job: `cargo test --lib -- --exact lexer::tests::token_is_eight_bytes`

### 2. Smoke tests (`tests/smoke.rs`)
Quick sanity checks for the lexer and parser on minimal inputs. Fast to run, broad signal.

Run with: `cargo test --test smoke`

### 3. FLS fixture tests (`tests/fls_fixtures.rs`)
Parse acceptance tests for Rust programs derived from FLS examples. Each test calls `assert_galvanic_accepts(fixture_name)`, which runs lex + parse only — not lowering or codegen.

Run with: `cargo test --test fls_fixtures`

Use fixture tests when: you want to verify galvanic can *parse* a language construct, but lowering/codegen isn't implemented yet.

**Fixture location:** `tests/fixtures/*.rs`. Files are named `fls_N_M_description.rs`.

**Convention:** Fixture source should be derived from FLS examples, not invented. If the spec doesn't provide an example, note that in the fixture comment.

### 4. End-to-end tests (`tests/e2e.rs`)
Full pipeline tests. Two modes:

**Assembly inspection** — compile through galvanic, inspect the emitted ARM64 text:
```rust
fn compile_to_asm(source: &str) -> String {
    // runs lex → parse → lower → codegen, returns assembly text
}

let asm = compile_to_asm("fn main() -> i32 { 1 + 2 }\n");
assert!(asm.contains("add"), "expected add instruction for +");
assert!(!asm.contains("mov     x0, #3"), "must not constant-fold");
```

**Run-and-check** — compile to a binary and execute via qemu, check exit code:
```rust
fn compile_and_run(source: &str) -> i32 {
    // full pipeline including assemble + link + qemu
}

let code = compile_and_run("fn main() -> i32 { 42 }\n");
assert_eq!(code, 42);
```

Run with: `cargo test --test e2e`

**When to use which mode:**
- Assembly inspection: whenever the test involves arithmetic, comparisons, control flow, or any expression where "it evaluated at compile time and emitted a constant" would produce the wrong instruction form.
- Run-and-check: when the runtime exit code is what matters (e.g., testing that a specific program returns a specific value).

Always prefer assembly inspection for arithmetic and control flow. A test that only checks the exit code for `fn main() -> i32 { 1 + 2 }` cannot distinguish "compiled correctly" from "const-folded and emitted mov x0, #3."

**Prerequisites for run-and-check tests:** `aarch64-linux-gnu-as`, `aarch64-linux-gnu-ld`, `qemu-aarch64`. Tests skip gracefully (not fail) if these are absent. On CI the e2e job installs them.

### 5. Benchmarks (`benches/throughput.rs`)
Throughput benchmarks using Criterion. Run in the bench CI job.

Run with: `cargo bench --bench throughput`

---

## FLS Constraint on Tests

The no-const-folding rule (FLS §6.1.2:37–45) must be enforced by tests, not just trusted from code review. For every arithmetic expression, comparison, or control flow path implemented in the lowering/codegen pass:

1. Write an assembly inspection test that verifies the *instruction form* is present.
2. Optionally include a negative assertion that the constant-folded result is NOT present.

This is the "litmus test" from `refs/fls-constraints.md`: Replace every literal in a function with a function parameter. If the implementation breaks, it was const-folding. Assembly inspection is the test-suite form of this check.

---

## Adding a New Feature: Test Checklist

1. **Parse acceptance**: Does the parser accept the construct? Add to a fixture file or smoke test.
2. **Lowering**: Does `lower()` produce correct IR? Test via `compile_to_asm()` — inspect the instructions.
3. **Codegen**: Does `emit_asm()` produce the right instruction? Use assembly inspection, not exit code.
4. **Full pipeline**: Does `compile_and_run()` produce the right exit code? Add if the runtime behavior matters.
5. **FLS citation**: Does every new code path have a `// FLS §X.Y: ...` annotation?
6. **Ambiguity**: If the FLS was silent or ambiguous about anything, add an entry to `refs/fls-ambiguities.md`.

---

## Running Tests Locally

```bash
cargo build          # check compilation
cargo test           # all tests (unit + smoke + fixture + e2e with skip if no tools)
cargo clippy -- -D warnings   # lint, must be clean
cargo bench --bench throughput -- --warm-up-time 2 --measurement-time 3  # benchmarks
```

E2E tests that need the cross toolchain will skip cleanly without it. To run e2e tests locally on macOS, install `aarch64-linux-gnu-as`, `aarch64-linux-gnu-ld`, and `qemu-aarch64` (available via Homebrew as `aarch64-elf-binutils` and `qemu`).
