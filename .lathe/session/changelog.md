# Changelog — Cycle 141

## Who This Helps
- **William (researcher)**: `for x in s` over `&[i32]` slice parameters is now a first-class
  language feature. The previous workaround (manual `while i < s.len() { ... i += 1 }`) compiled,
  but `for x in s` is the idiomatic Rust pattern. This closes the gap between "programs galvanic
  can compile" and "programs real Rust code looks like."
- **Compiler Researchers**: The lowering strategy is documented with FLS citations and the explicit
  AMBIGUOUS note — the spec desugars `for x in s` via `IntoIterator`, but galvanic special-cases
  `&[T]` at the IR level to avoid requiring a runtime iterator implementation.

## Observed
- Milestone 193 (cycle 138) implemented `&[T]` slice parameters with `.len()` and indexing.
  The natural next step (suggested in cycle 140's changelog) was `for x in slice` iteration.
- The existing `milestone_193_slice_param_sum` test used the manual `while i < s.len()` pattern
  to sum a slice. That's correct but not idiomatic Rust.
- The `for x in arr` path (array variables) and the integer range path both existed in the
  for-loop lowering. The `&[T]` slice path was missing — any attempt to write `for x in s`
  where `s: &[i32]` would fail with "for loop requires an integer range iterator".

## Applied
- **`src/lower.rs`**: Added a third case in `ExprKind::For` handling — slice iterator detection.
  When `iter` is a path to a variable in `local_slice_slots`, the loop:
  1. Loads the data pointer from `ptr_slot` before the loop (stored to a dedicated slot).
  2. Initialises a counter slot to 0.
  3. Each iteration: loads runtime length from `ptr_slot + 1`, compares counter < length,
     computes element address via `ptr + counter * 8`, loads element through the pointer,
     binds to the loop variable, runs the body, increments counter.
  All operations are runtime instructions — the length and element values are never folded.
- **`tests/e2e.rs`**: Added 10 new tests:
  - 8 compile-and-run: sum, single-element, break-on-first, mixed-indexing (dot product),
    fn-ptr higher-order, called-twice-different-lengths, continue-skip-negative, if-break.
  - 2 assembly inspection: `runtime_for_slice_emits_ldr_and_ptr_arithmetic` (ldr + mul present,
    result not folded to #6); `runtime_for_slice_called_twice_not_folded` (neither call result
    nor combined result folded to a constant).

## Validated
- `cargo test --test e2e -- milestone_194 runtime_for_slice` — 10 passed
- `cargo test` — 1684 passed (was 1674, +10), 0 failed
- `cargo clippy -- -D warnings` — clean
- PR #223 created: https://github.com/libliflin/galvanic/pull/223

## FLS Notes
- **FLS §6.15.1 AMBIGUOUS**: The spec desugars `for x in s` to
  `IntoIterator::into_iter(s)`, which requires trait dispatch. Galvanic special-cases
  `&[T]` at the IR level. A conforming implementation would require an `IntoIterator`
  impl for `&[T]` in the standard library — which galvanic doesn't have.
- **FLS §4.9**: The fat-pointer length at `ptr_slot + 1` is a runtime value. The loop
  condition loads it fresh on every iteration — not cached in a register. This is a
  known performance gap (cache the length before the loop) but is correct.

## Next
- Assembly inspection test for the for-slice feature would be a Claim 79 in `falsify.sh`
  — the two new `runtime_for_slice_*` tests should be added to the falsification fence
  to permanently guard this feature against a future "fold the slice length" regression.
- Alternatively, `&mut [T]` mutable slice parameters to enable in-place mutation,
  or `for x in &arr` syntax (borrowing an array as a slice inline rather than via
  an explicit `&a` at the call site).
