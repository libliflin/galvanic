# Changelog — Cycle 017, Round 3 (Builder)

## Goal
Fix the stale §4.9 entry so a Spec Researcher reads a clean, citable finding.
The verifier fixed `LoadIndexed` in `codegen.rs` (Round 2). This round
addresses the parallel defect in `StoreIndexed`.

## Who This Helps
- Stakeholder: Spec Researcher, Compiler Contributor
- Impact: The `StoreIndexed` AMBIGUOUS annotation now matches the `LoadIndexed`
  pattern across both `ir.rs` and `codegen.rs`. A Compiler Contributor reading
  either file sees consistent resolution statements — no second contradiction
  to stumble over.

## Applied
The `LoadIndexed` case in `ir.rs` was fixed (builder Round 1) and `codegen.rs`
was fixed (verifier Round 2). The `StoreIndexed` case carried the same "No
bounds check is emitted at this milestone" stale text and had no AMBIGUOUS
comment in `codegen.rs` at all.

**`src/ir.rs` — `StoreIndexed` AMBIGUOUS comment:**
Before: "No bounds check is emitted at this milestone."
After: Names the current resolution — `cmp`/`b.hs` when `len > 0`, deferred for
slice params with unknown length. Same structure as `LoadIndexed`.

**`src/codegen.rs` — `StoreIndexed` section:**
Before: Plain comments ("Bounds check when len > 0: cmp + b.hs before the store")
with no AMBIGUOUS annotation.
After: Full `FLS §6.9 AMBIGUOUS:` comment with resolution stated, matching the
`LoadIndexed` block above it.

- Files: `src/ir.rs`, `src/codegen.rs`

## Validated
- `cargo test`: 2084 passed, 0 failed.
- Three-file consistency (ir.rs / codegen.rs / fls-ambiguities.md) now holds
  for both `LoadIndexed` and `StoreIndexed`.
- Verifier: grep for `FLS §6.9 AMBIGUOUS` in `src/ir.rs` and `src/codegen.rs` —
  both should name the cmp/b.hs resolution with the deferred-slice caveat.
