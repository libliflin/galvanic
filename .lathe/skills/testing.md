# Testing — Galvanic

How this project tests. Read this before writing any test code.

---

## Test runner

`cargo test` — runs all test suites. No test framework beyond `std` (plus `criterion` for benchmarks and `tempfile` for smoke tests).

## Test suites

### Unit tests (`src/**/*.rs` — `#[cfg(test)]` modules)
Inline module-level unit tests. Key invariant tests here:
- `lexer::tests::token_is_eight_bytes` — asserts `Token` is 8 bytes (1 per cache line slot)
- `lexer::tests::span_is_eight_bytes` — asserts `Span` is 8 bytes
- IR type size assertions (check `src/ir.rs` for `assert_eq!(size_of::<T>(), N)` tests)

Run: `cargo test --lib`

### Smoke tests (`tests/smoke.rs`)
CLI-level integration tests: run the galvanic binary on real inputs, check exit codes and error output. Uses `tempfile` for temp `.rs` files and `Command::new(env!("CARGO_BIN_EXE_galvanic"))`. Tests cover:
- Empty file → exit 0
- Missing file → non-zero with clean error
- Lower error → error message names the failing function
- Lower error → summary line shows N/M functions succeeded
- Partial lowering → assembly emitted for successful functions

Run: `cargo test --test smoke`

### FLS fixture tests (`tests/fls_fixtures.rs`)
Parse-acceptance tests: every file in `tests/fixtures/fls_*.rs` is run through the lexer and parser. Verifies galvanic can at least lex and parse each FLS example. Does NOT test lowering or codegen.

Convention: fixture filename is `fls_<section>_<topic>.rs`. Example: `fls_6_5_3_nan.rs`.

Run: `cargo test --test fls_fixtures`

### Assembly inspection tests (`tests/e2e.rs` — `compile_to_asm`)
The most important tests for compiler correctness. These verify that galvanic emits the right *instructions*, not just the right exit code.

Pattern:
```rust
let asm = compile_to_asm("fn main() -> i32 { 1 + 2 }\n");
assert!(asm.contains("add"), "expected add instruction");
assert!(!asm.contains("mov     x0, #3"), "must not constant-fold");
```

These tests enforce FLS §6.1.2 Constraint 1: non-const code must emit runtime instructions. An exit-code test cannot distinguish correct codegen from compile-time folding. Assembly inspection closes that gap.

Helper: `compile_to_asm(source: &str) -> String` — runs lex → parse → lower → codegen without assembling.

### End-to-end tests (`tests/e2e.rs` — `compile_and_run`)
Full pipeline: lex → parse → lower → codegen → assemble (`aarch64-linux-gnu-as`) → link (`aarch64-linux-gnu-ld`) → run with `qemu-aarch64`. Checks that the ARM64 binary produces the correct exit code.

**Prerequisites:** `aarch64-linux-gnu-as`, `aarch64-linux-gnu-ld`, `qemu-aarch64`. Tests are **skipped** (not failed) when these tools are absent. CI installs them explicitly on `ubuntu-latest`.

Helper: `tool_available(tool: &str) -> bool` — use to gate e2e tests that need cross tools.

Run: `cargo test --test e2e -- --nocapture`

### Benchmarks (`benches/throughput.rs`)
Criterion benchmarks for pipeline throughput. Run: `cargo bench --bench throughput`

CI job: runs with `--warm-up-time 2 --measurement-time 3`, prints throughput summary, checks cache-line-critical data structure sizes haven't grown.

---

## Adding tests for a new feature

For every new language feature, the complete test chain is:
1. **Fixture:** `tests/fixtures/fls_<section>_<topic>.rs` — a compilable Rust program derived from the FLS section example.
2. **Parse acceptance:** `tests/fls_fixtures.rs` — add `assert_galvanic_accepts("fls_<section>_<topic>.rs")`.
3. **Assembly inspection:** `tests/e2e.rs` using `compile_to_asm` — verify the correct ARM64 instructions are emitted AND that compile-time folding did not happen.
4. **Full e2e (when applicable):** `compile_and_run` test verifying the correct exit code when run under QEMU.

Assembly inspection (step 3) is mandatory for any expression or statement that the compiler could theoretically constant-fold. This is the enforcement mechanism for FLS §6.1.2 Constraint 1.

---

## CI jobs

From `.github/workflows/ci.yml`:

| Job | What it checks | Gate |
|-----|---------------|------|
| `build` | `cargo build` + `cargo test` + `cargo clippy -D warnings` | Always |
| `fuzz-smoke` | Binary handles adversarial inputs without crashing; exit code checks | After `build` |
| `audit` | No `unsafe` in `src/`, no `Command` in library code, no network deps | Always |
| `e2e` | Full pipeline including assemble + link + QEMU run | After `build` |
| `bench` | Throughput benchmarks; cache-line size assertions | After `build` |

No `pull_request_target` or `issue_comment` triggers — CI runs only on `push` to `main` and `pull_request` to `main`. Permissions: `contents: read`.

---

## Invariants enforced by CI

These are checked automatically — do not work around them:
- **No `unsafe` code** — `audit` job fails if any `unsafe { }`, `unsafe fn`, or `unsafe impl` appears in `src/`.
- **No `Command` in library code** — only `src/main.rs` may shell out.
- **No network crates** — checked in `Cargo.toml`.
- **Cache-line type sizes** — `bench` job runs size assertions for `Token` and `Span`.

---

## Common gotchas

- **Skip, don't fail** — when cross tools aren't available locally, e2e tests should return early rather than `assert!(false)`. Pattern: `if !tool_available("aarch64-linux-gnu-as") { return; }`.
- **Assertion message required** — every `assert!` needs a message explaining what was expected and why. The failure message is the only documentation the reader has when CI breaks.
- **No compile-time results in e2e tests** — if your e2e test only checks the exit code, it is not sufficient for features that could be constant-folded. Always pair with an assembly inspection test.
