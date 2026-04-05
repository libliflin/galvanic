# Architecture

This skill exists to answer: what does the compiler pipeline look like, what exists today, and what order should phases be built in?

---

## Current state (as of initial skeleton)

The entire project is one file: `src/main.rs`. It does the following:

1. Reads `argv[1]` as a source file path.
2. Prints `"galvanic: compiling {filename}"`.
3. Exits 0.

No compilation happens. This is a binary scaffold, not a compiler.

---

## Pipeline phases (in order of implementation)

The compiler must be built phase by phase. Each phase depends on the previous one. Do not implement a later phase before the earlier one works.

### Phase 1: Lexer (tokenizer)
- Input: raw source text (`&str` or file contents)
- Output: a sequence of `Token` values
- FLS reference: §3 — Lexical structure
- Key decisions: What is the `Token` type? How are spans represented? How are errors reported?
- Cache-line angle: `Token` struct layout. If tokens are stored in a `Vec<Token>`, the struct size affects how many fit in a cache line during parsing.

### Phase 2: Parser
- Input: token stream from the lexer
- Output: an AST (abstract syntax tree)
- FLS reference: §5 — Expressions, §6 — Statements, §7 — Items
- Key decisions: What do AST nodes look like? Owned or arena-allocated? How are parse errors represented?
- Cache-line angle: AST node layout. Tree traversal patterns are cache-unfriendly by default; this is worth designing around from the start.

### Phase 3: Name resolution / type checking
- Input: raw AST
- Output: resolved AST with type annotations
- FLS reference: §8 — Names, §10 — Type system
- This phase is where most spec ambiguities tend to surface.

### Phase 4: IR (intermediate representation)
- Input: type-checked AST
- Output: a lower-level representation suitable for codegen
- Decision: what IR? LLVM? MIR-like? Custom? Given the cache-line research angle, a custom IR that makes data layout explicit may be valuable.

### Phase 5: Codegen (ARM64)
- Input: IR
- Output: ARM64 assembly or object code
- This is where the cache-line-aware codegen research question becomes concrete.

---

## Module structure (expected, not yet created)

As phases are implemented, suggest this layout:

```
src/
  main.rs          — CLI entry point (exists)
  lexer.rs         — or lexer/mod.rs when it grows
  parser.rs        — depends on lexer
  ast.rs           — AST types, referenced by parser
  resolve.rs       — name resolution
  types.rs         — type checker
  ir.rs            — IR types and lowering
  codegen/
    mod.rs
    arm64.rs       — ARM64 instruction emission
```

Do not create empty module files ahead of their implementation. Create each file when there is code to put in it.

---

## Design constraints

- **no_std target**: The README says galvanic implements "core Rust (no_std)". This means no standard library dependencies in the *compiled output* — but galvanic itself (the compiler binary) can and should use `std`. The compiler runs on a host machine with std available.
- **ARM64 only**: Codegen targets ARM64. No need to design for multiple architectures.
- **FLS fidelity first**: When the FLS and a pragmatic shortcut diverge, follow the FLS and document the divergence. The research value comes from strict FLS adherence.
- **Cache-line awareness**: Not a post-hoc optimization pass — a design constraint from the start. When making data structure decisions, think about how the data will be traversed and how that maps to cache lines.

---

## What "FLS-faithful" means in practice

When implementing a phase:
1. Read the relevant FLS sections before writing code.
2. Structure the code to mirror the spec structure where possible.
3. Add a comment above each major function or type with the FLS section it implements (e.g., `// FLS §3.2: Integer literals`).
4. If the spec is silent on something, add a `// FLS §X.Y: AMBIGUOUS — spec does not specify...` comment.
5. Record every ambiguity in the changelog's `FLS Notes` section.
