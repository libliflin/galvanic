# Architecture — Galvanic

Key architectural decisions visible in the code. Read this before touching any pipeline stage.

---

## Pipeline

```
source text
    │
    ▼
lexer::tokenize()     →  Vec<Token>           (FLS §2 — Lexical Elements)
    │
    ▼
parser::parse()       →  SourceFile (AST)     (FLS §5–§6, §7–§14, §18)
    │
    ▼
lower::lower()        →  Module (IR)           (FLS semantics — all language rules)
    │
    ▼
codegen::emit_asm()   →  String (ARM64 GAS)   (AAPCS64 ABI, cache-line discipline)
    │
    ▼
aarch64-linux-gnu-as  →  .o object file
    │
    ▼
aarch64-linux-gnu-ld  →  ELF binary (Linux ARM64)
```

Each stage has one job and a clean boundary. Nothing earlier in the pipeline knows about later stages. The IR is the contract between language semantics (`lower`) and machine instructions (`codegen`).

Only `src/main.rs` shells out to external processes. All library code (`lexer`, `parser`, `ast`, `ir`, `lower`, `codegen`) is pure Rust with no I/O.

---

## Module responsibilities

| Module | Job | FLS sections |
|--------|-----|--------------|
| `lexer` | Source text → `Vec<Token>`. Each `Token` is exactly 8 bytes (8 per 64-byte cache line). | §2 |
| `ast` | AST type definitions only — no logic. Structs for `Item`, `Expr`, `Pat`, `Ty`, etc. | §5, §6, §7–§14 |
| `parser` | `Vec<Token>` → `SourceFile` (AST). Recursive descent. | §5, §6, §7–§14, §18 |
| `ir` | IR type definitions: `Module`, `IrFn`, `Instr`, `IrValue`, `IrTy`. Every node carries an FLS traceability comment. | §4, §6.19, §8, §9 |
| `lower` | AST → IR. All FLS semantic rules live here. Emits runtime IR for non-const code (never const-folds). | All language semantics |
| `codegen` | IR → ARM64 GAS assembly. Cache-line discipline lives here. | ARM64 ISA, AAPCS64 |
| `main` | CLI driver. Reads source file, runs pipeline, shells out to assembler/linker. The only module that uses `std::process::Command`. | — |

---

## Two-tier lowering

`lower.rs` uses two paths depending on what the function returns:

**Tier 1 — scalar path (`lower_expr`):** For expressions producing a single scalar (`i32`, `bool`, `f32`, `f64`, pointer types). Returns an `IrValue` (register or constant). This is the default.

**Tier 2 — composite path (`lower_*_expr_into`):** For expressions inside functions whose return type is a composite (struct, enum, tuple). Writes into pre-allocated stack slots instead of returning a value — implements galvanic's register-packing calling convention.

| Function | When called |
|---|---|
| `lower_struct_expr_into` | Return type is a named struct (FLS §9, §6.11) |
| `lower_enum_expr_into`   | Return type is a named enum (FLS §9, §15) |
| `lower_tuple_expr_into`  | Return type is a tuple (FLS §9, §6.10) |

Decision point: `lower_fn` inspects the function's declared return type and routes to one of the four paths. Adding a new expression case for a composite-returning function → add a match arm in the relevant `lower_*_expr_into`. Adding a new case for scalar-returning functions → add a match arm in `lower_expr`.

---

## FLS traceability convention

Every IR node (`Instr` variant, `IrTy` variant, significant `IrValue` usage) must carry a traceability comment:

```rust
// FLS §6.5.6 — Arithmetic expressions: add
Instr::BinOp { op: IrBinOp::Add, .. }
```

When the spec is ambiguous or silent:
```rust
// FLS §6.10: AMBIGUOUS — spec does not define the ABI for tuple returns
```

Missing traceability is a code smell. Every new IR variant should have the citation at its definition site in `ir.rs`.

---

## Cache-line discipline

ARM64 instructions are 4 bytes each; 16 instructions fill one 64-byte cache line.

- `Token` is exactly 8 bytes → 8 tokens per cache line (enforced by size assertion test).
- `Span` is exactly 8 bytes → same.
- Every IR type with a cache-line note in `ir.rs` must have a corresponding `assert_eq!(size_of::<T>(), N)` test. This is enforced by the `bench` CI job.
- When adding a new IR type, add the cache-line commentary AND the size assertion immediately — don't defer.

The codegen is cache-line-aware in instruction selection (minimize instruction count), not just in data layout. Assembly output comments document register choices and cache-line reasoning.

---

## No unsafe rule

`src/` contains zero `unsafe` blocks, `unsafe fn`, or `unsafe impl`. This is enforced by the `audit` CI job and by `cargo clippy`. If you think you need `unsafe`, you're wrong — find the safe Rust alternative.

---

## Adding a new language feature (checklist)

1. Find the FLS section. Check `refs/fls-ambiguities.md` for known gaps.
2. **New syntax?** → `src/ast.rs` (type), `src/parser.rs` (parse case).
3. **New runtime behavior?** → `src/ir.rs` (new `Instr` or `IrValue` variant with FLS citation and cache-line note + size test).
4. **Lowering** → `src/lower.rs` (translate AST node to IR using FLS semantic rule; error must name function, FLS section, and construct).
5. **Codegen** → `src/codegen.rs` (translate IR to ARM64; comment register usage and cache-line reasoning).
6. **Tests:**
   - Fixture: `tests/fixtures/fls_<section>_<topic>.rs`
   - Parse acceptance: `tests/fls_fixtures.rs`
   - Assembly inspection: `tests/e2e.rs` using `compile_to_asm` (mandatory for anything that could be const-folded)
   - Full e2e: `compile_and_run` in `tests/e2e.rs` when applicable

---

## Domain boundaries

This project spans three authority domains. Bugs are often misattributed between them.

| Domain | Covers | Authoritative source |
|--------|--------|---------------------|
| FLS (Ferrocene Language Specification) | Rust language semantics: what constructs are valid, how they evaluate, type rules, ownership rules | `refs/fls-pointer.md` → https://rust-lang.github.io/fls/ |
| ARM64 ISA / AAPCS64 | Instruction encoding, calling conventions, register allocation, stack layout, system call ABI | `refs/arm64-abi.md`; AAPCS64 spec |
| Platform ABI (Linux / macOS / BSDs) | Binary format (ELF vs Mach-O), syscall numbers, dynamic linking | `refs/arm64-platform-abi.md` |

**Common confusion:** "The spec doesn't say how to encode large integers" → that's a domain boundary moment: FLS is silent (language domain), but AAPCS64 + ARM64 ISA provides the answer (machine domain). Document the gap in `refs/fls-ambiguities.md` and cite the ARM64 source in the codegen.

**Another common confusion:** "Should this behavior differ on macOS vs Linux?" → that's a platform ABI question, not a language semantics question. The FLS covers language semantics; macOS/Linux differences live in the platform ABI domain.

---

## Partial output on lowering failure

When lowering fails for some functions but succeeds for others, galvanic emits assembly for the successful functions (inspection-only, no entry point) and exits non-zero. This is intentional — a partial success must not be silently discarded. The Lead Researcher needs the artifact to inspect even when the program can't be run.

Exit code semantics:
- `0` — fully successful compile with `fn main` and no errors
- `0` — no errors, no `fn main` (library-only file)
- `1` — any error (lower fail, codegen fail, missing file, etc.)
- Non-zero on any lowering error even when partial assembly was emitted
