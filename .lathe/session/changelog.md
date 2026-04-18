# Verification — Cycle 022, Round 2 (Verifier)

## What I compared

- **Goal:** When `main` fails but other functions succeed, emit partial assembly annotated "inspection-only — no fn main; this assembly has no entry point." Exit code stays non-zero. Zero assembly only when zero functions lowered.
- **Builder's change (Round 1):** `src/codegen.rs` — `emit_asm_inspection_only`; `src/main.rs` — split the "no fn main" path. **Round 1 verifier additions:** extended `partial_lower_no_main_emits_inspection_assembly` smoke test with `_start`-absent assertion and ≥20 function-label count; added `compile_to_asm_inspection_only` helper and `inspection_only_asm_has_annotation_and_no_start` e2e test.

**What I ran:**
- `cargo test` — 2104 pass, 0 fail ✓
- `cargo clippy -- -D warnings` — clean ✓
- `cargo run -- tests/fixtures/fls_5_patterns.rs`:
  - stdout: `galvanic: emitted tests/fixtures/fls_5_patterns.s (inspection-only — no fn main; this assembly has no entry point)` ✓
  - exit: 1 ✓
- Verified annotation on line 1 of `.s`, no `_start`, exactly 20 function labels ✓
- Traced the boundary case manually: main-only-fails → `partial_module = None` → `return 1` at main.rs:108 → no assembly, no spurious message ✓
- Examined lower.rs:2604–2615: `partial_module = None` when `fns.is_empty()`, confirming invariant ✓
- Verified `no_main_prints_lowered_note` (clean library file) still passes ✓

## What's here, what was asked

**Gap found — zero-function boundary case was untested.**

The Round 1 verifier noted in its findings: "The 'main fails, zero other functions lowered' case produces the pre-existing message with exit 1. This is correct per the goal but untested."

Tracing the code path confirms the invariant: lower.rs:2605 sets `partial_module = None` when `fns.is_empty()`, so main.rs:108 (`None => return 1`) exits before any emit path. The code path at main.rs:143 (`return if had_lower_errors { 1 } else { 0 }`) is structurally unreachable with `had_lower_errors = true` — a correct invariant, but worth documenting via a test that would catch any future regression that accidentally emits assembly or the inspection-only message in this case.

## What I added

**`tests/smoke.rs`** — Added `main_only_fails_emits_no_assembly`:
- Source: struct defs + `fn main` that fails on a nested struct field variable (not a literal)
- Asserts: non-zero exit, no `.s` file created, no "inspection-only" message in stdout
- Closes the boundary-case gap the Round 1 verifier flagged

Full suite: **2105 pass, 0 fail** (up from 2104). Clippy clean.

- **Files:** `tests/smoke.rs`

## Notes for the goal-setter

- The `had_lower_errors && module.fns.is_empty()` branch inside the "no fn main" block (main.rs:118) is structurally dead for `had_lower_errors = true` because `partial_module = Some` implies `fns` is non-empty (lower.rs:2605 invariant). The defensive guard is harmless, but the comment on line 142 ("Exit non-zero if lower errors occurred, even when there's no fn main") describes a path that can't be reached when errors occurred. Low urgency — no correctness risk.
- None other.
