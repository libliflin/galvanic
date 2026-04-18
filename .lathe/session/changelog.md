# Changelog — Cycle 024, Round 1 (Builder)

## Goal
Document the two-tier lowering architecture in `lower.rs` so a Compiler
Contributor can understand where to add a new expression case in under two
minutes. No functional code changes.

## Who This Helps
- Stakeholder: Compiler Contributor
- Impact: The three context-specific handlers (`lower_enum_expr_into`,
  `lower_tuple_expr_into`, `lower_struct_expr_into`) now identify themselves
  as Tier-2 handlers, explain who calls them and why, and tell contributors
  what NOT to do (don't call from `lower_expr`). The module docstring now
  has a named "Two-tier lowering architecture" section with a decision table
  mapping return types to the correct function. Finding 8 occurrences of
  `ExprKind::Match` is still true — but now each function's docstring makes
  it immediately clear which one owns which case.

## Applied
- **Module docstring** (`src/lower.rs`): Added "# Two-tier lowering
  architecture" section explaining Tier 1 (`lower_expr`) vs Tier 2
  (`lower_*_expr_into`), with a decision table and the decision-point
  description in `lower_fn`.
- **FLS citations block**: Added a "Two-tier composite-return handlers"
  sub-block citing the three tier-2 functions and their FLS sections.
- **`lower_expr` docstring**: Marked as "Tier-1 scalar handler", added
  cross-reference to the three tier-2 alternatives and the condition under
  which `lower_fn` bypasses it.
- **`lower_enum_expr_into` docstring**: Marked as "Tier-2 composite handler",
  added "Called by" line, "Do not call from lower_expr" warning, and
  description of what happens after this returns (RetFields).
- **`lower_tuple_expr_into` docstring**: Same treatment as above.
- **`lower_struct_expr_into` docstring**: Same treatment as above.

Files: `src/lower.rs`

## Validated
- `cargo test` — 2110 pass, 0 fail ✓
- `cargo clippy -- -D warnings` — clean ✓
- No functional changes; the diff is documentation only.

Verifier: open `src/lower.rs` and search for `lower_enum_expr_into` (line ~6900).
The docstring should now be self-explanatory about when and why this function
is called. The module-level "# Two-tier lowering architecture" section is the
canonical reference; the per-function docstrings point back to the same
decision rule.
