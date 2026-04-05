# Ferrocene Language Specification — Reference Pointer

The FLS is the authoritative source for galvanic's implementation decisions.

**URL**: https://spec.ferrocene.dev/

Fetch current content directly from the spec rather than relying on a snapshot here. The spec is versioned and may update.

---

## Key sections for each compiler phase

### Lexer (Phase 1)
- **§3 — Lexical structure** — Tokens, whitespace, comments
  - §3.1 — Character set
  - §3.2 — Whitespace
  - §3.3 — Comments (line comments `//`, block comments `/* */`, doc comments)
  - §3.4 — Tokens (keywords, identifiers, literals, punctuation, operators)
  - §3.5 — Integer literals (decimal, hex, octal, binary; suffixes like `u32`, `i64`)
  - §3.6 — Float literals
  - §3.7 — String literals (regular, raw, byte)
  - §3.8 — Char literals

### Parser / AST (Phase 2)
- **§5 — Expressions** — All expression forms and their precedence
- **§6 — Statements** — `let`, expression statements, semicolon rules
- **§7 — Items** — `fn`, `struct`, `enum`, `impl`, `use`, `mod`, etc.
- **§4 — Macros** — Can defer macro expansion; implement a stub that preserves macro calls

### Name resolution (Phase 3)
- **§8 — Names and paths** — How identifiers resolve to items
- **§9 — Scopes** — Scope rules, shadowing, lifetimes in scope

### Type system (Phase 3)
- **§10 — Type system** — Type inference, coercions, trait objects
- **§11 — Trait system** — Trait resolution, impl selection

### Codegen (Phase 5)
- **§15 — Memory model** — Alignment requirements, layout of structs and enums
- **§16 — Operational semantics** — What operations actually mean at runtime

---

## How to use the FLS in this project

1. Before implementing any language feature, read the relevant FLS section.
2. Add a comment to your code: `// FLS §X.Y: <brief description of what this implements>`
3. If the spec is ambiguous or silent: `// FLS §X.Y: AMBIGUOUS — <describe the gap>`
4. Record every ambiguity in the `FLS Notes` section of the cycle's changelog.

The ambiguity notes are the primary research output of this project. Be thorough.

---

## Known FLS limitations to watch for

- The FLS is written for Rust semantics as implemented by rustc. Some edge cases may only be implied by examples, not stated as rules.
- Macro expansion is underspecified — the FLS describes the surface syntax but not full expansion semantics.
- The memory model section may lag behind the Rust reference on newer features.
- `no_std` environments: the FLS covers language-level semantics; `core` vs `std` availability is a library concern, but the language spec assumes `core` is always present.
