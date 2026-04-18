# Changelog — Cycle 015

## Stakeholder

Cache-Line Performance Researcher. Last four cycles served Lead Researcher, Spec Researcher, Lead Researcher, Compiler Contributor — the cache-line researcher has been absent from every visible cycle.

## What I experienced

I became the Cache-Line Performance Researcher walking their first-encounter journey:

1. Read the README: finds the claim that "codegen is obsessively cache-line-aware" — credible opening.
2. Ran `cargo bench`: throughput reported in MiB/s, benchmarks cover lexer, parser, and end-to-end — the perf claim has numbers behind it.
3. Looked for size assertion tests: `token_is_eight_bytes` in `lexer.rs` passes. `span_is_eight_bytes` in `ast.rs` passes. Both claims backed by CI-enforced tests.
4. Opened `src/ir.rs` to trace the same chain: finds detailed cache-line notes on every major type — "Instr and IrValue fit comfortably in a single cache line," "StaticValue fits in 16 bytes," notes on every enum variant. Then grepped for `#[cfg(test)]`: **zero results**. Zero test blocks in `ir.rs`, despite it being the most heavily annotated module.

**The hollowest moment:** The architecture doc (`architecture.md`) lists "Cache-line-critical types have size tests" as an invariant. The lexer and AST modules honor it. The IR module — the one that's been growing fastest across every recent cycle — has been silently violating it since it was written.

## Goal set

Add `#[cfg(test)]` size assertion tests to `src/ir.rs` for `Instr`, `IrValue`, and `StaticValue` — the three types whose module-level cache-line notes make concrete size claims — making those claims enforceable by CI.

The model is already in the codebase: `token_is_eight_bytes`. Apply it to the IR module. This makes the wrong state (size claims without tests) structurally impossible in the most-changed module in the codebase.
