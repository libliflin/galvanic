# Architecture

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

Each stage has one job and a clean boundary. Nothing earlier in the pipeline knows about later stages. The IR is the contract between `lower` and `codegen`.

`src/main.rs` is the only module that shells out to external processes (assembler, linker). The library (everything else) is pure Rust with no `unsafe` and no network deps.

## Key Invariants (enforced by CI)

- **No unsafe code** anywhere in `src/`. The `audit` job enforces this.
- **No `Command` in library code.** Only `src/main.rs` may shell out.
- **Every IR node traces to an FLS section.** Format: `// FLS §X.Y — <description>`.
- **Cache-line-critical types have size tests.** Types in `lexer` and `ir` with cache-line commentary must have `assert_eq!(size_of::<T>(), N)`.
- **No const folding in non-const contexts.** A regular `fn` body must emit runtime instructions even when all values are statically known. Assembly inspection tests in `tests/e2e.rs` enforce this.
- **Every "not yet supported" error cites an FLS section.** `lower_source_all_unsupported_strings_cite_fls` in `tests/smoke.rs` enforces this statically.

## Module Roles

| Module | Role | Size |
|--------|------|------|
| `lexer` | Source text → `Vec<Token>`. Token is 8 bytes (8 per cache line). | ~500 lines |
| `ast` | AST type definitions — no logic, just types. | ~1,700 lines |
| `parser` | `Vec<Token>` → `SourceFile`. Recursive descent. | ~3,800 lines |
| `ir` | IR type definitions. Every node traces to FLS. | ~1,200 lines |
| `lower` | AST → IR. All FLS semantic rules. | ~18,000 lines (the engine) |
| `codegen` | IR → ARM64 GAS. Cache-line discipline lives here. | ~1,600 lines |

## Error Message Architecture

When `lower()` fails, galvanic prints:
1. One line per failing function: `error: lower failed in '<name>': not yet supported: <construct> (FLS §X.Y)`
2. A summary line: `lowered N of M functions (K failed)` (only when fn_count > 0)
3. If some functions succeeded, emit their assembly anyway (partial output, non-zero exit)

This design serves the Lead Researcher: they see the full error landscape in one run, and partial output is never silently discarded.

## Partial Output Paths

Three distinct outcomes when `fn main` is involved:
- **All functions succeed + main present:** emit `.s`, exit 0
- **Some functions fail + main succeeds:** emit `.s` (partial, with "partial" in stdout), exit 1
- **Main fails + some other functions succeed:** emit `.s` annotated "inspection-only" (no `_start`), exit 1
- **Main fails + no other functions succeed:** no `.s`, exit 1

## Ambiguity Registry

`refs/fls-ambiguities.md` is the Spec Researcher's primary artifact. Each entry:
- Names the FLS section and the specific gap
- Documents galvanic's chosen resolution (what galvanic does and why)
- Provides a minimal reproducer with a specific assembly signature the researcher can verify
- Must be sorted by FLS section number with a navigable TOC

AMBIGUOUS annotations in source (`// AMBIGUOUS: §X.Y`) are the source of truth; the registry aggregates them.

## Platform Targets

- ARM64 only for codegen output (macOS, Linux, BSDs)
- The compiler itself runs on any Rust-supported platform
- Assembly targets Linux ELF format by default; macOS and BSD variants are noted in `refs/arm64-platform-abi.md`
- Cross-compilation toolchain: `aarch64-linux-gnu-as` + `aarch64-linux-gnu-ld`; emulation via `qemu-aarch64` for testing on non-ARM64 hosts
