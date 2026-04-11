# Testing — How Galvanic Tests

This file exists so the runtime agent can write tests that match the project's conventions rather than inventing new patterns. Read this before adding any test.

---

## Test layers and when to use each

### 1. Unit tests — `src/*.rs` inline `#[cfg(test)]`

Used for: layout assertions, single-function correctness, edge cases that don't need the full pipeline.

Current examples:
- `lexer::tests::token_is_eight_bytes` — enforces `size_of::<Token>() == 8`

Pattern:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_eight_bytes() {
        assert_eq!(std::mem::size_of::<Token>(), 8);
    }
}
```

When to add: when the claim is about a specific function or type that doesn't need a full pipeline run.

---

### 2. FLS fixture parse tests — `tests/fls_fixtures.rs`

Used for: verifying galvanic can lex and parse a given Rust program without error. Does NOT test lowering or codegen.

Pattern (from the existing file):
```rust
fn assert_galvanic_accepts(fixture: &str) {
    let fixture_path = format!(
        "{}/tests/fixtures/{fixture}",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(&fixture_path).unwrap();
    let tokens = galvanic::lexer::tokenize(&source).unwrap();
    galvanic::parser::parse(&tokens, &source).unwrap();
}

#[test]
fn fls_6_expressions() {
    assert_galvanic_accepts("fls_6_expressions.rs");
}
```

**Fixture naming:** `fls_{section}_{topic}.rs` — e.g., `fls_8_1_let_else.rs` for FLS §8.1 let-else. Use the FLS section number, not a feature name.

**Fixture file requirements:** 
- Must be valid Rust that galvanic can parse (even if it can't lower or codegen it yet)
- Must contain a comment citing the FLS sections it exercises
- Must be self-contained (no `use std::*`, no imports galvanic doesn't support)

When to add: whenever a new FLS section is targeted. Add the fixture first, then the parse test, then (if lowering works) the e2e test.

---

### 3. E2E tests — `tests/e2e.rs`

Used for: full pipeline verification — lex → parse → lower → codegen → assemble → link → run with qemu.

The file is large (1.1MB). The pattern for compile-and-run tests:
- Build the galvanic binary with `env!("CARGO_BIN_EXE_galvanic")`
- Write a fixture to a temp file (or use `tests/fixtures/`)
- Run galvanic to produce `.s` output
- Assemble with `aarch64-linux-gnu-as`
- Link with `aarch64-linux-gnu-ld`
- Run with `qemu-aarch64` and check exit code

E2E tests only run cleanly when the cross toolchain and qemu are present (Ubuntu CI). On macOS without the toolchain, compile-and-assemble may still work but link/run won't.

When to add: when a feature is fully implemented through codegen and produces an ARM64 binary. The fixture in `tests/fixtures/` should already have a `.s` file committed if it was previously emitted.

---

### 4. Benchmarks — `benches/throughput.rs`

Used for: regression guard on lexer and parser throughput.

Pattern: criterion benchmarks using `black_box`, `Throughput::Bytes`, fixtures from `tests/fixtures/`.

When to add: when adding a new fixture that represents a common pattern (not one-off). The existing stress fixtures (`stress_let_bindings(n)`) are good models for scale testing.

---

## Adversarial test fixtures

Galvanic's research purpose means adversarial inputs are first-class. Good adversarial fixtures:

- **Many let bindings** — 1000+ let bindings in a single function (exercises stack slot limits)
- **Deeply nested blocks** — 500 levels of `{ let _x = 0;` (exercises parser recursion)
- **Parameters + arithmetic** — `fn foo(a: i32, b: i32) -> i32 { a + b * 2 - 1 }` with non-literal inputs (verifies the "compiler not interpreter" constraint)
- **Mixed type operations** — i32, u32, f64 in the same function
- **Named struct with many fields** — a struct with 20 fields exercises struct codegen at scale

When writing a new adversarial fixture, add a comment at the top explaining what it's testing and what the expected behavior is (e.g., "expects exit 0 and emits .s without error" vs. "expects a clean non-zero exit with an error message").

---

## What NOT to do

- Do not add a test that only exercises the case where all inputs are literals (that's testing an interpreter behavior, not a compiler). Pair it with a test where inputs come from function parameters.
- Do not add `#[ignore]` to a test unless it has a comment explaining exactly when the ignore should be removed.
- Do not remove tests to make the suite pass — if a test is failing, that's information.
- Do not add a fixture that `use`s crates galvanic doesn't support. All fixtures must be `no_std`-compatible or use only things galvanic's lexer/parser can handle.
