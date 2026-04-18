# Cycle 025 — Customer Champion Changelog

## Stakeholder: Spec Researcher

**Rotation basis:** Cycles 024 = Compiler Contributor, 023 = Cache-Line Researcher,
022 = Lead Researcher, 021 = Spec Researcher. Spec Researcher is the most under-served
(last served four cycles ago).

## What I experienced

I became the Spec Researcher. I opened `refs/fls-ambiguities.md` knowing nothing about its
history. I verified the TOC is in sync with 48 body entries, entries are in section-number
order, and the file has a working table of contents. That part works.

I chose float and numeric-operator semantics as my topic — §6.5.x — and ran a two-minute
clock. The first two entries I found (§6.5.3 NaN, §6.5.5 IEEE 754) were clean and citable.
The third (§6.5.7 Shift Amount Modulo Behavior) was not.

The §6.5.7 entry has three parts that actively contradict each other:
1. **Galvanic's choice** (formal field): "No explicit masking instruction is emitted"
   (old behavior — hardware mod-64 delegation)
2. **Minimal reproducer assembly signature**: shows `cmp x1, #64` + `b.hs _galvanic_panic`
   (current behavior — explicit panic guard)
3. **Note**: "the **Galvanic's choice** description above is stale"

The formal description is wrong. The Note acknowledges it. The assembly signature shows
the actual current behavior. But the reader is left unable to cite anything — the
authoritative field says one thing, the Note says it's wrong, and the Note's own
description of the current behavior is informal.

Cross-referencing source files:
- `lower.rs:11044`: still frames the ambiguity in old terms ("ARM64 uses 6 bits for mod-64")
- `codegen.rs:1023`: correctly describes the current state (`FLS §6.5.9 AMBIGUOUS:
  galvanic checks against 64, not bit_width`)

Three places describe the same design decision with three different framings. The research
document's formal description is the most wrong.

**Hollowest moment:** Writing down "galvanic: no guard, uses hardware mod" from the
**Galvanic's choice** field, then finding the assembly signature shows the opposite, then
reading "Note: the field above is stale." Crossing out my note. Not knowing what to write
instead.

## Goal set

Fix the §6.5.7 entry to reflect current behavior (explicit panic guard for shift amounts ≥
64). Remove the stale Note. Add the narrow-type false negative (shifts of i32 in [32, 63]
are not caught by the guard) as a known gap. Extend the §6.5.9 entry with this same
false-negative as a third gap. Update the lower.rs source annotation at line 11044 to
match. No functional code changes — documentation only. Goal: after this fix, the §6.5.7
entry is citable.

## Why this and not something else

The TOC/body sync is fine (48 entries, in order). Other entries I checked (§6.22 Closures,
§6.18 Match Exhaustiveness) are clear and citable. The §6.5.7 entry is uniquely broken
because it acknowledges its own staleness in the body without correcting it. That's a
different class of problem from a missing entry — it's an entry that actively misleads
while also warning the reader it's misleading them.

---

# Verification — Cycle 024, Round 2 (Verifier)

## What I compared

- **Goal:** Document the two-tier lowering architecture in `lower.rs` so a Compiler Contributor can understand where to add a new expression case in two minutes. No functional changes.
- **Builder's round 1 + Verifier's round 1:** Module-level `# Two-tier lowering architecture` section, FLS citations sub-block for tier-2 handlers, updated docstrings on all three tier-2 functions and `lower_expr`. Round 1 verifier added `Match expression` to the "Handles:" lists of `lower_enum_expr_into` and `lower_struct_expr_into`, and `FLS §6.18` to `lower_tuple_expr_into`'s FLS citations.

**What I ran:**
- `cargo test` — 2110 pass, 0 fail ✓
- `cargo clippy -- -D warnings` — clean ✓
- Walked the Compiler Contributor journey: grepped `FLS §6.18` across `lower.rs` to check which tier-2 handlers a match-focused contributor would find ✓
- Read all three tier-2 function docstrings end-to-end comparing structure and FLS citation completeness ✓

## What's here, what was asked

Two gaps found after round 1's fixes:

**1. Missing `FLS §6.18` in `lower_enum_expr_into` and `lower_struct_expr_into` FLS citation blocks.**
The "Handles:" lists now mention match (added in round 1), but the FLS citation blocks did not. A contributor grepping `FLS §6.18` across `lower.rs` to find all match-handling code would find `lower_expr` (line 102) and `lower_tuple_expr_into` (line 7357), but miss the other two tier-2 handlers. This breaks FLS traceability for the most complex expression kind across exactly the handlers where composite-returning match expressions require special treatment.

**2. `lower_tuple_expr_into` lacked a "Handles:" list.**
The other two tier-2 functions have an explicit "Handles:" list followed by FLS citations. `lower_tuple_expr_into` had only FLS citations. The inconsistency breaks the predictable scanning pattern — a contributor reading all three in sequence hits a different docstring structure for this one.

## What I added

**`src/lower.rs` — three docstring additions:**
- `lower_enum_expr_into`: Added `FLS §6.17`, `FLS §6.4`, and `FLS §6.18` to FLS citation block. (Previously only had `§6.1.2:37–45` and `§15 AMBIGUOUS`.)
- `lower_struct_expr_into`: Added `FLS §6.18` to FLS citation block. (Had `§6.11`, `§6.17`, `§6.4`, `§6.1.2:37–45` but not §6.18.)
- `lower_tuple_expr_into`: Added a "Handles:" list before the FLS citations, listing all four expression kinds the function handles — consistent with the other two tier-2 functions.

After: `grep "FLS §6.18" lower.rs` returns citations in all three tier-2 function docstrings (lines 6935, 7357, 7624).

Files: `src/lower.rs`

## Notes for the goal-setter

- The Compiler Contributor journey now completes cleanly: grep for `FLS §6.18`, find all four handlers, each docstring tells you where to add a new arm. Two-minute goal is reachable.
- `lower_fn` itself has no docstring. The module-level decision table is the reference; inline comments in `lower_fn`'s routing branches are readable. Not a blocker for this goal.
- None other.
