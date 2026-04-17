# Testing — Galvanic

## Test runner

`cargo test` runs all tests. Three test files, each with a different scope:

---

## Test files

### `tests/smoke.rs` — Binary behavior

Tests the compiled `galvanic` binary directly via `std::process::Command`. Verifies the CLI contract: exit codes, usage messages, file-not-found behavior. These are the fastest tests and run without cross-compilation tools.

### `tests/fls_fixtures.rs` — Parse acceptance

Each test calls `assert_galvanic_accepts(fixture_name)`, which runs the lexer and parser on a fixture file and asserts no errors. These tests do **not** run lowering or codegen — they verify that galvanic can parse FLS-derived examples, even when lowering isn't yet implemented.

Adding a new fixture: drop a `.rs` file in `tests/fixtures/`, add a corresponding `#[test]` fn in `fls_fixtures.rs`.

### `tests/e2e.rs` — Full pipeline

Two kinds of tests live here:

**Assembly inspection** (`compile_to_asm(source)`): Runs the full lex → parse → lower → codegen pipeline and returns the emitted ARM64 assembly text as a `String`. These work everywhere — macOS, Linux, CI — because they never invoke the assembler or linker. Use these to verify the compiler emits the correct instruction forms. This is the primary way FLS §6.1.2 compliance is checked: inspect the assembly to confirm runtime instructions are emitted, not constant-folded results.

**Runtime execution** (`compile_and_run(source, expected_exit)`): Assembles the output with `aarch64-linux-gnu-as`, links with `aarch64-linux-gnu-ld`, runs under `qemu-aarch64`, and checks the exit code. These are **skipped** when the cross-toolchain is absent. On macOS, they are always skipped (macOS cannot run Linux ELF binaries even on Apple Silicon — the syscall ABI is different). CI (ubuntu-latest) is the authoritative source of truth for runtime tests.

The e2e suite is large (1,700+ assembly inspection tests). Run `cargo test --test e2e` to run only e2e tests. Run `cargo test --test e2e compile_to_asm_add` (any substring match) to run a specific test.

---

## Test fixtures (`tests/fixtures/`)

Each fixture is a `.rs` file containing real Rust code drawn from FLS examples. Some have a corresponding `.s` file (expected or generated ARM64 assembly).

Fixtures without `.s` files: parse-acceptance only — the construct is parsed but not yet lowered or compiled.  
Fixtures with `.s` files: the compiler can compile them end-to-end.

---

## CI jobs

Five jobs run on every PR and push to `main`:

| Job | What it runs |
|---|---|
| `build` | `cargo build`, `cargo test`, `cargo clippy -- -D warnings` |
| `fuzz-smoke` | Binary edge cases: no args, missing file, empty file, large inputs, deep nesting, binary garbage, NUL bytes, long lines |
| `audit` | No `unsafe` in `src/`, no `Command` in library code, no network deps in `Cargo.toml` |
| `e2e` | Installs `binutils-aarch64-linux-gnu` + `qemu-user`, runs `cargo test --test e2e` including runtime tests |
| `bench` | Runs throughput benchmarks, checks that data structure sizes haven't grown |

---

## Key invariants enforced by CI

**No unsafe code.** The `audit` job rejects any `unsafe { }`, `unsafe fn`, or `unsafe impl` in `src/`. CI blocks a PR if unsafe appears.

**No constant folding.** The FLS §6.1.2 constraint says all non-const code must emit runtime instructions. Tests in `e2e.rs` check the emitted assembly directly — a test that passes but emits a constant result instead of a runtime instruction is a false positive. The `compile_to_asm()` helper exists specifically to catch this.

**No `Command` in library code.** `main.rs` may invoke `aarch64-linux-gnu-as` and `aarch64-linux-gnu-ld`. The library (`lexer.rs`, `parser.rs`, `ir.rs`, `lower.rs`, `codegen.rs`) must never shell out.

**Token size.** `cargo test --lib -- --exact lexer::tests::token_is_eight_bytes` verifies the `Token` type fits in 8 bytes. This is a load-bearing cache-line invariant — each token takes exactly one slot alongside a `Span` in a 16-byte cache line.

---

## Adding a new feature test

1. Write a fixture in `tests/fixtures/your_feature.rs` with a minimal Rust program using the feature.
2. Add `fn fls_your_feature() { assert_galvanic_accepts("your_feature.rs"); }` in `fls_fixtures.rs` to verify parsing.
3. Once lowering and codegen work, add an assembly inspection test in `e2e.rs`:
   ```rust
   #[test]
   fn your_feature_emits_correct_asm() {
       let asm = compile_to_asm("fn main() { /* your feature */ }");
       assert!(asm.contains("add"), "expected add instruction");
       // Assert runtime instruction, not constant result
   }
   ```
4. Once the cross-toolchain is available on your machine, add a runtime test:
   ```rust
   #[test]
   fn your_feature_exits_correctly() {
       if !tools_available() { return; }
       compile_and_run("fn main() -> i32 { 2 + 3 }", 5);
   }
   ```
