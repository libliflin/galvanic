# Testing Conventions

How galvanic tests itself. Read this before writing new tests.

---

## Test suites

### `tests/smoke.rs` — CLI black-box tests

Tests the galvanic binary as an external user would. Invokes `galvanic` via `Command::new(env!("CARGO_BIN_EXE_galvanic"))` and asserts on exit code and stdout/stderr content.

**What goes here:** Error message format assertions, exit code contracts, CLI behavior for edge-case inputs (empty file, missing file, no args). These tests encode the Lead Researcher's promise: "the error output is always useful."

**Run with:** `cargo test --test smoke`

---

### `tests/fls_fixtures.rs` — Parse acceptance tests

Calls `galvanic::lexer::tokenize()` and `galvanic::parser::parse()` directly on fixture files. Does not invoke lowering or codegen.

**What goes here:** A test for each FLS fixture file confirming that galvanic parses it without error. When galvanic starts parsing a new FLS section, add the fixture here.

**Convention:** Each test is `fn fls_X_Y_<description>()` calling `assert_galvanic_accepts("fls_X_Y_<description>.rs")`. The fixture file lives at `tests/fixtures/fls_X_Y_<description>.rs`.

**Run with:** `cargo test --test fls_fixtures`

---

### `tests/e2e.rs` — Full-pipeline tests

Two kinds of tests live here:

**Assembly inspection tests** (`compile_to_asm(source)`): Run the full lex → parse → lower → codegen pipeline and return the emitted ARM64 assembly text as a string. Assert on specific instructions (e.g., `assert!(asm.contains("add"), "expected add instruction")`). These work everywhere — no cross-toolchain needed.

Purpose: Verify that runtime instructions are emitted, not compile-time folded constants. This enforces FLS Constraint 1 (`refs/fls-constraints.md`).

**Binary execution tests** (`compile_and_run(source)` or similar): Run the full pipeline including assemble (`aarch64-linux-gnu-as`) and link (`aarch64-linux-gnu-ld`), then execute via `qemu-aarch64`. Assert on exit code.

**Platform requirement:** Binary execution tests only work on Linux (or macOS with both the cross-toolchain and QEMU installed). Tests skip gracefully via `tools_available()` when the toolchain is absent. CI (ubuntu-latest) installs `binutils-aarch64-linux-gnu` and `qemu-user` for the `e2e` job.

**Run with:** `cargo test --test e2e` (parse + assembly inspection always; binary execution on Linux CI)

---

### `src/**` — Unit tests (inline)

Size assertion tests live inline in their modules:

```rust
#[test]
fn token_is_eight_bytes() {
    assert_eq!(std::mem::size_of::<Token>(), 8);
}
```

These enforce cache-line discipline: if a type grows beyond its target size, the test breaks.

**Run with:** `cargo test --lib`

---

## Fixtures

`tests/fixtures/` contains real Rust programs drawn from FLS examples, one file per spec section. Naming convention: `fls_<section>_<topic>.rs`. Some fixtures also have corresponding `.s` output files showing the expected assembly.

When adding support for a new FLS section:
1. Write a fixture in `tests/fixtures/fls_<section>_<topic>.rs`.
2. Add a parse acceptance test in `tests/fls_fixtures.rs`.
3. If codegen works end-to-end, add an assembly inspection test in `tests/e2e.rs`.

---

## CI jobs

| Job | What it checks | When it runs |
|-----|----------------|--------------|
| `build` | `cargo build`, `cargo test`, `cargo clippy -D warnings` | Every push and PR |
| `fuzz-smoke` | Binary robustness on adversarial inputs (empty file, garbage, deeply nested, very long lines) | Every push and PR, needs `build` |
| `audit` | No unsafe blocks, no `Command` in library code, no networking deps | Every push and PR |
| `e2e` | Full-pipeline binary execution tests with ARM64 cross-toolchain + QEMU | Every push and PR, needs `build` |
| `bench` | Throughput benchmarks, data structure size assertions | Every push and PR, needs `build` |

CI is the authoritative source for e2e results. Assembly inspection tests run everywhere; binary execution tests only run on Linux CI.

---

## Writing assembly inspection tests

The key convention for enforcing FLS Constraint 1 (no compile-time evaluation in non-const contexts):

```rust
let asm = compile_to_asm("fn main() -> i32 { 1 + 2 }\n");
assert!(asm.contains("add"), "expected runtime add instruction");
assert!(!asm.contains("mov     x0, #3"), "must not constant-fold");
```

The litmus test: replace every literal with a parameter. The codegen path must be identical.

```rust
// These two must emit structurally identical assembly:
let asm1 = compile_to_asm("fn foo(x: i32, y: i32) -> i32 { x + y }\n");
let asm2 = compile_to_asm("fn foo() -> i32 { 1 + 2 }\n");
// Both must use `add` — asm2 must NOT fold to `mov x0, #3`.
```
