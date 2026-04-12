# ARM64 Platform ABI Reference

Galvanic targets ARM64 across macOS, Linux, and the BSDs. The instruction set is identical; only the OS interface differs.

## Syscall convention

| | macOS | Linux | FreeBSD / OpenBSD / NetBSD |
|---|---|---|---|
| **Instruction** | `svc #0x80` | `svc #0` | `svc #0` |
| **Syscall number register** | `x16` | `x8` | `x8` |
| **Exit syscall number** | `1` | `93` | `1` |
| **Exit example** | `mov x16, #1; mov x0, #42; svc #0x80` | `mov x8, #93; mov x0, #42; svc #0` | `mov x8, #1; mov x0, #42; svc #0` |

The BSDs and Linux share the same ARM64 syscall instruction (`svc #0`) and register (`x8`). They differ in syscall numbers. macOS is the outlier — different instruction (`svc #0x80`) and register (`x16`).

A binary compiled for one platform **will not work on another** without recompiling the syscall sequences.

## Binary format

| | macOS | Linux | FreeBSD / OpenBSD / NetBSD |
|---|---|---|---|
| **Format** | Mach-O | ELF | ELF |
| **Assembler** | `as` (system, Clang) | `aarch64-linux-gnu-as` (GNU) | `as` (system, Clang or GNU) |
| **Linker** | `ld` (system) | `aarch64-linux-gnu-ld` (GNU) | `ld` (system) |
| **Symbol prefix** | `_` (e.g., `_start`) | none | none |
| **Entry point** | `_main` or set via `-e` | `_start` | `_start` |

## Assembly syntax

GAS syntax works across all platforms, with minor differences:

- **macOS** prefixes global symbols with `_`; everyone else does not
- **Section directives**: Linux uses `.section .text`; macOS/BSDs may use `.text` directly
- **Alignment**: `.align` semantics may differ (power-of-2 vs byte count)

## Current implementation status

**Implemented: Linux ARM64 only.** The codegen (`src/codegen.rs`) emits:
- Linux syscalls (`svc #0` with `x8 = 93` for exit)
- ELF-targeted assembly
- `_galvanic_panic` trampoline using Linux exit(101)

**Needed:** Platform-aware codegen that emits the correct syscall sequence per target. The platform differences are small — they're confined to `_start`, `_galvanic_panic`, and any future syscall sites. The codegen should:
- Accept a target platform flag (or detect from host)
- Emit the correct syscall instruction and register per platform
- Use the correct symbol naming convention (underscore prefix on macOS)

## Known traps

### Negative literal i32::MIN

The parser treats `-2147483648` as unary negation of `2147483648`. Since `2147483648 > i32::MAX`, the lowering stage rejects it. Use `(-2147483647 - 1)` to construct it at runtime.

### Overflow guards and untyped IR

The IR (`src/ir.rs`) `BinOp` has no type annotation. `IrBinOp::Add` is used for user-visible i32 addition AND for pointer/index arithmetic. A guard meant for i32 overflow will also fire on legitimate address calculations that produce values > 2^31.
