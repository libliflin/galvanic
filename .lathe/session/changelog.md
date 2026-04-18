# Verification — Cycle 022, Round 1 (Verifier)

## What I compared

- **Goal:** When `main` fails but other functions succeed, emit partial assembly annotated "inspection-only — no fn main; this assembly has no entry point." Exit code stays non-zero. Zero assembly only when zero functions lowered.
- **Builder's change:** `src/codegen.rs` — refactored `emit_asm` into `emit_asm_impl(module, include_entrypoint: bool)`; added `emit_asm_inspection_only(module)` calling `emit_asm_impl(module, false)`. `src/main.rs` — split the "no fn main" path: `had_lower_errors && !module.fns.is_empty()` → call `emit_asm_inspection_only`, write `.s`, exit 1. `tests/smoke.rs` — one new smoke test `partial_lower_no_main_emits_inspection_assembly`.

**What I ran:**
- `cargo test` — 2103 pass, 0 fail ✓
- `cargo clippy -- -D warnings` — clean ✓
- `cargo run -- tests/fixtures/fls_5_patterns.rs`:
  - stderr: `error: lower failed in 'main': not yet supported: expected struct literal \`Inner { .. }\` for nested struct field (FLS §6.11, §5.10.2)`
  - stderr: `lowered 20 of 21 functions (1 failed)` ✓
  - stdout: `galvanic: emitted tests/fixtures/fls_5_patterns.s (inspection-only — no fn main; this assembly has no entry point)` ✓
  - exit code: 1 ✓
- `head -5 tests/fixtures/fls_5_patterns.s` — annotation on first line ✓
- `grep "_start" tests/fixtures/fls_5_patterns.s` — empty output ✓ (no `_start`)
- `grep "^_galvanic_panic:" tests/fixtures/fls_5_patterns.s` — present ✓ (panic handler emitted because functions use arithmetic)
- Verified existing `fls_9_functions.rs` partial-success path still emits `(partial — some functions failed)` — no regression ✓

**Boundary case reviewed:**

When `had_lower_errors && module.fns.is_empty()` (main is the only function and it failed): the code falls through to `"lowered 0 function(s) — no fn main, no assembly emitted"` with `exit 1`. Correct per goal: "The only case where zero assembly is emitted is when zero functions lowered."

## What's here, what was asked

**Gap found — two missing assertions in the smoke test:**

The builder's smoke test verifies the annotation comment, stdout message, and that the `.s` file exists. It does NOT assert:
1. That `_start` is absent from the emitted assembly (the critical correctness guarantee — a researcher who assembles this output should not get a corrupted entry point)
2. That function bodies are actually present (the file could be just the annotation + `.text` header with zero functions)

**Gap found — no direct unit test of `emit_asm_inspection_only`:**

The smoke test goes through the CLI. `emit_asm_inspection_only` is a public API with no test calling it directly via `LowerErrors::partial_module`. A test at this level confirms the function contract independently of CLI argument parsing.

## What I added

**`tests/smoke.rs`** — Extended `partial_lower_no_main_emits_inspection_assembly` with two additional assertions:
- `!asm.contains("_start")` — inspection-only output must never contain `_start`
- Count function labels (lines ending `:`, non-`.`, non-`//`, not `_galvanic_panic`) — asserts ≥20 function bodies present, confirming successful lowerings were not discarded

**`tests/e2e.rs`** — Added cycle 022 section with:
- `compile_to_asm_inspection_only(source)` helper: calls `lower()`, extracts `LowerErrors::partial_module`, calls `emit_asm_inspection_only` directly
- `inspection_only_asm_has_annotation_and_no_start`: uses a minimal fixture (structs `Inner`/`Outer`, helpers `add` and `helper`, `main` that fails on non-literal nested struct field) to exercise the inspection-only path directly; asserts annotation comment present, `_start` absent, helper function labels present

Full suite after additions: **2104 pass, 0 fail** (up from 2103). Clippy clean.

- **Files:** `tests/e2e.rs`, `tests/smoke.rs`
- **PR:** libliflin/galvanic#440

## Notes for the goal-setter

- The "main fails, zero other functions lowered" case (`had_lower_errors && module.fns.is_empty()`) produces the pre-existing "no fn main, no assembly emitted" message with exit 1. This is correct per the goal but untested. If a future cycle wants to distinguish "main failed with nothing else to show" from "library file with no errors," a test and possibly a distinct message would serve the Lead Researcher well.
- None other.
