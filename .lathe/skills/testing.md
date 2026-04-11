# Testing in Galvanic

## Test runner
`cargo test` — standard Rust test harness. No external test runner.

## Test files

### `tests/e2e.rs` — Full pipeline, end-to-end
Runs the complete lex → parse → lower → codegen → assemble → link → run pipeline and checks exit codes or assembly output. Tests are gated on tool availability (ARM64 cross-toolchain + qemu-aarch64); they skip gracefully on macOS or any machine missing the tools. CI installs the tools explicitly.

Two helper functions drive all tests:
- `compile_and_run(source)` — full pipeline, returns `Option<i32>` exit code (None = tools not available, test skips)
- `compile_to_asm(source)` — pipeline through codegen only, returns assembly text string. Use this for assembly inspection tests.

**Assembly inspection tests are mandatory for every new e2e feature.** Exit code tests alone cannot prove the project is emitting runtime instructions rather than constant-folding. The pattern:

```rust
let asm = compile_to_asm("fn main() -> i32 { 1 + 2 }\n");
assert!(asm.contains("add"), "expected add instruction");
assert!(!asm.contains("mov     x0, #3"), "must not constant-fold");
```

### `tests/fls_fixtures.rs` — Parse acceptance only
Verifies that galvanic can lex and parse FLS example programs. Uses `assert_galvanic_accepts(fixture_name)`. Does NOT test lowering or codegen. A fixture test passing means the parser accepts the program — it says nothing about whether the compiler can produce correct code.

Fixtures live in `tests/fixtures/*.rs`. Each fixture should be a real Rust program derived from a specific FLS section example. Do not invent programs — derive them from the spec.

### `tests/smoke.rs` — Basic smoke tests
Minimal sanity checks (exact contents unknown, but separate from e2e).

### Unit tests in `src/lexer.rs`
Size-enforcement tests: `token_is_eight_bytes`, `span_is_eight_bytes`. These assert the cache-line layout invariant. If they fail after a change, the cache-line constraint has been violated and the change needs to be reconsidered, not the test.

## Claims methodology

The project's commits follow a pattern: `Claim 4k: add while-let runtime falsification for FLS §6.15.4`. Each "claim" covers one FLS section and has three required parts:

1. A parse fixture in `tests/fixtures/fls_X_Y_name.rs` (if one doesn't exist)
2. An e2e exit-code test in `tests/e2e.rs` verifying correct runtime behavior
3. An assembly inspection test in `tests/e2e.rs` verifying runtime instruction emission (e.g., `cbz` for a branch, `add` for addition)

A claim without the assembly inspection test is incomplete. The core constraint (`refs/fls-constraints.md`) requires runtime codegen, and only assembly inspection proves it.

## Running specific tests

```bash
cargo test                          # all tests
cargo test --test e2e               # e2e only
cargo test --test fls_fixtures      # parse fixtures only
cargo test --lib                    # unit tests only (including lexer size tests)
cargo test --lib -- --exact lexer::tests::token_is_eight_bytes  # one test
```

## What CI runs

1. `cargo build` — must pass
2. `cargo test` — all tests must pass
3. `cargo clippy -- -D warnings` — no warnings
4. fuzz-smoke job: adversarial inputs (garbage, NUL bytes, 10k let statements, 500-deep nesting) must exit cleanly (no signal, no hang)
5. e2e job (ubuntu-latest + arm64 cross-tools + qemu): `cargo test --test e2e`
6. audit job: no `unsafe` in `src/`, no `std::process::Command` outside `main.rs`, no network crate dependencies
7. bench job: criterion benchmarks run with throughput reporting

The e2e job gate (`needs: build`) means a build failure blocks e2e from running.
