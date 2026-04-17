# Changelog — Cycle 001, Round 1

## Goal
Emit partial assembly when lowering partially succeeds: carry the successfully-lowered
functions in `LowerErrors` so `main.rs` can write a `.s` file for the working subset
even when some functions fail. Exit code stays non-zero; errors still print.

## Who This Helps
- **Stakeholder:** Lead Researcher
- **Impact:** Running `cargo run -- fls_6_18_match_expressions.rs` now produces a 456-line
  `.s` file covering the 12 functions that lower successfully, instead of producing nothing.
  The researcher can inspect real runtime assembly for §6.18 match patterns immediately.
  Every parse-only fixture with one unsupported construct is now inspectable for all the
  constructs that do work.

## Applied

### `src/ir.rs`
- Added `#[derive(Debug)]` to `Module`, `StaticData`, `IrFn`, `Instr`, `IrTy`,
  `VtableShim`, `VtableSpec`, `ClosureTrampoline`.  Required so `LowerErrors` (which
  holds `Option<Module>`) can derive `Debug`. No structural changes.

### `src/lower.rs`
- Added `partial_module: Option<crate::ir::Module>` field to `LowerErrors`.
  Populated with `Some(Module { fns, … })` when at least one function lowered
  successfully; `None` when every function failed.
- At the error-return site, package the already-lowered `fns`, `static_data`,
  `trampolines`, `vtable_shims`, and `vtables` into the partial module.

### `src/main.rs`
- In the `Err(errs)` arm of the lower match: after printing errors and the summary
  line, check `errs.partial_module`. If it is `Some` and contains `fn main`, run
  `codegen::emit_asm` on it and write the `.s` file. Print a diagnostic message
  noting partial emission. Exit code remains 1.

## Validated

**Command:**
```
cargo run -- tests/fixtures/fls_6_18_match_expressions.rs
```

**Output:**
```
galvanic: compiling fls_6_18_match_expressions.rs
parsed 16 item(s)
error: lower failed in 'match_tuple': not yet supported: ...
lowered 12 of 13 functions (1 failed)
galvanic: emitting partial assembly for 12 succeeded function(s)
galvanic: partial assembly written to tests/fixtures/fls_6_18_match_expressions.s
```

456 lines of runtime ARM64 assembly (cmp/cset/cbz — not constant-folded).

**Where the verifier should look:**
- Run the command above; confirm the `.s` file is written and is non-empty.
- `head -20 tests/fixtures/fls_6_18_match_expressions.s` — should show `cmp`/`cset`/`cbz`.
- `cargo test` — 2051 passed, 0 failed.
