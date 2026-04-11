# Architecture — Galvanic Compiler Pipeline

This file exists because the runtime agent needs to understand the pipeline layering and the FLS-traceability convention before making any change. Both are non-obvious from a snapshot alone.

---

## Pipeline

```
source.rs
   │
   ▼
src/lexer.rs       — tokenize()    → Vec<Token>
   │                  FLS §2 (Lexical Elements)
   │                  Token: 8 bytes, repr(u8) discriminant
   ▼
src/parser.rs      — parse()       → SourceFile (AST)
   │                  FLS §3–§15 (Items, Types, Expressions)
   │                  Hand-written recursive descent
   ▼
src/ast.rs         — data types only, no logic
   │                  Span: 8 bytes (layout enforced)
   │                  Box<T> used for recursive types (arena is future work)
   ▼
src/lower.rs       — lower()       → Module (IR)
   │                  FLS §6 (Expressions), §8 (Statements), §9 (Functions)
   │                  Emits RUNTIME instructions only — no const folding in non-const contexts
   ▼
src/ir.rs          — data types only
   │                  IrFn, Instr, IrBinOp, StaticData, ClosureTrampoline, VtableShim/Spec
   │                  Each Instr variant documents its ARM64 encoding and cache-line cost
   ▼
src/codegen.rs     — emit_asm()    → String (GAS syntax)
                     FLS §9 (Functions), §18.1 (Entry point)
                     Target: AArch64 Linux ELF, GNU assembler syntax
                     Entry point: _start → main → sys_exit
```

The CLI (`src/main.rs`) drives the pipeline and optionally shells out to `aarch64-linux-gnu-as` + `aarch64-linux-gnu-ld` to produce a binary. Library code (`lib.rs` re-exporting the modules) never shells out.

---

## FLS Traceability Convention

Every piece of behavior implemented in galvanic must have a specific FLS citation. The project has two citation styles:

**Inline on the implementing line:**
```rust
// FLS §6.19: Return expressions — tail expression lowers to Instr::Ret.
body.push(Instr::Ret(ret_val));
```

**Doc comment on a type:**
```rust
/// FLS §2.6.1: Strict keywords.
pub enum TokenKind { ... }
```

**Ambiguities and deviations are documented with a specific tag:**
```rust
// FLS §6.23 AMBIGUOUS: spec requires a panic on divide-by-zero but the
// mechanism is unspecified. Galvanic does not yet insert the check.
```

**Partial implementations:**
```rust
// FLS §2.3 NOTE: NFC normalisation not yet applied. ASCII identifiers
// are handled correctly; non-ASCII is accepted but not normalised.
```

When adding a new feature, the citation goes on the specific code that implements the FLS behavior, not in a block comment at the top of the function. The reader should be able to find the FLS section and verify the line.

---

## Cache-Line Convention

Cache-line notes document layout decisions. They appear in:

1. **Type definitions** — explaining why a type has the size it has and what that buys.
2. **Module-level doc comments** — summarizing the hot-path layout story for that module.
3. **IR doc comments on Instr variants** — explaining the ARM64 instruction count and byte cost.

The only currently *enforced* cache-line constraints are:
- `Token: 8 bytes` — test `lexer::tests::token_is_eight_bytes`
- `Span: 8 bytes` — referenced in `ast.rs`, test may or may not exist

All other cache-line notes are design documentation. They become enforced when someone writes a `size_of!` test for them.

---

## Const-Evaluation Boundary (Critical)

See `.lathe/refs/fls-constraints.md`. The short version:

> Galvanic must emit **runtime instructions** for all non-const code. A regular `fn main()` body is NOT a const context, even if all values happen to be literals.

The litmus test: replacing a literal with a function parameter must not break the implementation. If a while loop only works when the bound is a literal (because lower.rs evaluates it at compile time), that is an interpreter, not a compiler.

`lower.rs` has a module-level comment confirming compliance. Every cycle that touches lowering should verify this invariant holds for the changed code path.

---

## IR Growth Pattern

The IR grows exactly one instruction type per milestone program. The pattern from `ir.rs`:

```
Milestone 11: LoadImm, BinOp        — arithmetic
Milestone 12: Store, Load           — let bindings / stack
Milestone 13: Label, Branch, CondBranch — control flow
Milestone 14: Call                  — function calls
Milestone 16: comparison ops        — while loops
Milestone 21: bitwise/shift ops     — §6.5.6, §6.5.7
```

When adding a new Instr variant:
1. Add it to `ir.rs` with FLS citation and cache-line note
2. Add the lowering in `lower.rs`  
3. Add the ARM64 emission in `codegen.rs`
4. Add a fixture in `tests/fixtures/` exercising it
5. Add the fixture to `tests/fls_fixtures.rs` (parse acceptance)
6. Add the fixture to `tests/e2e.rs` if it can be compiled and run end-to-end

---

## Test Layers

- **Unit tests** (`src/lexer.rs`, etc.) — layout assertions, specific edge cases
- **FLS fixture parse tests** (`tests/fls_fixtures.rs`) — each fixture in `tests/fixtures/` gets a `assert_galvanic_accepts()` call; verifies lex + parse only
- **E2E tests** (`tests/e2e.rs`) — full pipeline: galvanic → `.s` → `aarch64-linux-gnu-as` → `aarch64-linux-gnu-ld` → `qemu-aarch64` (runs on Ubuntu CI, requires cross toolchain)
- **Benchmarks** (`benches/throughput.rs`) — criterion, lexer + parser throughput on FLS fixtures and stress inputs

The e2e tests require `qemu-aarch64` and the cross toolchain. They run in CI on Ubuntu but not necessarily on macOS locally. If you're on macOS without qemu, the e2e tests will be skipped or fail gracefully.
