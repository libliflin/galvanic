# Architecture of galvanic

This file exists to answer: "What are the key design decisions in this codebase, and what must be preserved when making changes?"

---

## The two design goals

Every architectural decision in galvanic serves two masters simultaneously:
1. **FLS fidelity**: each data structure and function maps to a specific FLS section
2. **Cache-line awareness**: data layout is a first-class concern, not an afterthought

These goals are noted explicitly in the source code. When making changes, check both.

---

## Data layout constraints (DO NOT violate without explicit rationale)

### Token: 8 bytes

```rust
// src/lexer.rs
struct Token {
    kind: TokenKind,  // u8 (repr(u8))
    // ...
}
```

`Token` is designed to be 8 bytes so 8 tokens fit in one 64-byte cache line. `TokenKind` uses `#[repr(u8)]` to keep the discriminant to 1 byte. The parser's hot token iteration is the beneficiary. **Do not add fields to `Token` that would push it past 8 bytes without documenting the trade-off.**

### Span: 8 bytes

```rust
pub struct Span {
    pub start: u32,  // 4 bytes
    pub len: u32,    // 4 bytes
}
```

Exactly 8 bytes. Two `Span`s fit alongside a `Token` in one cache line. This is used for connecting AST nodes back to source for diagnostics. **Do not add fields.**

### AST nodes: Box-based for now, arena-flagged for later

The AST currently uses `Box<T>` for recursive types (e.g., `Box<Expr>` in `ExprKind::Unary`). The source code explicitly documents this as a known cache-inefficiency and flags an arena redesign (`u32` indices into flat `Vec<ExprData>`) as future work. **Do not "improve" this by changing the design. The Arena redesign is a planned future phase, not something to do incrementally.**

---

## Module structure

```
src/
  main.rs     — CLI entry point: reads args, calls lexer::tokenize, calls parser::parse
  lexer.rs    — Tokenizer: tokenize(src) -> Result<Vec<Token>, LexError>
  ast.rs      — AST node types: SourceFile, Item, FnDef, Expr, Stmt, etc.
  parser.rs   — Parser: parse(tokens, src) -> Result<SourceFile, ParseError>
```

The pipeline is strictly linear: source text → tokens → AST. There is no IR, no name resolution, no type checking, no codegen yet. The pipeline currently ends after parsing.

---

## FLS citation pattern

Every significant type and function includes an FLS section citation. The format is:

```rust
/// FLS §9: Functions.
```

For ambiguities:
```rust
/// FLS §9 AMBIGUOUS: the spec lists `FunctionQualifiers` but does not
/// enumerate which qualifier combinations are legal. ...
```

For notes about non-ASCII or other known gaps:
```rust
/// FLS §2.3 NOTE: this implementation handles ASCII correctly; non-ASCII
/// identifier characters are accepted but NFC normalisation is not yet applied.
```

**When adding new code, always include the relevant FLS section reference.** This is how galvanic tracks spec coverage and documents what's been verified.

---

## Parser design

Hand-written recursive descent. Each grammar rule maps to one method. Methods:
- Return `Result<T, ParseError>` — error means "leave cursor at offending token"
- Use `expect(kind)` to consume a required token or return a descriptive error
- Use `eat(kind)` to optionally consume a token (returns bool)
- Use `peek_kind()` to look ahead without consuming

**Precedence climbing** is encoded structurally: `parse_expr` → `parse_assign` → `parse_or` → `parse_and` → `parse_comparison` → ... → `parse_unary` → `parse_primary`. The chain matches the precedence table in the parser module doc.

---

## What's implemented (as of Phase 2)

| FLS Section | Status | Notes |
|---|---|---|
| §2 Lexical elements | Complete-ish | Full token set; Unicode NFC not applied |
| §3 Items | Partial | Only `fn` items; no struct, enum, trait, impl, use, mod |
| §4 Types | Partial | Path, Unit, Ref; no generics, tuples, arrays, slices |
| §6 Expressions | Partial | Literals, paths, blocks, unary, binary (all ops), calls, if, return |
| §8 Statements | Partial | Let, expression statement, empty; no item statements |
| §9 Functions | Partial | No qualifiers (const, async, unsafe, extern); no where clauses |

---

## What comes next (by FLS section, not yet implemented)

In rough dependency order:
- §3: `struct`, `enum`, `type`, `use`, `mod`, `impl`, `trait` items
- §4: Generic type arguments (`Vec<i32>`), tuple types, array/slice types
- §5: Patterns (struct patterns, tuple patterns, match arms)
- §6: Method calls, field access, index expressions, closure expressions, loop/while/for
- §7: Closures
- §10: Traits
- §11: Implementations
- §12–§17: Advanced items
- Codegen: ARM64 instruction selection, register allocation, cache-line-aware layout

---

## The `no_std` goal

The README mentions "core Rust (`no_std`)". Galvanic itself uses `std` (it reads files, uses process::Command in tests). The `no_std` refers to the **subset of Rust it compiles** — the initial target is no_std Rust programs. The compiler is not itself no_std.
