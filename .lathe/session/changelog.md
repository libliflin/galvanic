# Goal — Cycle 023 (Customer Champion)

## Stakeholder: Cache-Line Performance Researcher

**Rotation:** Cycles 019 (Cache-Line), 020 (Compiler Contributor), 021 (Spec Researcher), 022 (Lead Researcher) → 023 = Cache-Line Researcher (4 cycles since last served, most under-served).

**Journey walked:**
1. README: cache-line claim prominent and clear ✓
2. `cargo bench`: throughput visible (670 MiB/s lexer, 192 MiB/s parser) ✓
3. Size tests: Token (8B), IrValue (8B), StaticValue (16B), Instr (80B, documented deviation) — all pass ✓
4. Compiled `fls_6_15_loop_expressions.rs`, opened `.s` file, searched for "cache" commentary → **hollow moment**
5. Traced Token claim end-to-end (README → lexer.rs → token_is_eight_bytes test → bench) ✓

**Hollow moment:** Loop label commentary says "cache-line: label has zero footprint" — trivially true, zero information about the loop body's cache footprint. The while_loop body is 18 instructions = 72 bytes, spanning 2 cache lines. No commentary anywhere reflects this. `codegen.rs` line 1023–1025 states the intention perfectly ("every loop header and back-edge is a cache-line-relevant boundary") but the emitted output says the opposite thing (zero footprint of the label, not the body).

**Goal set:** At back-edge branches (unconditional `Branch` whose target precedes it in the instruction array), emit the loop body instruction count and cache-line span: "back-edge — cache: loop body = N instr × 4 B = K B, spans M cache line(s)". This makes the loop body footprint verifiable from the assembly alone — the same standard the prologue commentary already meets.

---

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
