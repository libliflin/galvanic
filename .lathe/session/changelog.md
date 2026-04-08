# Changelog — Cycle 153

## Who This Helps
- **William (researcher)**: Two new falsification claims close the fat-pointer coverage
  gap in the falsification suite. A future regression where galvanic clobbers the length
  of a `&mut [T]` fat pointer in a closure trampoline (or constant-folds a `for x in s`
  result for `s: &[T]`) would now be caught before merging.
- **CI / Validation Infrastructure**: Falsification suite grows from 84 to 86 registered
  claims. Both `for x in &[T]` iteration (Claim 85) and `&mut [T]` closure trampolines
  (Claim 86) are now load-bearing entries.

## Observed
- Cycle 152's changelog intended to add Claim 85 (`for x in &[T]` iteration) but the
  work was not committed. The falsification suite ended at Claim 84 with 83 registered
  claims.
- The cycle 152 "Next" section explicitly pointed to verifying that closures with
  `&mut [T]` explicit parameters correctly shift both fat-pointer registers in the
  trampoline (a potential sibling of the Claim 80 bug).
- Inspection of `lower.rs` line 18319: the `n_explicit_regs` computation uses
  `TyKind::Ref { inner, .. }` with `..` ignoring `mutable`, so both `&[T]` and
  `&mut [T]` are correctly counted as 2 register slots. No implementation bug —
  but no test defended this property.

## Applied
- **`tests/e2e.rs`**: Added two new tests in the Claim 86 section:
  - `runtime_closure_trampoline_shifts_fat_ptr_mut_slice_param` — verifies the
    trampoline emits `mov x2, x1` (len shift) and `mov x1, x0` (ptr shift) when
    the closure has a `&mut [i32]` explicit parameter.
  - `claim_86_closure_trampoline_mut_slice_param_passes_len_not_just_ptr` — adversarial:
    calls the same capturing closure with slices of length 3 and 2; both trampolines
    must shift the len register (`// shift explicit arg 1 to position 2`), proving the
    length is not clobbered.
- **`.lathe/claims.md`**: Added Claim 85 (for x in &[T] iteration, registering existing
  tests) and Claim 86 (&mut [T] closure trampoline fat-pointer shift).
- **`.lathe/falsify.sh`**: Added Claim 85 and Claim 86 blocks.

Files: `tests/e2e.rs`, `.lathe/claims.md`, `.lathe/falsify.sh`

## Validated
- `cargo test --test e2e -- runtime_closure_trampoline_shifts_fat_ptr_mut_slice_param claim_86_closure_trampoline_mut_slice_param_passes_len_not_just_ptr` → 2 passed, 0 failed
- `bash .lathe/falsify.sh` → 85 passed, 0 failed

## FLS Notes
- **FLS §4.9 AMBIGUOUS**: The fat-pointer ABI (data ptr in xN, length in xN+1) is not
  specified by the FLS. Galvanic's choice of two consecutive ARM64 registers is an
  implementation decision. Both `&[T]` and `&mut [T]` use the same layout — the FLS
  gives no guidance on whether mutability should affect the ABI. Documented in Claim 86.
- **FLS §6.22, §4.13**: The trampoline mechanism for passing capturing closures as
  `impl Fn` is an implementation strategy; the FLS specifies that closure captures
  are in scope but does not prescribe the calling convention.

## Next
- Verify that `for i in 0..n` (milestone 19) has assembly inspection coverage. If not,
  that is a gap: the loop-iteration falsification tests cover arrays and slices but the
  original range-based for loop may only have exit-code tests. An adversarial check
  that `for i in 0..n` with a function parameter `n` emits runtime `cmp`/`cbz`/`b`
  instructions (not constant-folded) would complete the for-loop coverage picture.
