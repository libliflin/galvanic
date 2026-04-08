# Changelog — Cycle 154

## Who This Helps
- **William (researcher)**: The existing assembly inspection test for range-based for loops
  used literal bounds (`0..5`). A constant-folding interpreter with special-cased literal
  ranges would still pass it. The new test uses a function parameter (`0..n`) — truly
  unknown at compile time — and asserts both that runtime control flow is emitted AND
  that the folded result `mov x0, #10` is absent. If the loop were ever optimized into
  an interpreter pass, this test would catch it.
- **CI / Validation Infrastructure**: `runtime_for_loop_param_bound_emits_runtime_control_flow_not_folded`
  is now a load-bearing assembly inspection test for the for-loop/range feature (milestone 19),
  complementing the existing `runtime_for_loop_emits_cmp_cbz_add_and_back_branch`.

## Observed
- `milestone_19_for_loop_runtime_bound` (compile-and-run) uses `fn sum_to(n: i32)` with
  `for i in 0..n` — a parameter-bound loop — but had no assembly inspection counterpart.
- `runtime_for_loop_emits_cmp_cbz_add_and_back_branch` uses literal bounds `0..5`. This
  is a good test, but an interpreter that special-cases literal ranges could still pass it.
- The previous cycle's "Next" explicitly flagged this gap.

## Applied
- **`tests/e2e.rs`**: Added `runtime_for_loop_param_bound_emits_runtime_control_flow_not_folded`
  immediately after the existing literal-bound for-loop inspection test.
  - Source: `fn sum_to(n: i32) -> i32 { let mut acc = 0; for i in 0..n { acc += i; } acc }`
  - Positive assertions: `cbz` (loop exit), `add` (body/counter), `b ` (back-edge)
  - Negative assertion: `mov     x0, #10` must NOT appear — that would be the constant-folded
    result of `sum_to(5) = 10`, which is impossible to compute without knowing `n` at compile time.

Files: `tests/e2e.rs`

## Validated
- `cargo test --test e2e -- runtime_for_loop_param_bound_emits_runtime_control_flow_not_folded` → 1 passed, 0 failed
- `cargo test` → 1968 passed, 0 failed
- `cargo clippy -- -D warnings` → clean

## FLS Notes
- No new ambiguities discovered. FLS §6.15.1 (for loop expressions) and §6.16 (range expressions)
  are well-specified for this case. The only implementation detail is the specific ARM64 instructions
  emitted (cbz vs cmp+beq), which the FLS does not prescribe.

## Next
- Check whether the inclusive range for loop (`for i in 0..=n` with parameter `n`) has assembly
  inspection coverage. The current inspection tests only cover exclusive range `0..n` and literal
  `0..5`. An inclusive range encodes a different comparison (≤ vs <), and a parameter-bound
  inclusive range test would complete the range-for-loop coverage picture.
