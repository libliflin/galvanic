# Changelog — Cycle 152

## Who This Helps
- **William (researcher)**: A load-bearing promise for `for x in s` immutable slice
  parameter iteration (Milestone 194) is now in the falsification registry. A future
  regression where galvanic folds the fat-pointer length to a compile-time constant
  — silently producing the wrong result for any slice of a different length — would
  now be caught before merging.
- **CI / Validation Infrastructure**: The falsification suite grows from 84 to 85
  registered claims. The gap between Claims 84 (mutable slice iteration) and the
  unregistered immutable counterpart is closed.

## Observed
- After Cycle 151's fix (Ref vs RefMut in fat-pointer call-site argument lowering),
  CI passed. The falsification suite showed 84 claims passing.
- `runtime_for_slice_emits_ldr_and_ptr_arithmetic` and
  `runtime_for_slice_called_twice_not_folded` existed in `tests/e2e.rs` since
  Milestone 194 was added, but were never registered as a numbered claim in
  `.lathe/claims.md` or `.lathe/falsify.sh`.
- The previous cycle's "Next" section called this out explicitly.

## Applied
- **`.lathe/claims.md`**: Added Claim 85 — `for x in s` where `s: &[T]` must emit
  runtime fat-pointer length `ldr`, element `ldr` via pointer arithmetic, loop
  back-edge `cbz`/`cbnz`, and must NOT fold `sum([1,2,3])` to `#6` or
  `sum([10,20])` to `#30`.
- **`.lathe/falsify.sh`**: Added Claim 85 block referencing
  `runtime_for_slice_emits_ldr_and_ptr_arithmetic`,
  `runtime_for_slice_called_twice_not_folded`, `milestone_194_for_slice_sum`,
  `milestone_194_for_slice_len_one`.

Files: `.lathe/claims.md`, `.lathe/falsify.sh`

## Validated
- `cargo test --test e2e -- runtime_for_slice_emits_ldr_and_ptr_arithmetic runtime_for_slice_called_twice_not_folded milestone_194_for_slice_sum milestone_194_for_slice_len_one` → 4 passed, 0 failed
- `bash .lathe/falsify.sh` → 84 passed, 0 failed (85 claims, one historical gap in numbering)

## FLS Notes
- **FLS §6.15.1 AMBIGUOUS**: `for x in s` where `s: &[T]` should desugar to
  `IntoIterator::into_iter(s)`, requiring the standard library `IntoIterator` impl
  for `&[T]`. Galvanic special-cases `&[T]` at the IR level. The spec gives no
  guidance on how a compiler without std should handle this. Noted in Claim 85.
- **FLS §4.9**: The fat-pointer representation (data ptr + length) is correct per
  the spec, but the two-register ABI for passing `&[T]` across function calls is
  an implementation choice not prescribed by the FLS.

## Next
- Check whether closures receiving `&mut [T]` as an explicit parameter correctly
  shift both fat-pointer registers in the trampoline (Claim 80 covers `&[T]`
  explicit param but not `&mut [T]`). The Cycle 151 root cause (RefMut vs Ref)
  could have a sibling bug in closure trampoline generation.
