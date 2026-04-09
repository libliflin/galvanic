# Galvanic Architecture

This skill exists to answer: "How does galvanic work, and where does a new feature go?"

---

## Pipeline

```
Source (.rs) → lexer::tokenize() → Vec<Token>
                                       ↓
                            parser::parse() → SourceFile (AST)
                                       ↓
                            lower::lower() → ir::Module
                                       ↓
                          codegen::emit_asm() → String (GAS assembly)
                                       ↓
                         aarch64-linux-gnu-as → .o
                                       ↓
                         aarch64-linux-gnu-ld → ELF binary
                                       ↓
                              qemu-aarch64 → runs
```

All five stages are in `src/`. The CLI in `src/main.rs` drives the pipeline and optionally invokes the assembler/linker.

---

## Modules

### `src/lexer.rs` — Tokenization (FLS §2)
- Entry point: `tokenize(source: &str) -> Result<Vec<Token>, String>`
- `Token` is exactly 8 bytes: `TokenKind` (repr(u8), 1 byte discriminant), span offset (u32, 4 bytes), span length (u16, 2 bytes), padding (1 byte). Cache-line layout: 8 tokens per 64-byte line.
- Whitespace and comments are consumed but not emitted.
- The `TokenKind` enum has `repr(u8)`. If it grows past 255 variants, `repr(u8)` becomes invalid. Currently well under that limit.

### `src/ast.rs` — Abstract Syntax Tree
- Represents parsed Rust source: `SourceFile`, items, expressions, statements, types.
- Not designed for the full Rust grammar — grows by exactly what each new milestone needs.
- Each AST node carries a source span (byte offset + length) for error reporting.

### `src/parser.rs` — Parsing (FLS §3–§19 selectively)
- Entry point: `parse(tokens: &[Token], source: &str) -> Result<SourceFile, String>`
- Recursive descent. No parser generator.
- Implements only the subset of Rust grammar needed for implemented milestones.

### `src/ir.rs` — Intermediate Representation
- Sits between AST and ARM64 codegen.
- Key types: `Module` (compilation unit), `IrFn` (function), `Instr` (flat instruction list), `IrValue` (constant values), `IrTy` (types).
- Also contains: `StaticData`, `ClosureTrampoline`, `VtableShim`, `VtableSpec`.
- **Every public type has a cache-line note** — this is CLAIM-3 and the project's primary research artifact.
- The IR is intentionally minimal: no SSA, no basic blocks (yet). Instructions are a flat `Vec<Instr>` per function. Labels and branches exist as instructions, not a CFG.
- Virtual registers map 1:1 to ARM64 registers at this stage. `u8` register indices. Register allocation is trivial (sequential).

### `src/lower.rs` — AST → IR lowering
- Entry point: `lower(source_file: &SourceFile, source: &str) -> Result<Module, String>`
- Walks the AST and emits IR instructions.
- Tracks a local register counter and stack-slot counter per function.
- Each `let` binding gets a stack slot; each expression gets a virtual register.

### `src/codegen.rs` — IR → ARM64 assembly
- Entry point: `emit_asm(module: &Module) -> Result<String, CodegenError>`
- Emits GNU assembler (GAS) syntax for `aarch64-linux-gnu-as`.
- Target: AArch64 Linux ELF, bare `_start` entry point (no libc).
- System call convention: syscall number in `x8`, args in `x0`–`x5`.
- Each `IrFn` emits a labeled function body. `_start` calls `main` and exits via `svc #0`.

### `src/main.rs` — CLI driver
- The only file allowed to use `std::process::Command` (to invoke `aarch64-linux-gnu-as` and `aarch64-linux-gnu-ld`).
- Usage: `galvanic <source.rs> [-o <output>]`
- Without `-o`: emits `<source>.s` next to the source file.
- With `-o`: assembles and links to a standalone binary.

---

## Key design decisions

### Milestone-driven IR growth
The IR and codegen grow by exactly the instructions needed for each milestone program. No instruction is added until a milestone program needs it. This is not a limitation — it's the research methodology. The `Instr` enum comment documents which milestone added each variant.

### Cache-line as first-class concern
Every type addition requires a cache-line note. The note documents: size in bytes, how many fit in a 64-byte line, and the cache-line tradeoff (e.g., "stack slots map to 8-byte chunks — one slot per half cache-line entry"). The notes are not aspirational — they reflect the actual footprint.

### FLS traceability
Every implementation decision cites the FLS section it implements (`FLS §X.Y`). Ambiguities are marked `FLS §X.Y AMBIGUOUS:` with a note on what's unclear. This is the primary output of the research.

### No unsafe in library code
The library is safe Rust throughout. `src/main.rs` invokes external tools (assembler, linker) and may use `Command`, but the compiler library itself stays safe.

### Flat instruction list (no CFG)
The IR uses a flat `Vec<Instr>` with explicit `Label`, `Branch`, and `CondBranch` instructions rather than a basic-block CFG. This is simpler for the current milestone scope and intentional: the research question about cache-line codegen doesn't require SSA or a CFG.

---

## Adding a new IR feature (checklist)

1. Add the `Instr` variant (or new type) to `src/ir.rs` with:
   - A doc comment explaining what it does and its FLS citation
   - A `Cache-line note:` comment documenting its size/footprint
2. Update `src/lower.rs` to emit the new instruction when the AST has the relevant construct
3. Update `src/codegen.rs` to emit the ARM64 assembly for the new instruction
4. Add a fixture program to `tests/fixtures/fls_X_Y_description.rs`
5. Add a parse-acceptance test to `tests/fls_fixtures.rs`
6. If the feature is e2e compilable, add it to `tests/e2e.rs`
7. Run `cargo test` and `cargo clippy -- -D warnings`
8. Run `.lathe/falsify.sh` to verify no claims broke
