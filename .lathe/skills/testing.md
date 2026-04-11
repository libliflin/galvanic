# Testing in Galvanic

This file answers: how does galvanic test? What are the layers, what are the conventions, and what should new tests look like?

---

## Three test layers

### 1. Parse-acceptance tests (`tests/fls_fixtures.rs`)

These tests verify that the lexer + parser accept a fixture file without error. They do NOT test lowering or codegen.

**Pattern:**
```rust
#[test]
fn fls_6_17_if_expressions() {
    assert_galvanic_accepts("fls_6_17_if_expressions.rs");
}
```

The `assert_galvanic_accepts` helper (defined at the top of `fls_fixtures.rs`) reads the fixture, runs the lexer, runs the parser, and panics on any error. It does not invoke `lower` or `codegen`.

**Fixture naming**: `fls_{section}_{name}.rs` — always derived from FLS section numbers. Never invent test programs — derive them from FLS examples. If the spec doesn't provide an example, note that in a comment at the top of the fixture.

**When to add**: Every new parser feature gets a fixture. The fixture should exercise the grammar construct in isolation, plus one or two combinations with related constructs.

**Location**: `tests/fixtures/` — all fixture files live here.

### 2. Assembly-inspection tests (`tests/e2e.rs`, `compile_to_asm` helper)

These tests run the full galvanic pipeline (lex → parse → lower → codegen) and inspect the emitted ARM64 assembly text — without assembling or linking.

**The key helper:**
```rust
fn compile_to_asm(source: &str) -> String {
    let tokens = galvanic::lexer::tokenize(source).expect("lex failed");
    let sf = galvanic::parser::parse(&tokens, source).expect("parse failed");
    let module = galvanic::lower::lower(&sf, source).expect("lower failed");
    galvanic::codegen::emit_asm(&module).expect("codegen failed")
}
```

**Why this matters**: A test that only checks the exit code cannot distinguish "compiled correctly" from "constant-folded at compile time." Assembly inspection closes that gap. For example:

```rust
let asm = compile_to_asm("fn main() -> i32 { 1 + 2 }\n");
assert!(asm.contains("add"), "expected 'add' instruction for 1+2, got:\n{asm}");
```

This test works on any platform (including macOS). Use it whenever you want to verify that galvanic emits *specific instructions*, not just that the exit code is right.

### 3. End-to-end run tests (`tests/e2e.rs`, `compile_and_run` helper)

These tests assemble and execute the binary via QEMU. They only run on Linux with `aarch64-linux-gnu-as`, `aarch64-linux-gnu-ld`, and `qemu-aarch64` installed. The CI `e2e` job installs these. On macOS they are silently skipped via `tools_available()`.

**When to add**: When the correct exit code provides additional confidence beyond what assembly inspection shows. Don't duplicate — if an assembly inspection test already covers a feature, an e2e test for the same feature is lower priority.

### 4. CLI smoke tests (`tests/smoke.rs`)

Tests the galvanic binary's command-line behavior: usage errors, missing files, empty files. These run everywhere. Use `env!("CARGO_BIN_EXE_galvanic")` to locate the binary.

### 5. Unit tests (inline in `src/*.rs`)

Structural invariants live here — mainly size assertions. The key existing test:

```rust
// src/lexer.rs
#[test]
fn token_is_eight_bytes() {
    assert_eq!(std::mem::size_of::<Token>(), 8);
}
```

Add `size_of` assertions whenever a new IR type or token type has a stated cache-line budget.

---

## What makes a good fixture

1. **Derived from the FLS, not invented.** Start with the FLS section's examples. Write the fixture as if it were a spec compliance test.

2. **Covers the grammar, not the happy path.** A fixture for `§6.17` (if expressions) should include: bare `if`, `if-else`, `if-let`, nested `if-else`, `if` with a block body containing multiple statements, `if` as an expression in a larger expression. One example is not a fixture.

3. **Has a comment header** citing the FLS section and explaining what it tests:
```rust
//! Fixture: FLS §6.17 — If and if-let expressions.
//! Tests galvanic's parser acceptance of the various if-expression forms.
//! Examples derived from FLS §6.17.
```

4. **Is parseable but doesn't need to be lowerable.** Parse-acceptance fixtures may contain constructs (generics, traits, closures) that the lowering pass doesn't support yet. That's fine — note it.

---

## FLS-tracing convention in tests

Every new test function should carry an inline comment citing the FLS section(s) it exercises. This is the same convention as production code. Example:

```rust
/// FLS §6.5.5: Addition operator. Verifies runtime instruction emission.
#[test]
fn add_emits_runtime_instruction() {
    let asm = compile_to_asm("fn main() -> i32 { 1 + 2 }\n");
    assert!(asm.contains("add"), "expected 'add' instruction");
}
```

---

## The const-vs-runtime litmus test (critical)

Any test that only checks an exit code can pass even if galvanic is constant-folding non-const code. Before writing or accepting a lowering test, ask:

> Would this test still pass if `lower.rs` just evaluated the expression at compile time and emitted `mov x0, #<result>`?

If yes, the test is insufficient. Add an assembly-inspection check that verifies specific runtime instructions are present. See `fls-constraints.md` (loaded as a ref) for full context on why this matters.

---

## Running tests locally on macOS

- `cargo test` — runs everything except e2e tests that require cross tools (those silently skip)
- `cargo test --lib` — unit tests only (fast, for structural invariants)
- `cargo test --test fls_fixtures` — parse-acceptance tests only
- `cargo test --test e2e` — assembly inspection + e2e tests (e2e runtime tests skip on macOS)
- `cargo clippy -- -D warnings` — lint (required to pass before commit)
