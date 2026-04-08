# Changelog — Cycle 139

## Who This Helps
- **William (researcher)**: Milestone 193 (`&[T]` slice parameters) now has complete
  three-layer test coverage. The parse acceptance fixture documents two FLS §4.9
  ambiguities clearly in code, making the research output visible in the fixture layer.
- **FLS Maintainers**: `tests/fixtures/fls_4_9_slices.rs` explicitly documents two
  §4.9 gaps — fat pointer ABI and bounds-checking mechanism — as `AMBIGUOUS` comments
  in a file derived from FLS examples. A maintainer reading this file sees real gaps.

## Observed
- Cycle 138 added Layer 2 (assembly inspection) and Layer 3 (compile-and-run) for
  milestone 193, but explicitly noted Layer 1 (parse acceptance fixture) was missing.
- `tests/fixtures/fls_4_9_slices.rs` did not exist; no `fls_4_9_slices` test in
  `tests/fls_fixtures.rs`. The three-layer coverage was incomplete.

## Applied
- **`tests/fixtures/fls_4_9_slices.rs`** (new): FLS §4.9-derived fixture with three
  functions — `slice_len`, `slice_sum`, `slice_first` — demonstrating `&[T]` params
  with `.len()`, indexing, and while-loop summation. Documents both §4.9 ambiguities:
  fat pointer ABI and bounds-checking mechanism.
- **`tests/fls_fixtures.rs`**: Added `fls_4_9_slices` test calling `assert_galvanic_accepts`.

## Validated
- `cargo test --test fls_fixtures fls_4_9_slices` — 1 passed
- `cargo test` — 1916 passed (was 1915, 1 new test)
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- **FLS §4.9 AMBIGUOUS**: Fat pointer ABI not specified (galvanic uses two consecutive
  ARM64 registers: data pointer + element count).
- **FLS §4.9 AMBIGUOUS**: Bounds checking mechanism not specified (galvanic omits it
  at this milestone).

## Next
- Implement `for x in slice` iteration (FLS §6.15.1 for-loop over `&[T]`), which
  is the natural follow-on now that slices are accessible. Currently `for` loops
  only work over integer ranges — extending to slice iteration would be the
  next meaningful step toward "programs that look like real Rust".
- Alternatively, implement `&mut [T]` parameters to enable in-place mutation via slices.
