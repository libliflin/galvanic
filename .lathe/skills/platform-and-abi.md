# Platform & ABI — ARM64 macOS and Linux

## Galvanic targets ARM64 on both macOS and Linux

The ARM64 instruction set is identical across both platforms. The differences are in how the program talks to the OS and how the binary is packaged.

## Platform ABI differences

### Syscall convention

| | macOS ARM64 | Linux ARM64 |
|---|---|---|
| **Instruction** | `svc #0x80` | `svc #0` |
| **Syscall number register** | `x16` | `x8` |
| **Exit syscall number** | `1` | `93` (`__NR_exit`) |
| **Exit example** | `mov x16, #1; mov x0, #42; svc #0x80` | `mov x8, #93; mov x0, #42; svc #0` |

A binary compiled for one platform **will not work on the other** — the syscall instruction and register convention are completely different. The CPU instructions (add, sub, mul, branches, loads, stores) are identical; only the OS interface differs.

### Binary format

| | macOS ARM64 | Linux ARM64 |
|---|---|---|
| **Format** | Mach-O | ELF |
| **Assembler** | `as` (system, Clang-based) | `aarch64-linux-gnu-as` (GNU) |
| **Linker** | `ld` (system) | `aarch64-linux-gnu-ld` (GNU) |
| **Symbol prefix** | `_` (e.g., `_start`, `_main`) | none (e.g., `_start`, `main`) |
| **Entry point** | `_main` or set via `-e` | `_start` |

### Assembly syntax differences

GAS (GNU Assembler) syntax is used on both platforms, but with minor differences:

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

## Testing by platform

### macOS (Apple Silicon) — local development

- `compile_to_asm()` assembly inspection tests: **work today** (pure Rust, no external tools)
- `compile_and_run()` runtime tests: **require macOS codegen support** (not yet implemented)
  - Once macOS codegen is implemented, these tests will run natively — no QEMU needed
  - Currently these tests skip because the Linux cross-tools are not available on macOS

### Linux x86_64 — CI environment

- Assembly inspection tests: work
- Runtime tests: work via `qemu-aarch64` (user-mode Linux syscall emulation)
- Cross-tools installed: `binutils-aarch64-linux-gnu`, `qemu-user`

### Linux ARM64 — CI or native

- Everything runs natively, no QEMU needed

## Key traps for developers

### Skipped tests look like passing tests

On macOS (until macOS codegen is implemented), `compile_and_run()` returns `None` and tests early-return. The Rust test harness reports these as "passed." **Hundreds of runtime tests appear green but never executed.** Only assembly inspection tests truly ran.

### Overflow guards affect ALL operations of an IR type

The IR (`src/ir.rs`) `BinOp` has no type annotation. `IrBinOp::Add` is used for:
- User-visible i32 addition (`a + b`)
- Pointer/index arithmetic (array offset calculations)
- Loop counter increments

A guard that panics on i32 overflow will also fire on legitimate pointer math that produces values > 2^31. You won't see this on macOS (tests skip) — only CI catches it.

### Negative literal i32::MIN

The parser treats `-2147483648` as unary negation of `2147483648`. Since `2147483648 > i32::MAX`, the lowering stage rejects it. To test with i32::MIN:
- Use `(-2147483647 - 1)` to construct it at runtime
- Or decompose into separate operations

## CI environment

The e2e CI job runs on `ubuntu-latest`:
```yaml
sudo apt-get install -y binutils-aarch64-linux-gnu qemu-user
```

A macOS CI job should also be added (using `macos-latest` which provides Apple Silicon) once macOS codegen support is implemented. This would test native execution without any emulation layer.
