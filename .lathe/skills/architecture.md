# Architecture

The galvanic compiler pipeline, module by module. Read this before adding a new feature.

---

## Pipeline

```
source text
    │
    ▼
lexer::tokenize()      →  Vec<Token>
    │
    ▼
parser::parse()        →  SourceFile (AST)
    │
    ▼
lower::lower()         →  Module (IR)
    │
    ▼
codegen::emit_asm()    →  String (ARM64 assembly text)
    │
    ▼
aarch64-linux-gnu-as   →  .o (object file)
    │
    ▼
aarch64-linux-gnu-ld   →  ELF binary (Linux ARM64)
```

Each stage has a single job and a clean boundary. Nothing earlier in the pipeline knows about later stages. The IR is the contract between language semantics (lower.rs) and machine instructions (codegen.rs).

---

## Module responsibilities

### `src/lexer.rs` — Tokenizer

Converts source text into `Vec<Token>`. Whitespace and comments are consumed, not emitted. The token stream ends with `TokenKind::Eof`.

Implements FLS §2 (Lexical Elements). Each `Token` is 8 bytes — `TokenKind` as `repr(u8)` plus a `Span` — so 8 tokens fit in one 64-byte cache line. This is enforced by a size test.

Does not know about grammar, expressions, or types.

### `src/ast.rs` — AST types

Type definitions for the Abstract Syntax Tree produced by the parser. Contains no logic — just types. An `Item` is a top-level declaration (function, struct, enum, impl, trait, static, const, type alias). An `Expr` is an expression. A `Pat` is a pattern.

### `src/parser.rs` — Parser

Converts `Vec<Token>` into a `SourceFile` (AST). Implements recursive descent. Has a `MAX_BLOCK_DEPTH` limit to produce clean errors (not stack overflows) on adversarial deeply-nested inputs.

Implements the grammar implied by FLS §5 (Patterns), §6 (Expressions), §7–§14 (Items), §18 (Crates and source files).

Does not know about types, values, or machine instructions.

### `src/ir.rs` — Intermediate Representation

Defines `Module`, `IrFn`, `Instr`, `IrValue`, `IrTy`, and related types. This is the bridge between language semantics and machine instructions.

Every IR node has an FLS traceability comment naming the spec section it implements. Cache-line notes explain the size and layout of each type.

The IR is intentionally minimal: only what the next milestone needs is added. There is no "design for the future" — new IR nodes are added when a feature is implemented, not speculatively.

Key types:
- `Module` — the compilation unit (one source file → one module). Contains `fns`, `statics`, `trampolines`, `vtable_shims`.
- `IrFn` — one function: its name, parameter types, return type, and a `Vec<Instr>`.
- `Instr` — one IR instruction (e.g., `Ret`, `BinOp`, `Call`, `Branch`).
- `IrValue` — a constant value or a reference to a local variable.
- `IrTy` — a type (i32, f64, bool, Unit, Ptr, etc.).

### `src/lower.rs` — AST → IR

Translates language semantics into the IR. This is where the FLS rules live: when to evaluate at compile time (const contexts only), how to lower match expressions, how to handle closures and trampolines, how to lower `impl Trait` dispatch.

When lowering fails (unsupported construct), it emits an error naming the failing function, the FLS section, and the specific construct. Partial failure is supported: if some functions lower successfully, the module is returned with those functions and the errors are collected.

Does not know about ARM64. Does not emit assembly.

### `src/codegen.rs` — IR → ARM64 assembly

Translates an IR `Module` into GNU assembler (GAS) syntax for `aarch64-linux-gnu-as`. Emits Linux ELF calling conventions: syscall via `svc #0` with syscall number in `x8`, arguments in `x0`–`x5`, return value in `x0`.

Cache-line reasoning lives here: how many instructions fit in a cache line, where `.align` directives are needed, how to lay out `_start` and function prologues for minimal cache pressure.

Does not know about Rust semantics or the FLS. If a question is "what does this Rust construct mean," that question belongs in lower.rs, not here.

### `src/main.rs` — CLI driver

Parses arguments, runs the pipeline, and calls the assembler/linker when `-o` is given. This is the only module that shells out to external processes (`Command`). CI enforces this via the `audit` job.

Runs the pipeline in a thread with a 64 MB stack to prevent stack overflows on deeply-nested inputs from becoming signal deaths.

---

## Adding a new language feature

1. **Identify the FLS section.** Find the relevant section in the FLS (`refs/.lathe/refs/fls-pointer.md` has the full TOC). Read it carefully. Note any ambiguities for `refs/fls-ambiguities.md`.

2. **Add AST nodes if needed.** If the feature requires new syntax, add types to `ast.rs` and a parser case to `parser.rs`.

3. **Add an IR node if needed.** If the feature produces new runtime behavior, add a new `Instr` variant or `IrValue` variant to `ir.rs`. Add an FLS traceability comment. Add a cache-line note.

4. **Add a lowering case.** Add a match arm in `lower.rs` that translates the new AST construct to the new IR node. The lowering case encodes the FLS semantic rule.

5. **Add a codegen case.** Add a match arm in `codegen.rs` that translates the new IR node to ARM64 instructions. Comment the register usage and cache-line reasoning.

6. **Write tests.**
   - Fixture in `tests/fixtures/fls_<section>_<topic>.rs`.
   - Parse acceptance test in `tests/fls_fixtures.rs`.
   - Assembly inspection test in `tests/e2e.rs` asserting runtime instructions are emitted.
   - If the feature adds a cache-critical type, add a size assertion in the relevant module's `#[cfg(test)]` block.

---

## Invariants

- **No unsafe code.** Enforced by CI `audit` job: `grep -rn 'unsafe'` in `src/` must return empty.
- **No `Command` in library code.** Only `src/main.rs` may shell out. Enforced by CI.
- **No networking dependencies.** The compiler must have no runtime network access.
- **Every IR node traces to an FLS section.** New IR nodes without FLS comments break the traceability chain.
- **Const evaluation only in const contexts.** FLS Constraint 1. Enforced by assembly inspection tests.
- **Cache-line-critical types have size tests.** New types in modules with cache-line commentary need corresponding `assert_eq!(size_of::<T>(), N)` tests.
