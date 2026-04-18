# Changelog — Cycle 016, Round 2 (Builder)

## Goal
- Add FLS section citations to all "not yet supported" error strings in `src/lower.rs`
  so a Compiler Contributor can navigate to the spec without reading surrounding code.

## Who This Helps
- Stakeholder: Compiler Contributor
- Impact: Every "not yet supported" error now links to the spec section governing the
  construct, cutting the time to find the relevant FLS section from "read the code" to
  "follow the citation."

## Applied
Two additions this round, each filling a gap the verifier's runtime test couldn't close:

**1. Fixed two missed uncited strings in `src/lower.rs`:**
- `lower.rs:6157` — "variable-init nested tuples are not yet supported" inside
  `Pat::Tuple` handling in `lower_tuple_pat_from_literal`. The surrounding comment
  already cited FLS §5.10.3; the error string itself did not. Added `(FLS §5.10.3, §8.1)`.
- `lower.rs:18485` — indexing error for non-array variables had `FLS §6.9` embedded
  mid-sentence (`, FLS §6.9`) rather than in the standard trailing form. Reformatted to
  `(FLS §6.9)` to match the invariant. The static test below surfaces this now rather
  than waiting for the path to be exercised by a fixture.

**2. Added `lower_source_all_unsupported_strings_cite_fls` to `tests/smoke.rs`:**
- Reads `src/lower.rs` directly and scans every non-comment line for "not yet supported".
  Any such line without `(FLS §` on the same line fails the test.
- Excludes the `LowerError` Display impl (`write!(f, ...)`) — it is the prefix
  machinery, not a message payload.
- Closes the gap the verifier identified: the runtime smoke test
  (`lower_error_includes_fls_citation`) catches violations only in paths exercised by
  `fls_5_patterns.rs`. The static test catches them at source level — any new uncited
  string fails CI immediately, regardless of which code path it's in.

Files: `src/lower.rs`, `tests/smoke.rs`

## Validated
- `cargo test --test smoke` — 9 pass, 0 fail. Both runtime and static citation tests pass.
- `cargo test` — 2084 pass, 0 fail.
- `cargo clippy -- -D warnings` — clean.
- Witness the static test: `cargo test --test smoke lower_source_all_unsupported_strings_cite_fls`
- Witness it fail: add a "not yet supported" string without `(FLS §` to `lower.rs` and re-run.
