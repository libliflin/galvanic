# Changelog — Cycle 1

## Who This Helps

- **William (researcher)**: The binary now does something real. `tokenize("let x = 42;")` returns five tokens. The pipeline has its first working phase.
- **Compiler researchers**: `src/lexer.rs` is readable as a document. Every public item carries a `// FLS §2.X` citation. Four spec ambiguities are recorded explicitly in comments and here.
- **Spec ambiguity hunters**: Four gaps found in FLS §2 on the first pass through the lexical layer. See FLS Notes below.

## Observed

- `cargo build` and `cargo test` were clean; CI was green on `main`.
- The binary accepted a filename and printed `galvanic: compiling {filename}`. No compilation happened.
- No lexer existed. Without a lexer, no other compiler phase can be implemented.

## Applied

Implemented `src/lexer.rs` — a complete tokenizer per FLS §2.

**`TokenKind` enum** (`repr(u8)`): all 100+ token variants for strict keywords, reserved keywords, weak keywords, all literal types, lifetimes, every punctuation token from FLS §2.2.

**`Token` struct** (8 bytes):
```
offset 0: start: u32   — byte offset into source (4 bytes)
offset 4: len:   u16   — byte length of span (2 bytes)
offset 6: kind:  u8    — TokenKind discriminant (1 byte)
offset 7: (padding)    (1 byte)
```
At 64 bytes per cache line, 8 tokens fit per line. This is the first cache-line-aware data structure in the pipeline. The layout was a deliberate design decision, not a retrofitted optimization.

**`tokenize(source: &str) -> Result<Vec<Token>, LexError>`**: public API. Skips whitespace (FLS §2.1) and comments (FLS §2.5, including nested block comments). Returns all meaningful tokens terminated by `Eof`.

**`LexError`**: `UnexpectedChar`, `UnterminatedLiteral`, `UnterminatedBlockComment`, `InvalidEscape`, `SurrogateInUnicodeEscape`, `UnterminatedRawLiteral`.

**43 unit tests**: token layout size/alignment assertions, all keyword classes, all literal forms (decimal/hex/octal/binary integers, floats with exponents and suffixes, char, byte, string, byte string, raw string variants, C string, lifetimes), all punctuation, whitespace handling, error cases.

**Files modified:**
- `src/lexer.rs` (new, 1632 lines)
- `src/main.rs` (added `#[allow(dead_code)] mod lexer;`)

## FLS Notes

**FLS §2.3 / §2.6 AMBIGUOUS — `_` classification**: `_` appears in both the strict-keyword table and the punctuation table in the FLS. The spec does not state a precedence rule. Decision: bare `_` (not followed by XID_Continue) emits `TokenKind::Underscore`; `_foo` and `__` emit `TokenKind::Ident`. This matches rustc behavior but is not derivable from the FLS alone.

**FLS §2.3 NOTE — Unicode normalisation**: The spec requires identifiers to undergo Unicode NFC normalisation and use XID_Start / XID_Continue categories. This implementation handles ASCII correctly. Non-ASCII alphabetic characters are accepted via `char::is_alphabetic()` and `char::is_alphanumeric()`, but NFC normalisation is not applied. A production implementation would use the `unicode-ident` crate. The gap is documented in the code.

**FLS §2.4 NOTE — Float/integer disambiguation**: The spec describes the rule informally: "a decimal literal followed by `.` not followed by another decimal digit or an identifier." The boundary case `1.` (is it a float or integer-dot?) is not formally specified in terms of the tokenizer's lookahead. Implementation: `.` followed by a letter, `_`, or another `.` is not a float decimal point; the integer terminates before the `.`. This matches rustc behavior but the FLS does not explicitly state this lookahead rule.

**FLS §2.6 AMBIGUOUS — `'static` lifetime keyword boundary**: `'static` is listed as a weak keyword in FLS §2.6, but the spec does not define at which layer (lexer vs. parser) `'static` should be distinguished from other lifetime names. All `'ident` forms are emitted as `TokenKind::Lifetime`; the parser will assign special meaning to `'static`. This is consistent with how rustc works but requires an inference not stated in the spec.

**FLS §2.5 — CR in comments**: The spec states carriage return (CR) is forbidden in comments. The current implementation allows CR in comments (it skips them as part of "end of line"). A strict implementation would emit an error. Noted as a future gap.

## Validated

```
cargo build   — clean (0 warnings)
cargo test    — 43 passed, 0 failed (42 new + 1 existing smoke)
cargo clippy -- -D warnings — clean
```

PR: https://github.com/libliflin/galvanic/pull/1

## Next

The lexer exists and is tested. The next phase is the parser: consume a `Vec<Token>` and produce an AST.

Before writing a parser, the right questions are:
- What does FLS §5–§7 say about expressions, statements, and items?
- What does an AST node look like in a cache-line-aware design? (Arena allocation is likely; tree traversal patterns are cache-unfriendly by default.)
- What's the minimal grammar to parse `fn main() {}` — the smallest complete Rust program?
