# Ferrocene Language Specification — Reference Pointer

The FLS is the authoritative source for galvanic's implementation decisions.

**URL**: https://rust-lang.github.io/fls/
(Previously at spec.ferrocene.dev, which now redirects here.)

Fetch current content directly from the spec. The spec is versioned and may update.

---

## FLS Table of Contents (verified 2026-04-05)

Use these section numbers in code citations (`FLS §X.Y`).

### §2 — Lexical Elements (→ lexer.rs)
- §2.1 Character Set
- §2.2 Lexical Elements, Separators, and Punctuation
- §2.3 Identifiers
- §2.4 Literals
  - §2.4.1 Byte Literals
  - §2.4.2 Byte String Literals (§2.4.2.1 Simple, §2.4.2.2 Raw)
  - §2.4.3 C String Literals (§2.4.3.1 Simple, §2.4.3.2 Raw)
  - §2.4.4 Numeric Literals (§2.4.4.1 Integer, §2.4.4.2 Float)
  - §2.4.5 Character Literals
  - §2.4.6 String Literals (§2.4.6.1 Simple, §2.4.6.2 Raw)
  - §2.4.7 Boolean Literals
- §2.5 Comments
- §2.6 Keywords (§2.6.1 Strict, §2.6.2 Reserved, §2.6.3 Weak)

### §3 — Items (→ parser.rs, ast.rs)

### §4 — Types and Traits
- §4.1–§4.9 Type kinds
- §4.10 Type Aliases
- §4.11 Representation
- §4.12 Type Model
- §4.13 Traits
- §4.14 Trait and Lifetime Bounds

### §5 — Patterns

### §6 — Expressions (→ parser.rs, ast.rs)
- §6.1 Expression Classification
- §6.2 Literal Expressions
- §6.3 Path Expressions
- §6.4 Block Expressions (§6.4.1 Async, §6.4.2 Const, §6.4.3 Named, §6.4.4 Unsafe)
- §6.5 Operator Expressions (§6.5.1–§6.5.12: borrow, deref, ?, negation, arith, bit, cmp, logical, type cast, assignment, compound assignment)
- §6.6 Underscore Expressions
- §6.7 Parenthesized Expressions
- §6.8 Array Expressions
- §6.9 Indexing Expressions
- §6.10 Tuple Expressions
- §6.11 Struct Expressions
- §6.12 Invocation Expressions (§6.12.1 Call, §6.12.2 Method Call)
- §6.13 Field Access Expressions
- §6.14 Closure Expressions
- §6.15 Loop Expressions (§6.15.1 For, §6.15.2 Infinite, §6.15.3 While, §6.15.4 While Let, §6.15.6 Break, §6.15.7 Continue)
- §6.16 Range Expressions
- §6.17 If / If Let Expressions
- §6.18 Match Expressions
- §6.19 Return Expressions
- §6.20 Await Expressions
- §6.21 Expression Precedence
- §6.22 Capturing
- §6.23 Arithmetic Overflow

### §7 — Values
### §8 — Statements (§8.1 Let, §8.2 Expression Statements)
### §9 — Functions
### §10 — Associated Items
### §11 — Implementations
### §12 — Generics
### §13 — Attributes
### §14 — Entities and Resolution
### §15 — Ownership and Destruction
### §16 — Exceptions and Errors
### §17 — Concurrency
### §18 — Program Structure and Compilation
### §19 — Unsafety
### §20 — Macros
### §21 — FFI
### §22 — Inline Assembly

---

## How to use the FLS in this project

1. Before implementing any language feature, read the relevant FLS section.
2. Add a comment to your code: `// FLS §X.Y: <brief description of what this implements>`
3. If the spec is ambiguous or silent: `// FLS §X.Y: AMBIGUOUS — <describe the gap>`
4. Record every ambiguity in the `FLS Notes` section of the cycle's changelog.

The ambiguity notes are the primary research output of this project.

---

## Test fixtures derived from FLS examples

The `tests/fixtures/` directory contains programs built from FLS examples:

- `fls_2_4_literals.rs` — Integer/float/boolean literals from §2.4
- `fls_6_expressions.rs` — Expression forms from §6
- `fls_9_functions.rs` — Function definitions from §9

**When adding a new language feature, add or extend a fixture file with an example from the relevant FLS section.** Do not invent Rust programs — derive them from the spec. If the spec doesn't provide an example, note that in the fixture comment.

---

## Known FLS gaps to watch for

- Macro expansion (§20) is underspecified for full semantics.
- The memory model (§15) may lag behind the Rust reference.
- `no_std`: the FLS covers language semantics; `core` vs `std` is a library concern.
- Non-ASCII identifiers: FLS §2.3 requires Unicode NFC normalization — not yet implemented.
