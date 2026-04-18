# Changelog — Cycle 017, Round 2 (Builder)

## Goal
Fix the stale and contradictory §4.9 fls-ambiguities entry so a Spec Researcher
can read a clean, citable finding about galvanic's bounds-check behavior.

## Who This Helps
- Stakeholder: Spec Researcher (cycle 017)
- Impact: The §4.9 entry now points to the actual source lines, and the IR doc
  comment no longer contradicts how the code behaves.

## Applied

**Two fixes this round:**

### 1. Stale IR comment in `src/ir.rs` (`LoadIndexed`)

The `LoadIndexed` variant's doc comment (line 750) said "Galvanic does not emit
bounds checks at this milestone." This is wrong: the `len` field exists precisely
to drive bounds-check emission in codegen. A Compiler Contributor reading ir.rs
would see a direct contradiction of the implementation. Updated to:

> Galvanic's resolution: when `len > 0`, codegen emits `cmp x{index_reg},
> #{len}; b.hs _galvanic_panic` before the load. Slice parameters that
> lack a static length (`len == 0`) receive no bounds check (deferred).

The AMBIGUOUS annotation now has all three required parts: the spec gap, the
current choice, and the deferred case.

### 2. Stale source citations in `refs/fls-ambiguities.md` §4.9

The entry cited `src/ir.rs:730`, `src/codegen.rs:926`, `src/lower.rs:17880`.
All three were wrong:
- `ir.rs:730` is `RetFields` — the `LoadIndexed` variant is at line 761.
- `codegen.rs:926` is sign-extension code — the bounds-check emission is at 1128.
- `lower.rs:17880` is a string-slice lookup — the indexing lowering is at 18325.

Updated to: `src/ir.rs:761`, `src/codegen.rs:1128`, `src/lower.rs:18325`.

**Files:** `src/ir.rs`, `refs/fls-ambiguities.md`

## Validated
- `cargo test --lib` — 215 passed, 0 failed.
- `cargo clippy -- -D warnings` — clean.
- Verifier: navigate to `src/ir.rs:761` (`LoadIndexed`), confirm the AMBIGUOUS
  comment names the resolution. Navigate to `refs/fls-ambiguities.md` §4.9,
  confirm source citations resolve to bounds-check code.
