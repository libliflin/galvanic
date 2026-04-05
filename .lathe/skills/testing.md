# Testing in galvanic

This file exists to answer: "What does a good test look like in this project, and where do tests live?"

---

## Current state (Phase 2, as of init)

There is exactly one test: `tests/smoke.rs::empty_file_exits_zero`. It:
1. Creates a temp file with a `.rs` suffix
2. Runs the `galvanic` binary on it
3. Asserts exit 0
4. Asserts stdout contains `"galvanic: compiling"`

This is an integration test (via `std::process::Command` + `env!("CARGO_BIN_EXE_galvanic")`). It tells us the binary runs. It tells us nothing about whether the lexer or parser produce correct output.

---

## Test locations

```
tests/          — Integration tests (Rust convention: separate test binaries)
  smoke.rs      — Current binary smoke test

src/lexer.rs    — Inline unit tests go in #[cfg(test)] mod tests { } at the bottom
src/parser.rs   — Same: inline #[cfg(test)] mod tests { }
src/ast.rs      — Less relevant; AST is just data structures
```

The pattern for inline tests in Rust:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        // ...
    }
}
```

---

## How to test the lexer

The public API is `lexer::tokenize(src: &str) -> Result<Vec<Token>, LexError>`.

A useful lexer test feeds a source string and checks the `TokenKind` sequence:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        tokenize(src)
            .unwrap()
            .iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn integer_literal() {
        assert_eq!(kinds("42"), vec![TokenKind::LitInteger, TokenKind::Eof]);
    }

    #[test]
    fn hex_literal() {
        assert_eq!(kinds("0xFF"), vec![TokenKind::LitInteger, TokenKind::Eof]);
    }

    #[test]
    fn fn_keyword() {
        assert_eq!(kinds("fn"), vec![TokenKind::KwFn, TokenKind::Eof]);
    }
}
```

Key things to test in the lexer:
- All literal types: integer (decimal, hex, octal, binary), float, string, char, byte literals
- Keywords: `fn`, `let`, `if`, `else`, `return`, `mut`, `pub`
- Operators and punctuation: `+`, `-`, `*`, `/`, `==`, `!=`, `->`, `::`, `{`, `}`
- Identifiers vs. keywords (e.g., `foo` vs. `fn`)
- Whitespace and comment stripping (they should not appear in output)
- Lifetime tokens: `'a`, `'static`
- Error cases: unterminated string, unknown character

---

## How to test the parser

The public API is `parser::parse(tokens: &[Token], src: &str) -> Result<SourceFile, ParseError>`.

The easiest way to test the parser is to lex first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;

    fn parse_ok(src: &str) -> SourceFile {
        let tokens = lexer::tokenize(src).expect("lex failed");
        parse(&tokens, src).expect("parse failed")
    }

    fn parse_err(src: &str) -> ParseError {
        let tokens = lexer::tokenize(src).expect("lex failed");
        parse(&tokens, src).expect_err("expected parse error")
    }

    #[test]
    fn empty_source() {
        let sf = parse_ok("");
        assert_eq!(sf.items.len(), 0);
    }

    #[test]
    fn simple_fn_no_args_no_return() {
        let sf = parse_ok("fn foo() {}");
        assert_eq!(sf.items.len(), 1);
        // Check it's a Fn item
        match &sf.items[0].kind {
            ItemKind::Fn(f) => assert_eq!(f.name.text("fn foo() {}"), "foo"),
            _ => panic!("expected Fn item"),
        }
    }

    #[test]
    fn fn_with_return_literal() {
        let src = "fn answer() -> i32 { 42 }";
        let sf = parse_ok(src);
        assert_eq!(sf.items.len(), 1);
        // body has no stmts, tail is LitInt(42)
    }
}
```

Key things to test in the parser:
- Empty source → 0 items
- Single fn with no params, no return type, empty body
- Fn with params: `fn add(a: i32, b: i32) -> i32 { a + b }`
- Return type: `-> i32`, `-> ()`, omitted (implicit unit)
- Tail expression vs. expression statement: `{ 42 }` vs. `{ 42; }`
- If expression: `if x { 1 } else { 2 }`
- Nested calls: `foo(bar(1, 2))`
- Let binding: `let x = 5;`, `let x: i32 = 5;`
- Binary operators across precedence levels: `a + b * c` (multiplication binds tighter)
- Multiple fns in one file
- Error cases: `fn foo(` → unterminated parameter list

---

## Integration test pattern (existing in tests/smoke.rs)

For testing the binary end-to-end, the existing pattern uses:
- `tempfile::NamedTempFile::with_suffix(".rs")` to create temp input files
- `env!("CARGO_BIN_EXE_galvanic")` to get the compiled binary path
- `std::process::Command` to run it

The `tempfile` crate is already a dev-dependency. Use this pattern for any integration tests that need real Rust source files on disk.

---

## What NOT to do

- Don't add golden-file tests (comparing serialized AST output to files). The AST Debug impl is fine for assertions, but golden files become brittle as the AST evolves.
- Don't test internal parser methods directly — test via the `parse()` public function.
- Don't delete the smoke test to "simplify." It catches a different failure mode (binary doesn't run at all) than unit tests.
