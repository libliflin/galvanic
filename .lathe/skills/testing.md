# Testing — galvanic

This file answers: "How does galvanic test things, what conventions do existing tests follow, and what should a new test look like?" Read this before writing any test.

---

## Test Layers

### Layer 1: Unit tests (inside `src/`)

In-module tests (`#[cfg(test)] mod tests { ... }`) for specific behaviors. Currently:
- `lexer::tests` — tokenization behavior, keyword recognition, layout assertions (`token_is_eight_bytes`)
- Others as needed

**Convention:** Test the public API of the module. Name tests after what they verify, not how they verify it.

### Layer 2: Parse-acceptance tests (`tests/fls_fixtures.rs`)

Each test in this file calls `assert_galvanic_accepts(fixture_name)`, which:
1. Reads `tests/fixtures/{fixture_name}` from disk
2. Runs `galvanic::lexer::tokenize(&source)` — asserts no error
3. Runs `galvanic::parser::parse(&tokens, &source)` — asserts no error

These tests do NOT run lowering or codegen. They verify the lexer and parser accept valid FLS programs. Some fixture programs use features galvanic can parse but not yet lower — that's fine here.

**When to add a new parse-acceptance test:** Every time a new `tests/fixtures/fls_*.rs` file is created, a matching `#[test] fn fls_X_Y_name() { assert_galvanic_accepts("fls_X_Y_name.rs"); }` goes in `tests/fls_fixtures.rs`.

### Layer 3: Full-pipeline tests (`tests/e2e.rs`)

Two helpers drive e2e tests:

**`compile_to_asm(source: &str) -> String`** — runs lex → parse → lower → codegen, returns assembly text. Used to inspect whether specific instructions appear (e.g., `assert!(asm.contains("add"))`). This is the adversarial tool for verifying runtime codegen: if the assembly lacks a runtime instruction that should be there, const-folding has occurred.

**`compile_and_run(source: &str) -> Option<i32>`** — runs the full pipeline including assemble + link + qemu-run, returns the exit code. Returns `None` when the aarch64 cross tools or qemu are not available (tests skip gracefully, not fail). On CI, the `e2e` job installs them explicitly.

**When to add e2e tests:** Every new milestone that changes codegen should have at least one e2e test. Prefer `compile_to_asm` when you want to verify a specific instruction is emitted. Use `compile_and_run` when you want to verify runtime behavior (exit code, side effects).

**Skipping gracefully:** `compile_and_run` returns `None` if tools are absent. Write tests as:
```rust
if let Some(exit_code) = compile_and_run(source) {
    assert_eq!(exit_code, 42);
}
```
Not as `unwrap()` — that would fail on macOS where cross tools aren't installed.

### Layer 4: Smoke test (`tests/smoke.rs`)

One test: `empty_file_exits_zero`. Verifies the CLI binary (not the library) accepts an empty `.rs` file without error. Uses `env!("CARGO_BIN_EXE_galvanic")` to locate the binary. This test runs in CI's standard `cargo test`.

### Layer 5: Benchmarks (`benches/throughput.rs`)

Criterion benchmarks. CI runs them with short measurement time (`--warm-up-time 2 --measurement-time 3`) just to ensure they don't panic — not to enforce performance budgets. The `bench` CI job also runs the `token_is_eight_bytes` size check.

---

## Fixture File Conventions (`tests/fixtures/`)

Each fixture file:
- Is named `fls_{section}_{brief_description}.rs`
- Contains a real Rust program derived from FLS examples (not invented)
- Has a comment at the top identifying the FLS section
- Does NOT need a `fn main()` unless it's testing a runnable program
- May contain features galvanic can't yet lower — the parse-acceptance test still passes

**Example fixture naming:**
- `fls_6_expressions.rs` → FLS §6 expression forms
- `fls_6_15_1_for_loop.rs` → FLS §6.15.1 specifically
- `fls_4_9_slices.rs` → FLS §4.9 slice types

When the spec doesn't provide an example, note that in the fixture comment:
```rust
// FLS §X.Y has no explicit example. This program is derived from the
// normative text at §X.Y:N.
```

---

## What "Milestone N" Means in Tests

Each milestone commit adds:
1. A fixture file in `tests/fixtures/` with a program exercising the new feature
2. A parse-acceptance test in `tests/fls_fixtures.rs`
3. An e2e test in `tests/e2e.rs` verifying the compiled output (exit code or assembly)

If a milestone only has steps 1 and 2, step 3 is missing and is the highest-value next change for that feature.

---

## The Compile-Time vs. Runtime Test Trap

**The most important testing rule in this codebase:**

A test that only checks an exit code does not prove correct codegen. If galvanic constant-folds `fn main() -> i32 { 1 + 2 }` to `mov x0, #3; ret`, the exit code is 3 regardless of whether the `add` instruction was emitted. The test passes; the spec violation goes undetected.

The fix: use `compile_to_asm` and assert the presence of the specific runtime instruction:
```rust
let asm = compile_to_asm("fn add(a: i32, b: i32) -> i32 { a + b }\nfn main() -> i32 { add(3, 4) }");
// The `add` function must emit a runtime add instruction, not a constant.
assert!(asm.contains("\tadd\t") || asm.contains(" add "),
    "expected runtime `add` instruction, got:\n{asm}");
```

When adding a new arithmetic or control-flow feature, always add a `compile_to_asm` assertion alongside any `compile_and_run` check.

---

## Running Tests Locally

```sh
cargo test                         # all tests (excludes e2e on macOS — tools not present)
cargo test --lib                   # unit tests only
cargo test --test fls_fixtures     # parse-acceptance only
cargo test --test e2e              # e2e only (skips gracefully without cross tools)
cargo bench                        # benchmarks (requires criterion)
```

---

## Adding a New Test: Checklist

1. **New FLS feature?** → Add fixture to `tests/fixtures/fls_*.rs`, add parse-acceptance test to `tests/fls_fixtures.rs`, add e2e test to `tests/e2e.rs`.
2. **New IR instruction or codegen change?** → Add `compile_to_asm` assertion in `tests/e2e.rs` checking the specific instruction is emitted.
3. **New CLI behavior?** → Consider adding to `tests/smoke.rs` or the CI `fuzz-smoke` job.
4. **New size-sensitive type?** → Add `size_of::<T>()` assertion as a unit test in the relevant `src/*.rs` module.
