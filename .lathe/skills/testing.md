# Testing in Galvanic

This skill exists to answer: "What does a new test look like in this project, and where should I put it?"

---

## Test structure

Galvanic has three test layers, each with a distinct purpose:

### 1. `tests/fls_fixtures.rs` — FLS parse-acceptance tests

These verify that the lexer and parser accept programs drawn from the FLS without error. They do **not** test lowering, codegen, or correctness of output — only that the input is syntactically accepted.

**When to add**: When a new FLS section is added to the parser. The fixture file goes in `tests/fixtures/fls_X_Y_description.rs`.

**Convention**:
```rust
#[test]
fn fls_6_5_arithmetic() {
    assert_galvanic_accepts("fls_6_5_arithmetic.rs");
}
```

The helper `assert_galvanic_accepts(fixture)` reads the file, tokenizes it, and parses it — panicking with a clear message if either step fails.

**Naming**: `fls_{section}_{short_description}.rs`, e.g. `fls_6_15_for_loop.rs`. Match the FLS section number exactly.

---

### 2. `tests/smoke.rs` — CLI-level smoke tests

These test the binary's CLI behavior: argument handling, error messages, and exit codes.

**When to add**: When new CLI behavior is added (flags, error paths, output formats).

**Convention**: Use `Command::new(env!("CARGO_BIN_EXE_galvanic"))` to get the release binary path. `tempfile::NamedTempFile` for input files.

**What these test**: That the CLI doesn't panic on edge cases, that usage errors give clean exits, that expected output appears on stdout.

---

### 3. `tests/e2e.rs` — Full-pipeline compile-and-run tests

These test the entire pipeline: lex → parse → lower → codegen → assemble → link → run under qemu. The test verifies the compiled binary's exit code.

**When to add**: When a new milestone program is implemented end-to-end. These are the most valuable tests — they confirm real correctness.

**Structure**: The e2e test helpers (in `tests/e2e.rs`) typically:
1. Write a Rust source file to a temp path
2. Run `galvanic <source> -o <output>`
3. Assemble and link the output `.s` to a binary
4. Run the binary under `qemu-aarch64`
5. Assert the exit code matches expected

The corresponding `.s` golden file (if present in `tests/fixtures/`) shows the expected assembly output. When the codegen changes, update both the test and the golden file.

**CI requirement**: e2e tests run in the `e2e` CI job which installs `binutils-aarch64-linux-gnu` and `qemu-user`. Only add e2e tests that use these tools — don't add dependencies on other cross-compilation tools.

---

## Fixture files

`tests/fixtures/` contains two kinds of files:

- `*.rs` — Rust source programs used as test inputs. Some are parse-only fixtures; others are compile-and-run fixtures with corresponding `.s` files.
- `*.s` — Golden ARM64 assembly output files. Present when the fixture has been run through full codegen. The e2e tests may compare against these or simply verify runtime behavior.

**Invariant**: Every `.s` file must have a corresponding `.rs` file with the same stem. (This is CLAIM-5 in `claims.md` and is checked by `falsify.sh`.)

---

## Benchmarks

`benches/throughput.rs` uses Criterion to measure lexer and parser throughput on FLS fixture inputs and synthetic stress inputs (N let bindings).

**When to add**: When adding a new parser or lexer feature that has performance implications. Use `Throughput::Bytes` to express results per byte of input.

**Note**: Benchmarks run in CI with a short warm-up (`--warm-up-time 2 --measurement-time 3`). They don't gate CI (no `--bench` exit-code check), but they produce output that's compared for regressions.

---

## Library unit tests

Key unit tests in `src/lexer.rs`:
- `lexer::tests::token_is_eight_bytes` — asserts `std::mem::size_of::<Token>() == 8`. This is CLAIM-1 and must never be removed.
- `lexer::tests::span_is_eight_bytes` — asserts `Span` is 8 bytes (may or may not exist; `|| true` in CI).

If you add a new size-sensitive type with a cache-line claim, add a corresponding unit test asserting its size. These tests are the concrete enforcement of cache-line documentation.

---

## What a well-formed new test looks like

For a new FLS section (e.g., §6.15 for-loops):

1. Write `tests/fixtures/fls_6_15_for_loop.rs` with representative examples drawn from the FLS spec text.
2. Add `#[test] fn fls_6_15_for_loop() { assert_galvanic_accepts("fls_6_15_for_loop.rs"); }` to `tests/fls_fixtures.rs`.
3. If the section is fully implemented end-to-end, write a milestone fixture and add it to `tests/e2e.rs`.
4. If there are adversarial inputs that should fail gracefully (not panic), consider adding them to CLAIM-6 in `falsify.sh`.
