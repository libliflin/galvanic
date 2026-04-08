# Changelog — Cycle 146

## Who This Helps
- **William (researcher)**: Claim 81 locks in the `for x in &arr` path as a named,
  tested promise. If a future change accidentally constant-folds the loop body for
  array parameters, `falsify.sh` will catch it before CI runs.
- **CI / Validation Infrastructure**: The falsification suite grows from 80 to 81 claims.
  Claim 81 specifically tests with a function parameter (values unknown at compile time)
  — the strongest possible adversarial design for this feature.

## Observed
- Cycle 145 added Milestone 195 (`for x in &arr`) with 2 assembly inspection tests and
  9 compile-and-run tests. Its "Next" section explicitly called out the missing Claim 81.
- The two assembly inspection tests (`runtime_for_arr_borrow_emits_indexed_load` and
  `runtime_for_arr_borrow_two_arrays_not_folded`) were in `tests/e2e.rs` but not
  registered in the falsification suite — so `falsify.sh` would not catch a regression.

## Applied
- **`tests/e2e.rs`**: Added `claim_81_for_arr_borrow_param_emits_indexed_load_not_folded`.
  Uses `sum_borrow(a: [i32; 3])` (array as function parameter — element values unknown
  inside the function body). Calls with `[1,2,3]` and `[10,20,30]`. Asserts: `ldr` present,
  no constant #6, #60, or #66.
- **`.lathe/claims.md`**: Added Claim 81 entry with stakeholder, promise, ARM64
  implementation rationale, FLS citations, and violated-if conditions.
- **`.lathe/falsify.sh`**: Added Claim 81 block that runs `claim_81_*`,
  `runtime_for_arr_borrow_emits_indexed_load`, and `runtime_for_arr_borrow_two_arrays_not_folded`.

## Validated
- `cargo test --test e2e -- claim_81 runtime_for_arr_borrow` — 3 passed, 0 failed
- `cargo test` — 1700 passed, 0 failed (was 1699, +1)
- `cargo clippy -- -D warnings` — clean
- `bash .lathe/falsify.sh` — 80 passed, 0 failed (was 79, +1 Claim 81)

## FLS Notes
No new ambiguities discovered. The existing FLS §6.15.1 and §4.9 AMBIGUOUS notes from
Cycle 145 remain accurate: galvanic bypasses `IntoIterator` dispatch and yields `i32`
(not `&i32`) as the loop variable type.

## Next
- `for x in &mut arr` (Milestone 196) — mutable borrow iteration enabling in-place
  element modification. The FLS §4.9 path is now well-defended for immutable borrows.
- Alternatively, a `str` iteration milestone could expand the `&str` coverage from
  length-only (Milestone 192) to byte-level access (`b.as_bytes()` or char iteration).
