# galvanic

A clean-room ARM64 Rust compiler built from the [Ferrocene Language Specification](https://spec.ferrocene.dev/).

## What this is

Galvanic implements core Rust (`no_std`) by reading the FLS, and its codegen is obsessively cache-line-aware. It exists to answer two questions:

1. **Is the FLS actually implementable by an independent party?** The spec claims to be a complete description of Rust. We're testing that claim by building a compiler from it without looking at `rustc` internals.

2. **What happens when a compiler treats cache-line alignment as a first-class concern in every decision?** Not as an optimization pass bolted on at the end, but as a constraint woven into layout, register allocation, and instruction selection from the start.

## What this is not

This is not a production compiler. It's a sacrificial anode — it exists to find ambiguities in the spec and to explore what "dumb but cache-aware" codegen can do. Nobody needs to use this. Value comes from what we learn.

Do not use this to compile anything you care about.

## Platform targets

Galvanic targets **ARM64** and supports two platform ABIs:

| | macOS (Apple Silicon) | Linux ARM64 |
|---|---|---|
| **Binary format** | Mach-O | ELF |
| **Syscall ABI** | `svc #0x80`, number in `x16` | `svc #0`, number in `x8` |
| **Assembler** | `as` (system) | `aarch64-linux-gnu-as` |
| **Linker** | `ld` (system) | `aarch64-linux-gnu-ld` |
| **Local testing** | Native on Apple Silicon | Native on ARM64, QEMU on x86_64 |
| **CI** | `macos-latest` runner | `ubuntu-latest` + cross-tools + QEMU |

The ARM64 instruction set (add, sub, mul, ldr, str, branches, etc.) is identical across both platforms. The differences are in binary format, entry point conventions, and how the program talks to the OS (syscalls).

## Building

```
cargo build
cargo test
```

### macOS (Apple Silicon)

Local development environment. Tests that compile and run ARM64 binaries execute natively — no emulation needed.

### Linux

CI environment. On x86_64 Linux, ARM64 binaries are assembled with the cross-toolchain and executed via `qemu-aarch64` (user-mode emulation). On ARM64 Linux, binaries run natively.

```bash
# Install cross-tools (Debian/Ubuntu x86_64):
sudo apt-get install binutils-aarch64-linux-gnu qemu-user
```

## License

MIT
