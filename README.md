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

Galvanic targets **ARM64** on both **macOS (Apple Silicon)** and **Linux**. The instruction set is identical across platforms; only the syscall ABI and binary format differ. See `refs/arm64-platform-abi.md` for the full comparison.

- **macOS**: local development and testing on Apple Silicon, also tested on CI
- **Linux**: tested on CI (x86_64 with QEMU, or native ARM64)

## Building

```
cargo build
cargo test
```

## License

MIT
