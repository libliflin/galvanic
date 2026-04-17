# Changelog — Cycle 001

## Stakeholder: Lead Researcher

**Rotation rationale:** The last 4+ goals served Compiler Contributor (×3) and Spec Researcher
(×1). The Lead Researcher — the person this compiler is built for — hadn't been served in 5+
cycles. Today's cycle was theirs.

## Journey walked

- Confirmed floor: `cargo test` — 2051 passed, 0 failed, build clean.
- Picked `fls_6_18_match_expressions.rs` as the most substantive parse-only fixture (13 functions
  covering §6.18 comprehensively: literal patterns, guards, boolean scrutinee, or-patterns, enum
  variants, match-in-let, tuple scrutinee, range patterns, nested match).
- Ran `cargo run -- tests/fixtures/fls_6_18_match_expressions.rs`.
- Result: 12 of 13 functions lowered; `match_tuple` failed (tuple expression as scrutinee is
  not yet supported).
- No `.s` file was produced.
- Verified separately that simple match expressions emit correct runtime assembly (cmp/cset/cbz
  branches — not constant-folded, ABI-correct).

## What I found

12 of 13 §6.18 functions compile successfully. The emitted assembly for those 12 is correct
runtime code. The one failure (`match_tuple`) is caused by a separate, unimplemented feature
(tuple expression in value context), not a match expression bug.

But the researcher cannot see any of this: `LowerErrors` carries no partial output, so 
`main.rs` emits nothing when any function fails.

## Goal set

**Emit partial assembly when lowering partially succeeds.**

When `lower()` returns partial results (some functions succeed, some fail), carry the
successfully-lowered `fns` in `LowerErrors` (or an equivalent partial-success return type)
so `main.rs` can emit a `.s` file for the successful functions. Exit code stays non-zero.
Error messages still print. But the artifact is produced.

This eliminates the whole class of "partial success produces no output" — every parse-only
fixture that has one unsupported construct will immediately become inspectable for all the
constructs that do work.

## Next cycle candidates

- The tuple-scrutinee match (`match (x, y) { (0, 0) => ... }`) is a natural next feature to
  unblock `fls_6_18_match_expressions.rs` completely.
- `fls_9_functions.rs` and `fls_2_4_literals.rs` are also parse-only and likely in the same
  partial-lowering situation.
- Once partial output is emitted, the researcher can document FLS findings from match pattern
  assembly — particularly the `AMBIGUOUS` question of whether wildcard pattern lowering order
  is specified by §6.18.
