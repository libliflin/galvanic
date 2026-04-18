# Verification — Cycle 024, Round 1 (Verifier)

## What I compared

- **Goal:** Document the two-tier lowering architecture in `lower.rs` so a Compiler Contributor can understand where to add a new expression case in under two minutes. No functional changes.
- **Builder's round 1 change:** Added `# Two-tier lowering architecture` module-level section, FLS citations sub-block for tier-2 handlers, updated docstrings on all three tier-2 functions and `lower_expr`.

**What I ran:**
- `cargo test` — 2110 pass, 0 fail ✓
- `cargo clippy -- -D warnings` — clean ✓
- Walked the Compiler Contributor journey: grepped for `ExprKind::Match`, got 8 locations, read docstrings on the 4 lowering functions to verify navigation achievable in under 2 minutes ✓

## What's here, what was asked

The architecture docs are accurate and the decision table is correct. One gap:

The "Handles:" lists in `lower_enum_expr_into` and `lower_struct_expr_into` omit `Match` expressions — but both functions handle `ExprKind::Match` (lines 7187, 7791). A contributor reading the "Handles:" list would believe match expressions aren't supported in the composite path and look elsewhere. `lower_tuple_expr_into` mentions "match arm" in prose but its FLS citations block was also missing `§6.18`.

## What I added

**`src/lower.rs` — three docstring additions:**
- `lower_enum_expr_into` "Handles:" list: added `Match expression — handles each arm body recursively`
- `lower_struct_expr_into` "Handles:" list: added `Match expression — handles each arm body recursively`
- `lower_tuple_expr_into` FLS citations: added `FLS §6.18: Match expressions where each arm body produces a tuple`

Files: `src/lower.rs`

## Notes for the goal-setter

- `lower_fn` itself has no docstring. The module-level doc points to it as the decision point and the inline comments in its routing branches are sufficient. Not a blocker for the two-minute goal.
- None other.
