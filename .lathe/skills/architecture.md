# Architecture — Galvanic

## Pipeline

```
source text
  → lexer::tokenize()    → Vec<Token>
  → parser::parse()      → ast::SourceFile
  → lower::lower()       → ir::Module
  → codegen::emit_asm()  → ARM64 assembly text (GAS syntax)
  → aarch64-linux-gnu-as → .o object file
  → aarch64-linux-gnu-ld → ELF binary
  → qemu-aarch64 / native ARM64 → runs
```

Each stage is a separate module with a clean public API. The binary (`src/main.rs`) drives the pipeline and shells out to the assembler/linker. The library (`src/lib.rs`) exposes the Rust-only stages (lex, parse, lower, codegen) for testing.

## Key constraint: no compile-time evaluation in non-const functions

This is the project's central integrity check. See `refs/fls-constraints.md` for the full rationale. Short version: galvanic is a compiler, not an interpreter. Every regular function body must emit runtime ARM64 instructions. Constant folding of non-const code is forbidden, even when it would produce the correct exit code.

The litmus test: replace every literal in a function with a parameter. If the implementation breaks, it was evaluating at compile time.

## Module responsibilities

### `src/lexer.rs` — FLS §2
Tokenizes source text. Emits only meaningful tokens (no whitespace/comments). Terminates with `TokenKind::Eof`.

**Cache-line layout is enforced:** `Token` is 8 bytes (`repr(u8)` discriminant, 24-bit span). This is locked by `lexer::tests::token_is_eight_bytes`. Do not add fields to `Token` or change `TokenKind` to `repr(u16)` without explicit discussion of the cache-line tradeoff.

`Span` encodes `(start: u32, end: u32)` as a pair of 32-bit byte offsets into the source string.

### `src/parser.rs` — FLS §3–§6
Recursive-descent parser. One method per grammar rule. Returns `ParseError` on failure with the offending span. Has a `MAX_BLOCK_DEPTH` limit (200) to prevent stack overflow on adversarial input.

Operator precedence is encoded in the call graph (not a Pratt parser). 13 levels from assignment (lowest) to primary (highest). See module doc for the full table.

### `src/ast.rs`
The AST node types. `SourceFile` is the root. `Item` covers top-level declarations. `Expr`/`ExprKind` covers all expression forms. No semantic information — just structure.

### `src/ir.rs`
A minimal, flat IR for ARM64 codegen. `Module` contains `Vec<IrFn>`. `IrFn` contains `Vec<Instr>`. Instructions are explicit stack operations (stack slots indexed by integer), loads/stores, arithmetic, branches, and calls. No SSA, no phi nodes — this is a simple 1:1 codegen target.

### `src/lower.rs` — FLS §6–§9
Translates `SourceFile` → `Module`. Each `FnDef` becomes an `IrFn`. Local variables are allocated stack slots (tracked by name in a `HashMap<String, usize>`). The lowering pass is the core of the compiler — every FLS feature that produces runtime behavior has to be implemented here.

**FLS citation discipline:** every `lower_*` function should cite the FLS section it implements. Ambiguities go as `// FLS §X.Y: AMBIGUOUS — <description>` inline.

### `src/codegen.rs` — FLS §18.1
Translates `Module` → ARM64 GAS assembly text. Linear traversal of `Vec<Instr>`. Bare `_start` entry point (no libc).

**Dual-platform target:** Galvanic targets both macOS ARM64 and Linux ARM64. The ARM64 instructions are identical; only the syscall ABI and binary format differ. See `.lathe/skills/platform-and-abi.md` for the full comparison. Currently only Linux is implemented (syscall via `svc #0`, number in `x8`). macOS support (`svc #0x80`, number in `x16`) is needed.

## ARM64 calling convention (as implemented)

Arguments: x0–x{n-1} (first n args). Callee spills to stack immediately. Return value: x0. Caller saves x0 before placing args if needed. This is a simplified (non-ABI-conformant) convention sufficient for internal calls — not the full AAPCS64.

## The "claims" methodology

Each FLS section implemented follows the "Claim" pattern:
- "Claim 4k: add while-let runtime falsification for FLS §6.15.4"
- One claim = one FLS section = parse fixture + e2e exit-code test + assembly inspection test

The assembly inspection test is the proof that the claim is true at runtime, not just "the exit code happened to be right."

## What the project can compile today (known as of recent commits)

Fully through the pipeline (e2e + assembly inspection):
- fn main with i32 / unit return (§9, §18.1)
- Integer literals, boolean literals (§2.4.4.1, §2.4.7)
- Arithmetic: +, -, * (§6.5.5)
- Let bindings (§8.1)
- If/else (§6.17)
- Function calls with parameters (§6.12.1, §9)
- Mutable assignment (§6.5.10)
- While, while-let, loop, break, continue (§6.15.2–§6.15.4, §6.15.6–§6.15.7)
- Match expressions (§6.18)
- Struct expressions (§6.11)
- Path expressions / named blocks (§6.3, §6.4.3)

Parse only (parse fixture, no codegen):
- Closures (§6.14), for loops (§6.15.1), range expressions (§6.16)
- Generics (§12), traits (§13), impl blocks, associated items (§10)
- Patterns (§5), type aliases (§4.10), const/static items (§7)
- Let-else (§8.1), dyn trait (§4.13), unsafe (§19), slices (§4.9)

The gap between "parse fixture" and "e2e codegen" is where future claims live.

## Benches

`benches/throughput.rs` — criterion benchmark measuring compilation throughput (tokens/second or similar). Run with `cargo bench`. CI runs with short warm-up/measurement to catch regressions.

## Build requirements

- Rust stable (edition 2024)
- For e2e runtime tests on Linux: `binutils-aarch64-linux-gnu`, `qemu-user`
- For e2e runtime tests on macOS: native `as` and `ld` (once macOS codegen is implemented)
- `cargo build` and `cargo test --lib` work everywhere; assembly inspection tests work everywhere

The binary shells out to platform-appropriate assembler and linker when given `-o output`. No other external dependencies.

## Platform targets

Galvanic targets ARM64 on both macOS and Linux. The instruction set is identical; only the syscall ABI and binary format differ. See `.lathe/skills/platform-and-abi.md` for the full comparison.

**Current state:** Only Linux ARM64 codegen is implemented. macOS codegen (different syscall convention, Mach-O format) is needed so that developers on Apple Silicon can run the full test suite locally. Until then, runtime e2e tests skip on macOS and only CI (Linux) executes them.
