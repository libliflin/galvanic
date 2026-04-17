# Architecture — Galvanic

## What galvanic is

A clean-room ARM64 Rust compiler built strictly from the Ferrocene Language Specification (FLS), targeting `no_std` Rust. Two research questions drive every design decision:

1. Is the FLS actually implementable by an independent party?
2. What happens when cache-line alignment is a first-class design constraint — not an optimization pass, but a constraint woven into layout, register allocation, and instruction selection from the start?

This is not a production compiler. It's a research instrument. Value comes from what is learned about the spec and about cache-aware codegen.

---

## Pipeline

Source `.rs` file → **lexer** → **parser** → **AST** → **lower** → **IR** → **codegen** → ARM64 GAS assembly → (assembler + linker → binary)

Each stage lives in its own file:

| File | Stage | FLS anchor |
|---|---|---|
| `src/lexer.rs` | Tokenization | §2: Lexical Elements |
| `src/parser.rs` | Parse tokens → AST | §4–§18: Grammar |
| `src/ast.rs` | AST node types | §4–§18 |
| `src/lower.rs` | AST → IR | §6, §8, §9, §15, … |
| `src/ir.rs` | IR node types | Design doc (no direct FLS section) |
| `src/codegen.rs` | IR → ARM64 GAS | ABI + §18.1 |
| `src/main.rs` | CLI driver | — |

The library (`src/lib.rs`) exports all pipeline modules. `main.rs` is the CLI wrapper only — it parses arguments and invokes the library. The library must never shell out (`std::process::Command` is forbidden in library code).

---

## Key design constraints

### Safe Rust only

No `unsafe` blocks, no `unsafe fn`, no `unsafe impl`. CI enforces this. The compiler is safe Rust end-to-end — this is a deliberate constraint to keep the implementation trustworthy and auditable.

### No constant folding in non-const contexts

FLS §6.1.2 is explicit: compile-time evaluation is only permitted in const contexts (`const` items, `const fn` when called from a const context, `const { }` blocks, `static` initializers, etc.). A regular `fn main()` body is not a const context, even if every value is statically known. Galvanic must emit runtime instructions for all non-const code.

The litmus test: if you could replace a literal with a function parameter and the compiler would break (by emitting the wrong constant), it's a constant-fold bug.

This constraint is enforced by assembly inspection tests that check the actual emitted instructions, not just exit codes.

### Cache-line awareness

Every data structure in `ir.rs`, `ast.rs`, and `lexer.rs` documents its cache-line layout. The `Token` type is exactly 8 bytes (enforced by a size assertion test). The `Span` type is exactly 8 bytes. `Instr` and `IrValue` are small enums designed to fit in one cache line per instruction.

When adding a new IR node or AST type, document its size and cache-line impact in a `// Cache-line note:` comment. This is not optional — it's how the research question is answered.

### FLS traceability

Every module, type, and non-trivial function has `// FLS §N.M: ...` citations in comments. Ambiguities discovered during implementation are marked `// AMBIGUOUS: §N.M — ...` in source and documented in `refs/fls-ambiguities.md`. Constraints on what the compiler must not do are documented in `refs/fls-constraints.md`.

---

## IR design

The IR is intentionally minimal. Nothing is added until it is needed by the next runnable program milestone. This prevents premature abstraction and keeps the FLS mapping clear.

Key IR types (in `src/ir.rs`):
- `Module` — the compilation unit (one per source file)
- `IrFn` — a function, with a name, parameters, and a list of `Instr`
- `Instr` — an instruction (load immediate, binary op, return, branch, call, etc.)
- `IrValue` — a value (register slot, immediate, unit)
- `IrTy` — a type (i32, u32, f32, f64, bool, unit, ptr, etc.)

The IR does not have SSA form — it uses stack slots (spill-everything model). This is deliberate: SSA would require a separate pass, and the research value is in the FLS mapping, not in register allocation sophistication.

---

## Codegen target

Output is ARM64 GAS (GNU Assembler) syntax:
- Architecture: AArch64
- ABI: AAPCS64 (same register conventions on macOS, Linux, and BSDs)
- Binary format: Linux ELF with bare `_start` entry point (Linux syscalls via `svc #0`)
- The output `.s` file is assembled with `aarch64-linux-gnu-as` and linked with `aarch64-linux-gnu-ld`

**Important:** The emitted binary uses Linux syscalls and Linux ELF format. It **cannot** run on macOS, even on Apple Silicon, because macOS uses Mach-O format and a different syscall ABI. On macOS, assembly and runtime tests are skipped. CI (ubuntu-latest with `qemu-aarch64`) is the authoritative runtime test environment.

For platform ABI differences (macOS vs Linux vs BSDs), see `refs/arm64-platform-abi.md`.

---

## Adding a new language feature

The pattern to follow, in order:

1. **AST node** (`src/ast.rs`): Add the new expression, statement, or item type. Document the FLS section. Note cache-line size if it's a frequently-traversed node.

2. **Lexer** (`src/lexer.rs`): Add any new tokens the feature requires. Document the FLS section.

3. **Parser** (`src/parser.rs`): Add the parser case that recognizes the new construct and builds the AST node. Document the FLS section.

4. **Lowering** (`src/lower.rs`): Add the AST-to-IR translation. Emit runtime instructions (not constant-folded results). Document FLS citations. If the spec is silent on something, add an `AMBIGUOUS` annotation.

5. **IR** (`src/ir.rs`): Add any new instruction or type the lowering needs. Document cache-line layout.

6. **Codegen** (`src/codegen.rs`): Add the IR-to-assembly translation. Document the ABI register usage and any cache-line impact.

7. **Tests**: Parse acceptance test in `fls_fixtures.rs`, assembly inspection test in `e2e.rs`, and a runtime test in `e2e.rs` (skipped when cross-toolchain is absent).

---

## Trampoline and vtable shim functions

When closures are passed as `impl Fn` arguments, galvanic generates **trampoline functions** in `Module::trampolines`. A trampoline bridges the captured-variable calling convention (arguments spread across extra registers) to the `impl Fn` calling convention. See `src/ir.rs` `ClosureTrampoline` for the design.

When `dyn Trait` dispatch is used, galvanic generates **vtable shim functions** in `Module::vtable_shims`. A shim adapts the "fields in registers" calling convention used by galvanic's struct methods to the "data pointer in x0" convention expected by vtable callers. Each shim loads the struct's fields from a stack-allocated pointer and tail-calls the concrete method.

These are generated during lowering and emitted as regular assembly functions during codegen.

---

## Stack layout

Galvanic uses a spill-everything model: every local variable is assigned a stack slot in the function prologue. Arguments are spilled from their parameter registers (`x0`..`x7` for integers, `d0`..`d7` for floats) to the stack immediately on entry. This is conservative but keeps the IR simple and the FLS mapping clear.

Frame setup: `sub sp, sp, #N` where N is computed at lowering time. Frame teardown: `add sp, sp, #N` before `ret`.

---

## Benchmark suite

`benches/throughput.rs` (Criterion) measures compile throughput on representative programs. CI runs benchmarks and reports timing, and checks that data structure sizes haven't grown (via size assertion tests in `src/lexer.rs`).
