# Architecture — Galvanic

Key architectural decisions visible in the codebase. For the builder and verifier.

---

## Pipeline

Galvanic is a linear six-stage compiler pipeline:

```
Source text
  → Lexer (src/lexer.rs)        — produces Vec<Token>
  → Parser (src/parser.rs)      — produces SourceFile (AST, src/ast.rs)
  → Lower (src/lower.rs)        — produces Module (IR, src/ir.rs)
  → Codegen (src/codegen.rs)    — produces ARM64 assembly text
  → Assembler (aarch64-linux-gnu-as) — produces .o
  → Linker (aarch64-linux-gnu-ld)   — produces ELF binary
```

The last two stages (assemble/link) are invoked from `src/main.rs` using subprocess commands. The library (`src/lib.rs`) only covers lex through codegen — the CLI driver in `main.rs` is the only place that shells out.

**Why this matters for the builder:** Never add subprocess invocations (`std::process::Command`) to any file other than `src/main.rs`. The audit CI job enforces this.

---

## IR Design Philosophy

The IR (`src/ir.rs`) is intentionally minimal. Nothing is added before it is needed by the next runnable binary. The comment at the top of `ir.rs` says: "The design will grow with each milestone program." This is not debt — it is the design.

Every new IR node must earn its place by being needed by an actual source program that galvanic should compile. Do not add speculative IR nodes.

---

## Cache-Line Awareness

Cache-line alignment is a first-class design constraint, not an optimization pass. Every public type in the IR carries a cache-line note explaining how it fits (or will fit) in 64-byte cache lines. The `Token` type is enforced to be exactly 8 bytes (tested in `lexer::tests::token_is_eight_bytes`).

When adding new IR nodes or AST nodes, add a cache-line note. When an existing note says "will be revisited when the instruction set grows," that means: if your change makes the type larger, document the tradeoff.

---

## FLS Traceability

Every implementation decision cites the FLS section it implements:

```rust
// FLS §9: Functions — each IrFn maps to one source-level function.
// FLS §6.1.2:37–45 — const-folding is forbidden in runtime contexts.
// FLS §X.Y: AMBIGUOUS — <describe what the spec doesn't say>
```

This is not optional decoration. The research value of galvanic comes from these citations. When a section is ambiguous, the `AMBIGUOUS` annotation and `refs/fls-ambiguities.md` entry are the primary outputs.

---

## No Const-Folding in Runtime Contexts

The hardest correctness constraint: FLS §6.1.2:37–45 forbids evaluating runtime code at compile time, even when all values are statically known. A regular `fn` body is not a const context. Galvanic must emit runtime instructions even for `fn main() -> i32 { 1 + 2 }`.

Assembly inspection tests enforce this:
```rust
let asm = compile_to_asm("fn main() -> i32 { 1 + 2 }\n");
assert!(asm.contains("add"), "expected add instruction");
assert!(!asm.contains("mov     x0, #3"), "must not constant-fold");
```

**For the builder:** When implementing arithmetic, control flow, or any expression that has a constant value in a test case, write an assembly inspection test — not just an exit-code test.

---

## Test Tiers

Four test tiers, each with a different scope:

| Tier | File | What it tests |
|---|---|---|
| Unit | `src/*.rs` (inline `#[test]`) | Data structure properties (token size, span size) |
| Smoke | `tests/smoke.rs` | Parser/lexer on minimal inputs |
| Fixture | `tests/fls_fixtures.rs` | Parse acceptance of FLS-derived programs |
| E2E | `tests/e2e.rs` | Full pipeline: lex → parse → lower → codegen → assemble → run |

E2E tests have two modes:
- **Assembly inspection**: call `compile_to_asm()`, assert on instruction presence/absence
- **Run-and-check**: call `compile_and_run()`, assert on exit code

Always prefer assembly inspection for any test involving arithmetic, comparisons, or control flow. Exit-code tests are appropriate only when the runtime behavior (not the instruction form) is what matters.

---

## Compilation Thread Stack

`src/main.rs` spawns the entire compilation pipeline on a thread with a 64 MB stack (matching rustc's own budget). This prevents stack overflows in the recursive-descent parser from killing the process with a signal. The fuzz-smoke CI job tests this with deeply nested inputs.

---

## Platform Targeting

Galvanic emits **Linux ARM64 ELF binaries**. Even on macOS (Apple Silicon), the output is Linux ELF — the e2e tests use `qemu-aarch64` to run the result. The assembler and linker are `aarch64-linux-gnu-as` / `aarch64-linux-gnu-ld` (GNU cross tools).

The platform ABI differences (macOS vs. Linux vs. BSD syscall numbers, entry point conventions) are documented in `refs/arm64-platform-abi.md`.

---

## No Unsafe Code

The library (`src/`) has no unsafe blocks. The audit CI job enforces this with a grep. Do not add `unsafe` blocks.

---

## No Network Dependencies

Cargo.toml has no networking crates. The audit CI job checks this. The compiler has no runtime network access.
