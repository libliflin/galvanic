# Architecture

*Why this exists: the runtime agent needs to understand the pipeline and major design decisions to make good implementation choices. This is not a language reference — it's what you can't derive from a quick read of the code.*

---

## Pipeline

```
source text
    → lexer::tokenize()        → Vec<Token>           [src/lexer.rs]
    → parser::parse()          → SourceFile (AST)     [src/parser.rs, src/ast.rs]
    → lower::lower()           → Module (IR)          [src/lower.rs, src/ir.rs]
    → codegen::emit_asm()      → String (GAS text)    [src/codegen.rs]
    → aarch64-linux-gnu-as     → .o
    → aarch64-linux-gnu-ld     → ELF binary
```

The CLI driver is `src/main.rs`. The library is `src/lib.rs` (pub mod for all phases). The library has no runtime dependencies — `tempfile` and `criterion` are dev-only.

---

## Calling Convention (ABI)

Galvanic uses a flat register ABI: each scalar field of a struct or tuple is passed in a separate register (`x0`, `x1`, ...). This differs from the C ABI (pointer to struct in `x0`) and was chosen because it maps cleanly to the FLS value semantics.

Consequences:
- Struct returns: fields come back in `x0`, `x1`, ...
- `&self` methods: the struct's fields arrive in `x0..x{n-1}`, not a pointer.
- `dyn Trait` vtable shims in `ir.rs::VtableShim` bridge between "pointer to struct in `x0`" (vtable call convention) and galvanic's flat field registers.
- Closures with captures: captured values are placed in ARM64 callee-saved registers (`x27`, `x26`, ...) before a `bl` to the closure body. The `ClosureTrampoline` in `Module` handles the bridge for `impl Fn` callbacks.

---

## IR Design

`src/ir.rs` contains:
- `Module` — top-level compilation unit (functions, statics, trampolines, vtable shims, vtables)
- `IrFn` — a function: name, parameters, return type, instruction list, stack slot count
- `Instr` — one instruction (enum: `LoadImm`, `BinOp`, `Store`, `Load`, `Ret`, `Branch`, `Call`, etc.)
- `IrValue` — a value reference (register, stack slot, immediate)

The IR is minimal and grows milestone by milestone. Nothing is added until the next milestone needs it.

**Cache-line design:** Every type in `ir.rs` carries a cache-line note documenting its size and how many fit per 64-byte line. These notes are currently aspirational for the IR (the instruction set is growing) but structural for `Token` (8 bytes, enforced by test).

**Stack layout:** Each function allocates a fixed stack frame at entry (`sub sp, sp, #N`) large enough for all local variables. Stack slots are assigned by index (slot 0, slot 1, ...) during lowering.

---

## Const vs. Runtime Constraint

This is the most critical architectural constraint. See `.lathe/refs/fls-constraints.md` in full.

The summary: a regular `fn` body (including `fn main`) is NOT a const context. Even if every value is a compile-time constant, the compiler must emit runtime ARM64 instructions. The litmus test: if swapping a literal for a function parameter would break the implementation, it's an interpreter, not a compiler.

**Assembly inspection tests** exist in `tests/e2e.rs` (starting around line 396) specifically to catch const-fold violations. When adding a new arithmetic or control-flow feature, add both an exit-code test AND an assembly inspection test.

---

## Testing Layers

Three distinct test layers. Keep them separate — mixing them hides what's actually been implemented:

1. **`tests/smoke.rs`** — CLI smoke test. Runs the binary binary, checks exit codes and stdout. No library internals.

2. **`tests/fls_fixtures.rs`** — Parse acceptance. Calls `lexer::tokenize` + `parser::parse` on fixture files in `tests/fixtures/`. A passing test here means "galvanic can lex and parse this FLS construct" — NOT that it can compile or run it.

3. **`tests/e2e.rs`** — Full pipeline. Calls the whole pipeline (lex → parse → lower → codegen) and either:
   - Uses `compile_to_asm()` to inspect the emitted ARM64 assembly (no external tools, works on macOS)
   - Uses `compile_and_run()` to assemble, link, and execute the binary via QEMU (requires `aarch64-linux-gnu-as`, `aarch64-linux-gnu-ld`, `qemu-aarch64`; tests self-skip on macOS)

---

## Fixture Files

`tests/fixtures/` contains `.rs` files and paired `.s` files (expected assembly for some). The `.rs` files are real Rust programs drawn from FLS examples. The `.s` files are reference assembly outputs for specific milestones.

Fixture file naming: `fls_X_Y_description.rs` where `X.Y` is the FLS section number.

---

## CI Structure

Five CI jobs (`.github/workflows/ci.yml`):

- **`build`**: `cargo build` + `cargo test` + `cargo clippy -- -D warnings`. Runs on every push/PR.
- **`fuzz-smoke`**: Adversarial CLI inputs (large inputs, NUL bytes, binary garbage, deeply nested braces). Tests that the compiler doesn't panic, hang, or crash on malformed input.
- **`audit`**: Grep-based invariant checks. No `unsafe` blocks in library code. No `std::process::Command` in library code (only `main.rs` may shell out). No networking crates.
- **`e2e`**: Installs `binutils-aarch64-linux-gnu` + `qemu-user` on `ubuntu-latest`, runs `cargo test --test e2e`. Full-pipeline tests including actual ARM64 execution.
- **`bench`**: `cargo bench` + the `token_is_eight_bytes` size check.

The `e2e` and `fuzz-smoke` jobs depend on `build` (run in parallel after build passes).

---

## What "One Milestone" Looks Like

A typical new milestone adds:
1. IR support in `src/ir.rs`: new `Instr` variant(s) or `IrValue` variant(s)
2. Lowering in `src/lower.rs`: code to emit the new instruction(s) for the corresponding AST nodes
3. Codegen in `src/codegen.rs`: ARM64 assembly emission for the new instruction(s)
4. Tests in `tests/e2e.rs`: exit-code test(s) + assembly inspection test(s) (for any new arithmetic/comparison operations)
5. FLS citations in all of the above: `FLS §X.Y` in comments and doc comments

If a milestone introduces a new type with a performance claim, a `size_of` test or static assertion may also be warranted.
