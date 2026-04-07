# Architecture — Galvanic's Pipeline and Design Decisions

This file exists to orient the runtime agent on galvanic's internal structure
so it can make changes that fit the existing design rather than fighting it.

---

## Pipeline

```
Source text (.rs)
     │
     ▼
  lexer.rs          tokenize() → Vec<Token>
     │
     ▼
  parser.rs         parse() → SourceFile (AST)
     │
     ▼
  ast.rs            — AST types (SourceFile, Item, FnDef, Expr, Stmt, Pat, Ty, ...)
     │
     ▼
  lower.rs          lower() → Module (IR)
     │
     ▼
  ir.rs             — IR types (Module, IrFn, Instr, IrValue, StaticData, ...)
     │
     ▼
  codegen.rs        emit_asm() → String (GAS ARM64 assembly text)
     │
     ▼
  main.rs           — assembles/links via aarch64-linux-gnu-as + aarch64-linux-gnu-ld
```

---

## Key Architectural Constraints

### IR is intentionally minimal

`src/ir.rs` grows only as required by the next milestone. Nothing is added
before it is needed. This keeps the IR simple and traceable to the FLS sections
that motivated each node type. Do not add IR nodes speculatively.

### Lowering emits runtime instructions, never interprets

`src/lower.rs` is a compiler pass, not an interpreter. Every non-const code
path emits runtime IR instructions. The litmus test is in the module doc:

> If replacing a literal with a function parameter would break the
> implementation, you built an interpreter, not a compiler.

The `lower` pass must not constant-fold non-const code. Any value that could
be a function parameter at runtime must be loaded from a stack slot or register.

### FLS traceability throughout

Every public type, trait, and significant function cites the relevant FLS section:

```rust
// FLS §9: Functions — each `IrFn` maps to one source-level function.
```

When the spec is ambiguous: `// FLS §X.Y: AMBIGUOUS — <describe the gap>`.
This is the project's primary research output.

---

## Token Layout (lexer.rs)

`Token` is exactly **8 bytes**. This is enforced by `token_is_eight_bytes` test.

```
offset 0 │ kind: TokenKind (u8, repr(u8)) — 1 byte
offset 1 │ (padding)                      — 1 byte  
offset 2 │ (padding)                      — 2 bytes
offset 4 │ span.start: u32                — 4 bytes
// wait, actually it's:
```

`TokenKind` uses `#[repr(u8)]` so the discriminant is 1 byte. Span uses 4+4 bytes (start: u32, len: u32). With the u8 discriminant and alignment, the total Token fits in 8 bytes — 8 tokens per 64-byte cache line.

This is why parser iteration over `Vec<Token>` is cache-efficient: one cache
line load covers 8 tokens.

**If you add a new `TokenKind` variant**, check that the total remains under 256
variants (u8 can hold 0..=255) and the test still passes.

---

## AST Layout (ast.rs)

AST nodes use `Box<T>` for recursive fields (e.g., `Box<Expr>` for nested expressions). This is acknowledged tech debt — an arena design with `u32` indices would be more cache-friendly, but the current priority is FLS correctness, not premature optimization. The module doc explicitly flags this.

`Span` is 8 bytes (start: u32, len: u32) — the one controlled layout in the AST.

---

## IR Layout (ir.rs)

`Instr` and `IrValue` are small enums intended to fit in one cache line per instruction. As new instruction types are added, their cache-line implications must be documented in a comment on the type.

`StaticData` holds a name (String, heap pointer) and a value (StaticValue enum). At this milestone, statics are not on the hot path — the cache-line tradeoff between statics (ADRP + LDR, 12 bytes) and constants (MOV, 4 bytes) is documented in the module.

---

## Codegen Conventions (codegen.rs)

- Emits **GNU assembler (GAS) syntax** for `aarch64-linux-gnu-as`.
- Target: AArch64 Linux ELF, bare `_start`, no libc.
- Entry point: `_start` calls `main`, then `sys_exit` (x8=93, x0=return value).
- ARM64 instructions are 4 bytes; 16 fill a 64-byte cache line.
- `emit_asm` returns a `String`. The caller (main.rs) writes it and assembles.
- **No `unsafe` code** in codegen.rs — string formatting uses `fmt::Write`.

---

## Module Boundaries

| File | Responsibility | FLS sections |
|------|---------------|--------------|
| `lexer.rs` | Tokenization | §2 Lexical Elements |
| `parser.rs` | AST construction | §3–§6, §8–§13 |
| `ast.rs` | AST types | all parsed constructs |
| `lower.rs` | AST → IR, type/scope context | §6.1.2, §7, §8, §9, §10... |
| `ir.rs` | IR types | IR design |
| `codegen.rs` | IR → ARM64 assembly text | §18.1 (entry point), §9 (functions) |
| `main.rs` | CLI, assemble+link | user-facing pipeline |

---

## Adding a New Feature (Standard Flow)

1. **Read the FLS section** for the feature.
2. **Add AST node(s)** in `ast.rs` with FLS citations.
3. **Add parser rule** in `parser.rs` mapping the grammar production.
4. **Add IR node(s)** in `ir.rs` only if needed.
5. **Add lowering** in `lower.rs` — emit runtime IR, never interpret.
6. **Add codegen** in `codegen.rs` — emit the ARM64 instruction(s).
7. **Add fixture** in `tests/fixtures/fls_X_Y_*.rs`.
8. **Add parse test** in `tests/fls_fixtures.rs`.
9. **Add assembly inspection test** in `tests/e2e.rs` — with positive AND negative assertion.
10. **Add compile-and-run test** in `tests/e2e.rs`.

Steps 9 and 10 are both required. Step 9 without step 10 means the runtime
execution was never tested. Step 10 without step 9 means an interpreter would
pass the test.

---

## Scope / Variable Storage

`lower.rs` maintains a scope stack (HashMap of name → stack slot index) during
lowering. Local variables are spilled to stack slots at declaration and loaded
at use. This is a deliberate simplicity choice — all variables go through the
stack. Register allocation optimization is future work.

Function parameters are spilled from argument registers (x0..x7) to stack slots
at function entry. This matches the AArch64 ABI for small integers.

---

## No Standard Library

Galvanic targets `no_std` ARM64 binaries (no libc startup, no runtime, direct
syscalls). The galvanic compiler itself uses std, but the programs it produces
do not link libc. The `_start` entry point calls main and exits via syscall 93.
