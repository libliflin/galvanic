# Goal: Implement §6.18 Match Exhaustiveness — Compile-Time Gap Check

## What

Add a compile-time exhaustiveness check to galvanic's match expression lowering
(`src/lower.rs`) so that non-exhaustive match arms produce a diagnostic error
rather than silently falling through to undefined behavior.

The builder should:

1. In the match lowering path (`lower_match` or equivalent in `src/lower.rs`),
   after collecting all match arms, verify exhaustiveness:
   - If any arm is a wildcard (`_`) or an identifier pattern, the match is trivially
     exhaustive — no check needed.
   - If all arms are literal patterns on an integer type, warn/error that the match
     may not be exhaustive (a wildcard or range pattern is needed). Galvanic's choice:
     emit a compile-time `Err` indicating "non-exhaustive match — add a wildcard arm."
   - If all arms are enum variant patterns, verify that every declared variant name
     appears at least once (or a wildcard exists). Emit a compile-time `Err` for
     missing variants.

2. Add at least two test cases to `tests/e2e.rs`:
   - A test that verifies galvanic *rejects* a clearly non-exhaustive integer match
     (all literal arms, no wildcard) by asserting `compile_to_asm()` or `compile_and_run()`
     returns an error rather than producing assembly.
   - A test that verifies galvanic *accepts* the same match once a wildcard arm is added.

3. Update the §6.18 entry in `refs/fls-ambiguities.md`:
   - Change "No exhaustiveness check is performed at this milestone" to document the
     scope of what IS now checked and what remains deferred (e.g., range pattern
     exhaustiveness, tuple/struct pattern completeness).
   - Add the FLS gap: the spec says exhaustiveness is required but provides no algorithm.
     Document galvanic's chosen algorithm (heuristic: any arm with wildcard/ident = trivially
     exhaustive; literal-only arms with no wildcard = non-exhaustive for integer types).

The check does NOT need to be complete. A conservative check that catches the most
common case (literal-only integer arms with no wildcard or ident catch-all) is enough.
False negatives (accepting a non-exhaustive match) are acceptable at this milestone.
False positives (rejecting a valid exhaustive match) are not acceptable.

## Which stakeholder

**William (the researcher)** and **FLS spec readers (the Ferrocene team)**.

The §6.18 entry in `refs/fls-ambiguities.md` currently reads:
> "If no arm matches at runtime, the match expression falls through to undefined behavior."

This is the most visible correctness gap in the ambiguities document — a case where
galvanic silently produces wrong code rather than catching the error. Every FLS spec
reader who opens the ambiguities doc will see this. Closing it (even partially) changes
the narrative from "galvanic ignores non-exhaustive matches" to "galvanic checks the
most common case and documents what the FLS leaves underspecified."

For William: this is the kind of change that makes the research output credible.
Reporting "the FLS doesn't define an exhaustiveness algorithm" is more convincing when
galvanic has a concrete implementation that makes a defensible choice.

## Why now

The project just completed a four-commit documentation sprint (#256–#261). The
ambiguities and constraints documents are in good shape. The build is clean. The test
count is at 1987, all passing.

The §6.18 gap is the most prominent correctness hole called out in `refs/fls-ambiguities.md`:
"falls through to undefined behavior." Every other deferred gap (bounds checking, overflow
panics, unsafe enforcement) requires a runtime panic infrastructure that doesn't exist yet.
Exhaustiveness checking is different — it happens entirely at compile time. No runtime
infrastructure needed. One function, one new error kind, two tests.

This is the single deferred item in the ambiguities doc that can be closed without a
panic runtime. Doing it now moves the document from "here are the gaps" to "here are the
gaps, and here's the one we fixed."

---

## Acceptance criteria

- `cargo build` passes.
- `cargo test` passes (all 1987 existing tests continue to pass).
- At least one new test demonstrates that a literal-only integer match without a wildcard
  arm is rejected at lowering time.
- At least one new test demonstrates that adding a wildcard arm makes the same match
  compile and produce the correct result.
- The §6.18 entry in `refs/fls-ambiguities.md` is updated to reflect the new state:
  what is checked, what is not, and what the FLS leaves undefined.
- No new FLS citations are wrong or vague.

## FLS notes

- **§6.18:37–41**: "A match expression is exhaustive if every possible value of the
  subject expression type is covered by at least one match arm." The spec requires
  exhaustiveness but provides no algorithm. This is the primary FLS gap galvanic's
  check must document.
- The heuristic: if any arm is a wildcard (`_`) or an identifier pattern (which binds
  any value), the match is trivially exhaustive. Otherwise, for integer-typed subjects
  with only literal arms: non-exhaustive.
- Enum exhaustiveness: if the subject is an enum, collect all variant names from the
  enum definition; check that every variant appears in at least one arm or a wildcard
  arm exists.
