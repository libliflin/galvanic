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
