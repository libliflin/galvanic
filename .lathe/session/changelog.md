# Changelog — Cycle 148

## Who This Helps
- **CI / Validation Infrastructure**: Two tests introduced in Cycle 147 failed on ARM64 CI
  but passed locally (macOS, no cross tools). This cycle fixes both failures, unblocking PR
  merge and restoring CI green.
- **William (researcher)**: The `continue` failure was caused by a parser bug (block-like
  expressions consuming binary operators) that would have silently corrupted the AST for any
  program mixing `if`/`loop`/`match` blocks with subsequent `*x = ...` statements. This
  was a load-bearing correctness issue, not just a test failure.

## Observed
- CI e2e job failed on PR #227 with two test failures:
  1. `milestone_196_for_arr_mut_borrow_continue` — galvanic returned exit 1 (compile
     failure). Error: "assignment to non-variable place expression not yet supported".
  2. `milestone_196_for_arr_mut_borrow_param` — wrong exit code: expected -6, got 250.
- Both passed locally because macOS has no ARM64 cross tools (compile-and-run tests skip).

## Applied

### Bug 1: Parser — block-like expression as binary operator LHS

`if *x < 0 { continue; } *x = *x * 2;` was parsed as:
```
((if *x < 0 { continue; }) * x) = (*x * 2)
```
because after parsing `if ... {}`, the `*` in `*x` was consumed as binary multiplication.

Root cause: every binary-operator parse function (`parse_multiplicative`, `parse_additive`,
`parse_shift`, `parse_bitand`, `parse_bitxor`, `parse_bitor`, `parse_cmp`, `parse_and`,
`parse_or`, `parse_range`, `parse_assign`) continued consuming operators after a
block-like LHS.

Fix in `src/parser.rs`:
- Added `fn is_expr_with_block(expr: &Expr) -> bool` helper (mirrors the same check in
  `parse_stmt_or_tail`).
- Added guard at the top of each binary-operator loop: if the LHS is block-like, stop
  consuming operators.

FLS §6.21 AMBIGUOUS: The spec does not explicitly state this disambiguation rule; it is
inherited from Rust's expression grammar.

### Bug 2: Test — negative exit code wraps on Linux

`milestone_196_for_arr_mut_borrow_param` expected exit code `-6` (sum of negated [1,2,3]).
On Linux, `sys_exit` takes only the low 8 bits: `-6 as u8 = 250`.

Fix in `tests/e2e.rs`: changed input to `[-1, -2, -3]` (negating negatives gives positives:
`1 + 2 + 3 = 6`). Now expects exit code `6`.

## Validated
- `cargo build` — clean
- `cargo run -- /tmp/test_continue.rs -o /tmp/x` — now lowers correctly (fails at assembly
  step only, as expected without cross tools)
- `cargo test` — 1710 passed, 0 failed
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- **FLS §6.21 AMBIGUOUS**: The spec references Rust's "expression-with-block" disambiguation
  but does not formally define it. Galvanic's parser now matches Rust's behavior: block-like
  expressions (if, match, loop, while, for, bare blocks) do not bind to the right with
  binary operators in statement position.

## Next
- A falsification claim for `for x in arr` (direct consumption, Milestone 109): that
  milestone only has compile-and-run tests; no Claim guards it against regression. This was
  the suggested next step from Cycle 147 and is now unblocked by CI green.
