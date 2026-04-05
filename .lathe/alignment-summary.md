# Alignment Summary

Read this before starting cycles. It takes 30 seconds and will save you from cycles that don't matter.

---

## The approach: vertical slices, not horizontal layers

**Do not build the compiler phase-by-phase.** Build it program-by-program.

Pick the simplest Rust program galvanic can't yet compile to a running ARM64 binary. Make it work end-to-end. Then pick the next one. Each cycle should extend what galvanic can **actually compile and run**.

The front-end (lexer + parser) is already far ahead of the back-end. The next several cycles should be focused almost entirely on IR and codegen — not on adding more syntax to the parser.

---

## Who this serves

**William (you)** — Wants to see valid ARM64 binaries. "The parser accepts this" is not enough — "this compiles and runs correctly" is the bar. Cycles that widen the parser without extending codegen are not progress.

There are no external users, no library consumers, no downstream teams. This keeps the alignment simple.

---

## Key tensions

**Front-end breadth vs. pipeline depth**: The parser handles far more syntax than the rest of the pipeline can consume. Depth wins — extend codegen before widening the parser.

**Cache-line awareness vs. pragmatism**: Cache-line awareness should shape every data structure and codegen decision, but should not delay getting the pipeline working. Design thoughtfully, then implement.

**FLS fidelity vs. vertical progress**: Follow the FLS faithfully for each vertical slice — but don't detour into implementing every FLS section in a phase before moving to the next phase.

---

## Current focus

**Next milestone**: `fn main() -> i32 { 0 }` compiles to an ARM64 binary that exits with code 0.

To get there:
1. Add a minimal IR (just enough for "return integer from main")
2. Add ARM64 codegen (emit assembly, use external assembler/linker)
3. Add an end-to-end test (compile, run via QEMU on CI, check exit code)

---

## What could be wrong

- **ARM64 on CI**: CI runs on x86_64. Need `qemu-user` and `gcc-aarch64-linux-gnu` for cross-compilation and emulation. The CI workflow will need updating.
- **FLS version**: The FLS at spec.ferrocene.dev is the assumed reference. If working against a specific version, note it in `.lathe/refs/fls-pointer.md`.
- **Assembly strategy**: Starting with text assembly (`.s` files) + external assembler is pragmatic. A built-in assembler is future work.
- **Branch protection**: Verify that `main` requires PRs and status checks before merging.
