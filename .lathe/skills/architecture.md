# Architecture of galvanic

This file exists to answer: "What are the key design decisions in this codebase, and what must be preserved when making changes?"

---

## The two design goals

Every architectural decision in galvanic serves two masters simultaneously:
1. **FLS fidelity**: each data structure and function maps to a specific FLS section
2. **Cache-line awareness**: data layout is a first-class concern, not an afterthought

These goals are noted explicitly in the source code. When making changes, check both.

---

## Development approach: vertical slices

Galvanic grows by making progressively more complex programs compile end-to-end. Not by completing one phase before starting the next.

The pipeline is: **source text → tokens → AST → IR → ARM64 assembly → binary**

Each cycle should extend what galvanic can compile all the way through this pipeline. A change that only touches the front-end (lexer/parser) without extending the back-end is usually not the right change — unless the front-end is the bottleneck for the next end-to-end milestone.

### Milestone programs (in order)

These are the programs galvanic should be able to compile, in roughly this order:

1. `fn main() -> i32 { 0 }` — exit with code 0
2. `fn main() -> i32 { 1 + 2 }` — integer arithmetic
3. `fn main() -> i32 { let x = 42; x }` — local variables
4. `fn main() -> i32 { if true { 1 } else { 0 } }` — control flow
5. Two functions, one calls the other — function calls
6. Loops, mutation, basic control flow graphs

Each milestone should have an end-to-end test: compile the `.rs` file, run the binary, check the result.

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

**Coming next** (add these as they become needed, not before):

```
  ir.rs       — or ir/mod.rs — Intermediate representation for codegen
  codegen.rs  — or codegen/mod.rs — ARM64 instruction emission
```

The pipeline is strictly linear: source text → tokens → AST → IR → ARM64. Each new phase is added when the next vertical milestone requires it.

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

## ARM64 codegen strategy

For the initial milestones:
- Emitting assembly text (`.s` files) and using an external assembler/linker (`aarch64-linux-gnu-as`, `aarch64-linux-gnu-ld`) is the bootstrap approach. A built-in assembler can come later.
- On CI (ubuntu-latest, x86_64), use QEMU user-mode emulation (`qemu-aarch64`) to run ARM64 binaries.
- The first binary targets a bare Linux `_start` entry point using syscalls (no libc dependency). Exit via `mov x0, <code>; mov x8, #93; svc #0`.
- Cache-line awareness becomes concrete at codegen: instruction alignment, data section layout, stack frame layout. Every cache-line decision should be documented.

---

## What's implemented (as of current state)

| Phase | Status | Notes |
|---|---|---|
| Lexer | Working | Full token set; Unicode NFC not applied |
| Parser | Working | fn items, expressions, if-else, loops, let, blocks, calls, field access |
| IR | Does not exist | **This is the current bottleneck** |
| Codegen | Does not exist | **This is the current bottleneck** |
| End-to-end | Does not exist | No program can be compiled to a binary |

The front-end is ahead of the back-end. The next several cycles should focus on IR and codegen, not on widening the parser.

---

## The `no_std` goal

The README mentions "core Rust (`no_std`)". Galvanic itself uses `std` (it reads files, uses process::Command in tests). The `no_std` refers to the **subset of Rust it compiles** — the initial target is no_std Rust programs. The compiler is not itself no_std.
