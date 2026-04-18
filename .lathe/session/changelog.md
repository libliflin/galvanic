# Verification — Cycle 022, Round 3 (Verifier)

## What I compared

- **Goal:** When `main` fails but other functions succeed, emit partial assembly annotated "inspection-only — no fn main; this assembly has no entry point." Exit code stays non-zero. Zero assembly only when zero functions lowered.
- **Builder's change (Round 2):** Added `main_only_fails_emits_no_assembly` smoke test — closes the boundary-case gap Round 1 verifier flagged: main-only-fails → `partial_module = None` → no .s file, no inspection-only message, exit 1.

**What I ran:**
- `cargo test` — 2105 pass, 0 fail ✓
- `cargo clippy -- -D warnings` — clean ✓
- `cargo run -- tests/fixtures/fls_5_patterns.rs` → inspection-only path, exit 1 ✓
- `cargo run -- /tmp/boundary_test.rs` (source matching the new test) → "lowered 0 of 1 functions (1 failed)", no .s file, no inspection-only message in stdout, exit 1 ✓
- Confirmed no `/tmp/boundary_test.s` created ✓

**Boundary case verified manually:**
The builder's test source (`Outer { inner: x }` where `x` is a variable) genuinely triggers "not yet supported: expected struct literal `Inner { .. }` for nested struct field (FLS §6.11, §5.10.2)" — the exact same error class as the goal fixture's failing `main`. This is not a synthetic failure; it exercises the real lower.rs code path.

## What's here, what was asked

Matches: the work holds up against the goal from my lens.

All four goal requirements are covered by tests:
1. Main fails, others succeed → inspection-only assembly emitted, exit 1 — tested by `partial_lower_no_main_emits_inspection_assembly` (smoke) and `inspection_only_asm_has_annotation_and_no_start` (e2e) ✓
2. `_start` absent from inspection-only output — asserted in the smoke test extension ✓
3. Function bodies present (≥20 labels) — asserted in the smoke test extension ✓
4. Main-only fails → zero assembly, no inspection-only message, exit 1 — tested by `main_only_fails_emits_no_assembly` (this round) ✓

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

- The `had_lower_errors && module.fns.is_empty()` branch at main.rs:118 inside the "no fn main" block is structurally dead when `had_lower_errors = true`, because `partial_module = Some(...)` implies `fns` is non-empty (lower.rs invariant at line 2605). This is harmless, but the defensive guard could be removed in a cleanup cycle without changing behavior.
- None other.
