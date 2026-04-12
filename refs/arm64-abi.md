# ARM64 ABI Reference

The compiler translates language semantics (FLS) into machine code via the ABI. Many things the FLS leaves unspecified are defined here. Each target platform layers its own conventions on top of the base ARM64 standard.

## Authoritative sources

### Base standard
- **AAPCS64** — [Procedure Call Standard for the Arm 64-bit Architecture](https://github.com/ARM-software/abi-aa/blob/main/aapcs64/aapcs64.rst) (ARM-software/abi-aa)
- **ELF for AArch64** — [ELF for the Arm 64-bit Architecture](https://github.com/ARM-software/abi-aa/blob/main/aaelf64/aaelf64.rst)
- **ARM ARM** — ARM Architecture Reference Manual (instruction set, encoding, behavior)

### Platform-specific
- **macOS** — [Writing ARM64 code for Apple platforms](https://developer.apple.com/documentation/xcode/writing-arm64-code-for-apple-platforms)
- **Linux** — follows AAPCS64 with SysV ELF extensions
- **FreeBSD** — follows AAPCS64 with ELF; see `sys/arm64/` in FreeBSD source
- **OpenBSD/NetBSD** — follow AAPCS64 with ELF
- **Syscall ABIs** — see `refs/arm64-platform-abi.md`

## Register conventions (AAPCS64 §6.1.1)

The base register table is shared across all platforms:

| Register | Role | Saved by |
|---|---|---|
| `x0`–`x7` | Arguments and return values | Caller |
| `x8` | Indirect result location register | Caller |
| `x9`–`x15` | Temporary (scratch) registers | Caller |
| `x16`–`x17` | Intra-procedure-call scratch (IP0/IP1) | Caller |
| `x18` | Platform register — **see platform differences below** | — |
| `x19`–`x28` | Callee-saved registers | Callee |
| `x29` | Frame pointer (FP) | Callee |
| `x30` | Link register (LR) — return address | Callee |
| `sp` | Stack pointer (must be 16-byte aligned) | — |
| `d0`–`d7` | Float/SIMD arguments and return values | Caller |
| `d8`–`d15` | Callee-saved float registers | Callee |
| `d16`–`d31` | Temporary float registers | Caller |

### Platform differences in register usage

| Register | macOS | Linux | FreeBSD | OpenBSD |
|---|---|---|---|---|
| `x18` | **Reserved** — used by OS for thread-local storage. Do not touch. | Available as temporary (caller-saved), but some distros reserve it | Reserved for platform use | Reserved for platform use |
| `x16`/`x17` | Used for PAC (pointer authentication). Linker veneers also use them | Linker veneers (IP0/IP1) | Linker veneers | Linker veneers |
| `x29` (FP) | **Required** — Apple mandates frame pointer in all functions | Optional (can be omitted with `-fomit-frame-pointer`) | Optional | Optional |

**Galvanic rule: do not use `x18`. Always emit frame pointer (`x29`).** This satisfies all platforms.

### What this means for galvanic

- Arguments go in `x0`–`x7` (integer) and `d0`–`d7` (float). More than 8 spill to the stack.
- Return value in `x0` (integer) or `d0` (float). Composites up to 16 bytes in `x0`/`x1`.
- `x9`–`x15` are free scratch — galvanic uses `x9` and `x10` as temporaries.
- `x16`/`x17` are scratch but may be clobbered by linker — galvanic uses `x17` for the MIN/-1 overflow guard constant (safe within a single function).
- `x18` — **never use**. Reserved on macOS and some other platforms.
- `x30` (LR) is set by `bl` — must be saved/restored if the function calls others.
- `sp` must remain 16-byte aligned at all times (AAPCS64 §5.2.2.2).

## Parameter passing (AAPCS64 §6.4)

This is consistent across all platforms — everyone follows AAPCS64 here.

**Fundamental types** (integers, floats, pointers up to 8 bytes):
- Passed in the next available register (`x0`–`x7` or `d0`–`d7`)
- Return in `x0` or `d0`

**Small composites** (structs/tuples up to 16 bytes, <=2 members):
- Passed in consecutive registers: `x0`/`x1` or `d0`/`d1`
- Returned in `x0`/`x1`

**Larger composites** (>16 bytes or >2 members):
- Caller allocates stack space, passes pointer in `x8`
- Callee writes result to `[x8]`

**macOS variation:** Stack-passed arguments are packed (no padding to 8-byte slots). Other platforms pad each stack argument to a multiple of 8 bytes. This only matters when >8 arguments overflow to the stack.

### FLS ambiguities resolved by ABI

| FLS "ambiguity" | ABI answer |
|---|---|
| §4.8/§4.9 — Fat pointer ABI (`&str`, `&[T]`) | Two-register composite: ptr in `x0`, len in `x1` |
| §6.10 — Tuple return convention | Small tuples (<=2 fields): `x0`/`x1`. Larger: via `x8` pointer |
| §10.1 — Method `self` parameter | First arg in `x0` like any other parameter |
| §4.13 — dyn Trait fat pointer return | Two-register composite: data in `x0`, vtable in `x1` |
| §6.22 — Closure captures | Implementation choice; captures as leading hidden parameters |
| Stack alignment | `sp` must be 16-byte aligned (AAPCS64 §5.2.2.2) — all platforms |

## Stack frame layout

AAPCS64 does not mandate a specific frame layout, but the standard pattern is:

```
[higher addresses]
  caller's frame
  ─────────────
  saved LR (x30)       ← [sp + frame_size - 8]   (if function calls others)
  saved FP (x29)       ← [sp + frame_size - 16]  (optional on Linux/BSDs, required on macOS)
  local slot N-1        ← [sp + (N-1)*8]
  ...
  local slot 1          ← [sp + 8]
  local slot 0          ← [sp]
[lower addresses]
```

Frame size must be a multiple of 16 (for `sp` alignment). Galvanic rounds up: `frame_size = ((slots * 8) + 15) & !15`.

**macOS requirement:** Frame pointer (`x29`) must always be saved and point to the previous frame record. This enables crash reporting and profiling tools. Galvanic should always emit frame pointer setup for cross-platform compatibility.

## Integer overflow and arithmetic

This is hardware behavior, identical across all platforms:

- ARM64 arithmetic operates on 64-bit registers. `add x0, x1, x2` produces a 64-bit result.
- For i32 semantics, the result must be checked against the 32-bit signed range.
- `sxtw xD, wD` sign-extends the low 32 bits to 64 bits — comparing `xD` with the sign-extended value detects i32 overflow.
- Hardware flags (`adds`/`subs` with condition codes) are another option but galvanic uses the `sxtw`/`cmp` approach.

## Binary format by platform

### ELF (Linux, FreeBSD, OpenBSD, NetBSD)

- Entry point: `_start` (no underscore prefix)
- `.text` section for code, `.data` for initialized data, `.bss` for zero-initialized
- `.globl _start` exports the entry point
- Static linking with platform-appropriate `ld`
- No libc, no C runtime — bare `_start` with direct syscalls

### Mach-O (macOS)

- Symbol prefix: `_` (e.g., `_start` in source becomes `__start` in the object file)
- Entry point: set via `ld -e _main` or similar
- Assembler: system `as` (Clang integrated assembler)
- Linker: system `ld`
- Pointer authentication (PAC) may affect function pointers on newer hardware — galvanic does not yet handle this

## Platform-specific considerations

### macOS
- **Pointer Authentication Codes (PAC):** Apple hardware signs return addresses and function pointers. For bare-metal-style programs this may not apply, but if galvanic ever links against system libraries, `paciasp`/`autiasp` instructions may be needed around function prologues/epilogues.
- **No red zone:** Unlike x86_64 macOS, ARM64 macOS has no red zone (area below `sp` that's safe from signal handlers). All local storage must be below the current `sp`.
- **W^X enforcement:** macOS enforces write-xor-execute on memory pages. Cannot generate code at runtime without special entitlements.

### Linux
- Follows standard AAPCS64 without significant deviations.
- `x18` is technically available but best avoided for portability.

### BSDs
- Follow AAPCS64 closely.
- `x18` reserved on OpenBSD; best avoided on all BSDs.
- ELF format, same toolchain as Linux cross-compilation.
