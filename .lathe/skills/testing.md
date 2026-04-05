# Testing

This skill exists to answer: how does galvanic test things, and what should new tests look like?

---

## What exists today

### Integration test: `tests/smoke.rs`

One test: `empty_file_exits_zero`.

```rust
use std::process::Command;

#[test]
fn empty_file_exits_zero() {
    let empty = tempfile::NamedTempFile::with_suffix(".rs").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .arg(empty.path())
        .output()
        .expect("failed to run galvanic");

    assert!(output.status.success(), "expected exit 0, got {:?}", output.status);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("galvanic: compiling"), "unexpected output: {stdout}");
}
```

**Pattern**: Use `tempfile::NamedTempFile` to create a real file on disk. Use `Command::new(env!("CARGO_BIN_EXE_galvanic"))` to run the actual compiled binary. Assert on exit status and stdout.

**Dev dependency**: `tempfile = "3"` is already in `Cargo.toml`.

---

## How to add new integration tests

Add to `tests/smoke.rs` or create new files under `tests/` for each major phase. Follow the established pattern:

- Use `tempfile` to write real `.rs` source files.
- Run the binary with `Command::new(env!("CARGO_BIN_EXE_galvanic"))`.
- Assert on exit code, stdout, and (when codegen exists) output files.

Example for a future lexer test (when galvanic supports a `--lex` or `--tokens` flag):

```rust
#[test]
fn tokenizes_simple_let() {
    let mut src = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    writeln!(src, "let x = 42;").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_galvanic"))
        .args(["--tokens", src.path().to_str().unwrap()])
        .output()
        .expect("failed to run galvanic");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Keyword(let)") || stdout.contains("let"), "unexpected: {stdout}");
}
```

---

## Unit tests for library code

When internal modules exist (lexer, parser, etc.), unit tests should live in the same file using `#[cfg(test)]` modules:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_integer_literal() {
        let tokens = tokenize("42");
        assert_eq!(tokens, vec![Token::IntLiteral(42)]);
    }
}
```

Unit tests are better for testing individual components in isolation. Integration tests verify end-to-end behavior.

---

## Test inputs to use

Don't test only trivial inputs. When a phase is implemented, test it against:

1. **Empty input** — already covered by smoke test
2. **Minimal valid input** — `fn main() {}` is the smallest complete Rust program
3. **All token types** — identifiers, keywords, integer literals, float literals, string literals, char literals, operators, punctuation, comments (line and block)
4. **Error cases** — malformed input: unclosed string literal, invalid character, etc.
5. **`no_std` patterns** — since galvanic targets no_std, test with `#![no_std]` at the top

Build these test inputs yourself — you don't need external Rust code. A `let x: u32 = 42;` or `fn add(a: i32, b: i32) -> i32 { a + b }` covers a wide range of lexical elements.

---

## Running tests

```
cargo test                          # all tests
cargo test -- --nocapture           # with stdout visible
cargo test smoke                    # run only tests matching "smoke"
```

CI runs `cargo test` without `--nocapture`. Make sure tests pass without extra flags.
