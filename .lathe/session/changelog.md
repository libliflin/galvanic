# Verification — Cycle 016, Round 2 (Verifier)

## What I compared

**Goal:** Add FLS section citations to all "not yet supported" error strings in
`src/lower.rs`. Builder's round 2 closed two remaining uncited strings and added
a static source-level test to enforce the invariant permanently.

**Code I read:**
- Builder's diff — two `lower.rs` string fixups (line 6157 and 18485) and a new
  `lower_source_all_unsupported_strings_cite_fls` test in `tests/smoke.rs`.
- The static test logic: reads `src/lower.rs` line-by-line, skips lines starting
  with `//`, `*`, or containing `write!(f,`, then fails on any line that contains
  `"not yet supported"` without `"(FLS §"` on the same line.

**What I ran:**
- `cargo test --test smoke` — 9 pass, 0 fail.
- `cargo test` — 2084 pass, 0 fail.
- `cargo clippy -- -D warnings` — clean.
- `grep -n '"not yet supported' src/lower.rs` — confirmed every string literal
  carries a `(FLS §` citation. The only uncited line is line 79 (the Display impl
  prefix `write!(f, "not yet supported: {msg}")`), which the static test correctly
  excludes.
- Multi-line string at line 6155–6158 (the nested tuple fix): the citation
  `(FLS §5.10.3, §8.1)` lands on the same line as `"not yet supported"` (line 6157),
  so the static test's per-line check catches it correctly.

## What's here, what was asked

Matches: the work holds up against the goal from my lens.

- All 29 non-Display `"not yet supported"` string literals in `lower.rs` carry
  `(FLS §...)` citations.
- The static smoke test makes the invariant enforceable by CI — any future
  contributor who adds an uncited string will see the test fail immediately,
  regardless of whether that code path is exercised by a fixture.
- The runtime test (`lower_error_includes_fls_citation`) remains as a
  complementary witness: it confirms the citation survives the full pipeline to
  stderr, not just that it exists in the source.

The two-test strategy (static source check + runtime CLI check) closes both
failure modes the verifier identified last round. Nothing is left open.

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

**Multi-line string edge case.** The static test checks line-by-line. A future
contributor who writes a continuation string where the citation lands on a
separate line from the phrase would fail the test correctly. The current
multiline string at line 6155 is structured correctly (citation on the same
continuation line as "not yet supported"). Worth noting as a style rule for
contributors: keep the citation on the same line as the phrase. No code change
needed — the test already enforces the right outcome.

**ARM64/ABI constraint errors without FLS citations.** There are ~40 additional
`LowerError::Unsupported` call sites in `lower.rs` with messages like "extern /
bodyless functions", "self fields exceed ARM64 register window", etc. These are
structural/ABI constraint errors — the static test doesn't flag them because they
don't contain "not yet supported". They could receive similar citation treatment in
a future cycle if a Compiler Contributor hits them.
