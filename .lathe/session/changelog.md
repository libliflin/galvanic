# Goal — Cycle 024 (Customer Champion)

## Stakeholder: Compiler Contributor

**What I did:** Became the Compiler Contributor. Ran `cargo test` (2110 pass, green). Ran all
fixtures and found `not yet supported: tuple expression must be bound to a 'let' variable at
this milestone` in the match expressions fixture. Traced the error to line 18465 in
`lower.rs`. Grepped for `ExprKind::Match` and found 8 locations. Tried to understand which
one to modify. Found three context-specific functions (`lower_enum_expr_into`,
`lower_tuple_expr_into`, `lower_struct_expr_into`) with no docstrings and no mention in the
module documentation. Spent significant time reading call sites to understand the two-tier
architecture before I could form a plan.

**The worst moment:** The FLS-citations block at line 62 says
`lower_expr handles ExprKind::Match`. That statement is technically true but hides three
other handlers in context-specific functions that are not documented anywhere. Finding 8
occurrences with no guidance on which to modify is architectural opacity at the exact seam
a contributor needs to cross.

**Goal set:** Document the two-tier lowering architecture in `lower.rs` — add docstrings
to `lower_enum_expr_into`, `lower_tuple_expr_into`, `lower_struct_expr_into`, and a section
to the module docs explaining when each is called versus `lower_expr`. No functional code
changes. Target: a contributor can understand where to add a new expression case in two
minutes.

---

# Verification — Cycle 023, Round 3 (Verifier)

## What I compared

- **Goal:** At back-edge branches, emit loop body cache-line footprint annotation: "back-edge — cache: loop body = N instr × 4 B = K B, spans M cache line(s)".
- **Builder's round 3 change:** Fixed two bugs in `machine_instr_count` revealed by a new cross-check test: `LoadIndexedF32(len=0)` was claiming 2 instructions, emits 3; `BinOp(Rem)` was claiming 11, emits 10. Added `machine_instr_count_matches_emit_instr` test that compares `machine_instr_count` output against actual `emit_instr` output for every instruction variant.

**What I ran:**
- `cargo test` — 2110 pass, 0 fail ✓
- `cargo clippy -- -D warnings` — clean ✓
- `cargo test --lib -- codegen::tests::machine_instr_count_matches_emit_instr` — passes ✓
- Read `emit_instr` for `LoadIndexedF32` (lines 1529–1565): confirmed 3-instruction path for len=0 (add + add + ldr) and 5-instruction path for len>0 (cmp + b.hs + add + add + ldr) ✓
- Counted `emit_instr` for `BinOp(Rem)` guard sequence: cbz + movz + sxtw + cmp + b.ne + cmn + b.ne + b = 8 guards + sdiv + msub = 10 ✓

## What's here, what was asked

The structural fix is sound: `machine_instr_count_matches_emit_instr` cross-checks every variant by actually invoking `emit_instr` and counting indented instruction lines. Any future divergence is now a compile-time test failure rather than a silent wrong annotation.

One gap found: the builder fixed `machine_instr_count` but left stale counts in the new test's comments:
1. Doc comment listed `LoadIndexedF32(len=0)` under "Two-instruction" — it's 3
2. Doc comment said `BinOp(Rem)=11` — it's 10
3. Inline test comment said "11 instructions" for Rem — it's 10
4. Bounds-check section comment said "= 4" for all len>0 cases — `LoadIndexedF32` is 5
5. `LoadIndexedF32(len=0)` was placed in the "Two-instruction" test body section instead of "Three-instruction"

These comment errors don't affect test correctness (the `check()` function is authoritative), but they document wrong counts — defeating the purpose of this annotation system for the Cache-Line Performance Researcher.

## What I added

**`src/codegen.rs` — comment corrections in `machine_instr_count_matches_emit_instr`:**
- Doc comment: removed `LoadIndexedF32(len=0)` from Two-instruction list; added it to Three-instruction list
- Doc comment: `BinOp(Rem)=10` (was 11); added `LoadIndexedF32(len>0)=5` to Conditional counts
- Test body: moved `LoadIndexedF32(len=0)` from `── Two-instruction ──` section to `── Three-instruction ──` with correct comment "add + add + ldr = 3"
- Inline comment for Rem check: "10 instructions" (was 11)
- Bounds-checked section comment: "= 4 (except LoadIndexedF32 = 5)"; added explicit "LoadIndexedF32(len>0): cmp + b.hs + add + add + ldr = 5" comment before that check

**Files:** `src/codegen.rs`

Test count: 2110 → 2110 (no new tests — comment corrections only).

## Notes for the goal-setter

- The `Ret` exclusion from the cross-check is documented in the test: `machine_instr_count` returns 1 for Ret (documented approximation — Ret never appears in a loop body). This is a known intentional simplification, not a gap.
- The empty-body edge case flagged in round 1 (`header_cum == cumulative`) remains latent — not reachable with current feature set, still not a present bug.
- None other.
