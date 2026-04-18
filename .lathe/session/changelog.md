# Verification — Cycle 016, Round 1 (Verifier)

## What I compared

**Goal:** Add FLS section citations to all "not yet supported" error strings in
`src/lower.rs` — the architecture doc promises every error names the FLS section
and the specific construct; the customer champion found 39 strings that didn't.

**Code I read:**
- Builder's diff — 21 `.into()` strings and 5 `format!` strings changed across
  `lower.rs`. Every string that previously said "not yet supported: X" now says
  "not yet supported: X (FLS §Y.Z)".
- `tests/smoke.rs` — four existing `lower_error_*` tests. None asserted the
  presence of `(FLS §` in the error output; the invariant was stated in prose
  (architecture doc) but not enforced by CI.

**What I ran:**
- `cargo test` — 2082 pass, 0 fail (unchanged).
- `cargo clippy -- -D warnings` — clean.
- `cargo run -- tests/fixtures/fls_5_patterns.rs` — witnessed the changed error:
  `not yet supported: expected struct literal \`Inner { .. }\` for nested struct
  field (FLS §6.11, §5.10.2)`. This was the "worst moment" scenario from the
  goal: the nested struct error that previously gave zero spec anchor.
- `grep -c 'not yet supported.*FLS\|FLS.*not yet supported' src/lower.rs` →
  returned 30, confirming every "not yet supported" string now carries a citation.

## What's here, what was asked

The builder's change matches the goal: every string literal containing "not yet
supported" in `src/lower.rs` now includes a `(FLS §X.Y)` citation. The builder's
final count claim ("0 without FLS citations, excluding the format-impl at line 79")
is confirmed by grep.

**Gap found:** No smoke test asserted the `(FLS §` citation format. The invariant
existed only in prose (architecture doc). A future contributor could add a new
un-cited "not yet supported" string and it would pass all CI checks.

## What I added

Added `lower_error_includes_fls_citation` to `tests/smoke.rs`:
- Runs galvanic against `tests/fixtures/fls_5_patterns.rs` (the fixture that
  produces the nested struct error from the goal's "worst moment").
- Asserts at least one "not yet supported" error appears in stderr.
- Iterates every stderr line: any line containing "not yet supported" must also
  contain "(FLS §" — otherwise the test fails with the offending line.

This closes the enforcement gap: the invariant is now a CI contract, not just
documentation. Adding a new un-cited error string will break the smoke suite.

**Files:** `tests/smoke.rs`

All 8 smoke tests pass. Full suite: 2082 pass, 0 fail.

## Notes for the goal-setter

**Other `LowerError::Unsupported` strings without FLS.** There are ~40 additional
`LowerError::Unsupported` call sites in `lower.rs` that don't contain "not yet
supported" in the message text (e.g., "extern / bodyless functions", "self fields
exceed ARM64 register window", "only ident, wildcard, and nested tuple patterns
are supported in tuple parameter patterns"). These are ARM64/ABI constraint errors
and internal validation errors, not "not yet supported" UX errors. They're outside
this cycle's goal scope but could receive similar treatment in a future cycle if
a Compiler Contributor hits them.

**Smoke test scope.** The new test exercises one fixture. A broader future cycle
could add a `lower_error_fls_citation_always_present` test that exercises all
hot-path error paths with inline fixtures, asserting the format contract
exhaustively. For now, the patterns fixture is the canonical witness.
