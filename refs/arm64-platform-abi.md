# ARM64 Platform ABI — macOS vs Linux

Galvanic targets ARM64 on both macOS and Linux. The instruction set is identical; only the OS interface differs.

## Syscall convention

| | macOS ARM64 | Linux ARM64 |
|---|---|---|
| **Instruction** | `svc #0x80` | `svc #0` |
| **Syscall number register** | `x16` | `x8` |
| **Exit syscall number** | `1` | `93` (`__NR_exit`) |
| **Exit example** | `mov x16, #1; mov x0, #42; svc #0x80` | `mov x8, #93; mov x0, #42; svc #0` |

A binary compiled for one platform **will not work on the other**. CPU instructions (add, sub, mul, branches, loads, stores) are identical; only the OS interface differs.

## Binary format

| | macOS ARM64 | Linux ARM64 |
|---|---|---|
| **Format** | Mach-O | ELF |
| **Assembler** | `as` (system, Clang-based) | `aarch64-linux-gnu-as` (GNU) |
| **Linker** | `ld` (system) | `aarch64-linux-gnu-ld` (GNU) |
| **Symbol prefix** | `_` (e.g., `_start`, `_main`) | none (e.g., `_start`, `main`) |
| **Entry point** | `_main` or set via `-e` | `_start` |

## Assembly syntax differences

GAS syntax is used on both platforms, with minor differences:

- **Section directives**: Linux uses `.section .text`; macOS may use `.text` directly
- **Global symbols**: Both use `.globl`, but macOS prefixes symbols with `_`
- **Alignment**: `.align` semantics may differ (power-of-2 vs byte count)
- **Comment syntax**: Both support `//` line comments in ARM64 mode

## Current implementation status

**Currently implemented: Linux ARM64 only.** The codegen (`src/codegen.rs`) emits:
- Linux syscalls (`svc #0` with `x8 = 93` for exit)
- ELF-targeted assembly (no Mach-O symbol prefix)
- `_galvanic_panic` trampoline using Linux exit(101)

**macOS ARM64 support is needed.** The codegen should be extended to:
- Detect or accept a target platform flag
- Emit the correct syscall sequence per platform
- Use the correct symbol naming convention
- The `_start` / `_galvanic_panic` trampoline needs platform-specific variants

## Known traps

### Negative literal i32::MIN

The parser treats `-2147483648` as unary negation of `2147483648`. Since `2147483648 > i32::MAX`, the lowering stage rejects it. To test with i32::MIN, use `(-2147483647 - 1)` to construct it at runtime.

### Overflow guards and untyped IR

The IR (`src/ir.rs`) `BinOp` has no type annotation. `IrBinOp::Add` is used for user-visible i32 addition AND for pointer/index arithmetic. A guard meant for i32 overflow will also fire on legitimate address calculations that produce values > 2^31.
