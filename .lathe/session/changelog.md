# Changelog — Cycle 150

## Who This Helps
- **William (researcher)**: `for x in s` where `s: &mut [T]` now compiles. This is the
  idiomatic Rust pattern for in-place slice transformation (e.g., doubling all elements of
  any-length slice, normalizing values). Previously only fixed-size local arrays supported
  mutable iteration (Milestone 196). With Milestone 197, callers can pass slices of any
  runtime-determined length.
- **CI / Validation Infrastructure**: Claim 84 grows the falsification suite from 83 to 84
  passing claims. Any future change that breaks the address-yielding path will be caught
  immediately.

## Observed
- Cycle 149 hardened `for x in arr` (consuming) with Claim 83. The "Next" section said to
  check milestones 127–132 for assembly inspection coverage — they have negative assertions.
- The natural completion of the for-loop coverage matrix was `for x in &mut slice`:
  - `for x in arr` (Milestone 109) with Claim 83
  - `for x in &arr` (Milestone 195) with Claim 81
  - `for x in &mut arr` (Milestone 196) with Claim 82
  - `for x in &[T]` (Milestone 194) with assembly inspection
  - `for x in &mut [T]` (Milestone 197) ← this cycle

## Applied
- **`src/lower.rs`**:
  - Added `local_mut_slice_slots: HashSet<u8>` field to `LowerCtx` — tracks which fat-pointer
    slots hold `&mut [T]` (mutable) rather than `&[T]` (immutable) slices.
  - In parameter lowering: when `TyKind::Ref { mutable: true, inner: Slice }`, inserts into
    both `local_slice_slots` (for `.len()` and indexing) AND `local_mut_slice_slots`.
  - Changed `slice_iter` type from `Option<u8>` to `Option<(u8, bool)>` where bool = `is_mut_slice`.
  - In element-binding section: when `is_mut_slice`, stores `r_addr` (element address) directly
    into `elem_slot` — no `LoadPtr`. For immutable slices, existing `LoadPtr + Store` unchanged.
  - In closure parameter handling: also propagates `local_mut_slice_slots` for `&mut [T]` params.
- **`tests/e2e.rs`**: 10 new tests:
  - 8 compile-and-run (`milestone_197_*`): double-in-place, increment, set-all, single-element,
    param (adversarial), result-in-arithmetic, called-twice, continue
  - 2 assembly inspection: `runtime_for_mut_slice_emits_mul_and_store_not_folded` and
    `runtime_for_mut_slice_called_twice_not_folded`
- **`.lathe/claims.md`**: Added Claim 84 with rationale, FLS citations, violated-if conditions.
- **`.lathe/falsify.sh`**: Added Claim 84 block running 4 tests.

Files: `src/lower.rs`, `tests/e2e.rs`, `.lathe/claims.md`, `.lathe/falsify.sh`

## Validated
- `cargo test --test e2e -- milestone_197 runtime_for_mut_slice` → 10 passed, 0 failed
- `cargo test` → 1722 passed, 0 failed (was 1712, +10)
- `cargo clippy -- -D warnings` → clean
- `bash .lathe/falsify.sh` → 83 passed, 0 failed (was 82, +Claim 84)
- PR #229 created: https://github.com/libliflin/galvanic/pull/229

## FLS Notes
- **FLS §6.15.1 AMBIGUOUS**: `for x in s` where `s: &mut [T]` should desugar to
  `IntoIterator::into_iter(s)`, requiring the standard library `IntoIterator` impl for
  `&mut [T]`. Galvanic special-cases `&mut [T]` at the IR level.
- **FLS §4.9 AMBIGUOUS**: The loop variable `x` should have type `&mut T`. Galvanic stores
  the element address (i64 pointer) in `x`'s slot — same observable behavior for `*x`
  reads/writes on Copy types (i32), but the binding type is not tracked.

## Next
- **Claim 85 for Milestone 194** (`for x in &[T]` slice): the two existing assembly inspection
  tests are not registered in `falsify.sh`. Adding Claim 85 would close this gap.
- **§6.16 Range Expressions with step**: `(0..10).step_by(2)` — extends for-loop coverage
  to non-unit steps. Currently galvanic only handles unit increments (0..n, 0..=n).
