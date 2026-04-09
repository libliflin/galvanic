# Architecture — galvanic

This file answers: "What are the key design decisions in galvanic, and where do things live?" Read this when you're about to modify IR, lowering, or codegen, or when you're not sure which module owns a feature.

---

## Pipeline

```
source text
    ↓
lexer::tokenize()          → Vec<Token>                    src/lexer.rs
    ↓
parser::parse()            → ast::SourceFile               src/parser.rs
    ↓
lower::lower()             → ir::Module                    src/lower.rs
    ↓
codegen::emit_asm()        → String (GAS syntax)           src/codegen.rs
    ↓
aarch64-linux-gnu-as + ld  → ELF binary                    (main.rs, CLI only)
```

The CLI in `main.rs` drives the full pipeline. The library (`src/lib.rs`) exposes all four stages as public modules so tests can drive them independently.

---

## Module Responsibilities

### `lexer.rs` — Tokenization (FLS §2)

Produces a flat `Vec<Token>` from source text. Whitespace and comments are consumed but not emitted. The token stream is always terminated with `Token::Eof`.

**Critical layout constraint:** `Token` is **8 bytes** (`TokenKind: u8` + `start: u32` + `len: u16` + padding). This is enforced by the `token_is_eight_bytes` test and is the foundation of the cache-line rationale: 8 tokens per 64-byte cache line. Do not add fields to `Token` without re-examining this.

### `ast.rs` — AST Node Types (FLS §3–§22)

Passive data — no logic. All node types derive `Debug`. Recursive types use `Box<T>` (heap-allocated children, potential cache misses on deep traversal — noted as future arena work in the module doc).

`Span` is **8 bytes** (`start: u32` + `len: u32`). This is explicitly documented; a `span_is_eight_bytes` test would be the natural companion to the token test.

### `parser.rs` — Parsing (FLS §3–§22)

Consumes `Vec<Token>`, produces `ast::SourceFile`. Recursive descent. Returns `Result<SourceFile, ParseError>`. Fixture tests in `tests/fls_fixtures.rs` drive parse-acceptance without running the full pipeline.

### `lower.rs` — AST → IR (FLS §6–§18, §19)

**The most complex module.** Translates AST nodes to IR instructions. This is where the FLS constraint against const-folding non-const code lives. Read `.lathe/refs/fls-constraints.md` before modifying this file.

Key state during lowering:
- `stack_slots: u8` — grows as `let` bindings are encountered
- A slot-to-register map (`HashMap<String, u8>`) for local variables
- An enum variant registry (`EnumDefs`) built during item pre-scan
- A const item registry for `const` items

**The litmus test (from `fls-constraints.md`):** If replacing a literal with a function parameter would break your implementation, you've written an interpreter. Every arithmetic op, loop, and conditional in a non-const function must emit runtime IR.

### `ir.rs` — Intermediate Representation

Intentionally minimal and grows exactly as needed. Key types:

- `Module` — top-level compilation unit: functions, statics, trampolines, vtable shims, vtables
- `IrFn` — one function: name, ret_ty, body (flat `Vec<Instr>`), stack_slots, saves_lr, float const pools
- `Instr` — IR instruction enum (LoadImm, BinOp, Store, Load, Label, Branch, CondBranch, Call, Ret, ...)
- `IrValue` — register reference (`Reg(u8)`) or constant (`I32(i32)`, `Unit`, etc.)
- `IrTy` — type of an IR value (I32, I64, F32, F64, Bool, Unit, Ptr, ...)
- `StaticData`, `ClosureTrampoline`, `VtableShim`, `VtableSpec` — supporting structures

**Design rule:** Add to `Instr` exactly when the next milestone program requires it. No speculative additions.

### `codegen.rs` — ARM64 Assembly Emission (FLS §9, §6.19, §18.1)

Consumes `ir::Module`, writes GAS-syntax ARM64 assembly as a `String`. The emitted text is intended for `aarch64-linux-gnu-as`.

Target: AArch64 Linux ELF, bare `_start` entry point (no libc). Exit via `sys_exit` syscall (x8=93, x0=exit_code).

**Calling convention:** galvanic uses ARM64 AAPCS64 for inter-function calls. Structs are passed with one register per field. Closures use callee-saved registers (x19–x28) for captures.

---

## Cache-Line Design Philosophy

Every type on a hot iteration path documents its cache-line footprint. The pattern is:
1. Choose layout to minimize `size_of<T>` for hot types
2. Add a `# Cache-line note` section to the type's rustdoc
3. Add a `size_of` assertion test for the most critical types

Hot paths: lexer token stream, IR instruction list, codegen string buffer.
Not hot: error types, build-time registries, string names.

---

## FLS Citation Convention

Every implementation of a language feature carries a citation:
```rust
// FLS §6.5.5: Arithmetic operator expressions.
```

When the spec is ambiguous or silent:
```rust
// FLS §6.23: AMBIGUOUS — the spec requires a panic on integer overflow but
// does not specify the runtime mechanism. Galvanic does not yet insert the check.
```

Ambiguities are the primary research output of this project. Document them in code and in changelogs.

---

## Testing Architecture

See `.lathe/skills/testing.md` for the full testing picture. Quick orientation:

- `tests/smoke.rs` — CLI smoke test (single test)
- `tests/fls_fixtures.rs` — parse-acceptance tests for every fixture file
- `tests/fixtures/*.rs` — one file per FLS section with representative programs
- `tests/e2e.rs` — full-pipeline tests (lex → parse → lower → codegen → assemble → link → qemu-run)
- `benches/throughput.rs` — criterion benchmarks for throughput and data structure sizes

---

## What's Not Yet Implemented

The compiler emits `LowerError::Unsupported` or `CodegenError::Unsupported` for features that parse but aren't lowered/codegen'd yet. Common examples to look for in the snapshot's test output. Any `Unsupported` that matches the next FLS section in sequence is the natural target for the next milestone.
