# Changelog — Cycle 132 (Red-Team)

## Who This Helps
- **William (researcher)**: Claim 74 now prevents the "fails silently" regression
  called out in Cycle 131's Next. If `assoc_known` is ever dropped or mis-threaded
  in `eval_const_expr`, the falsification suite catches it immediately — instead of
  `const LIMIT = i32::MAX` silently resolving to 0 with no error.
- **CI / Validation Infrastructure**: The falsification fence around milestone 188
  is now adversarial. `claim_74_const_chain_through_builtin_assoc_not_zero` tests
  a const-chain (`const DERIVED = LIMIT - 1` where `LIMIT = i32::MAX`) that would
  silently produce exit code 0 if two-segment path evaluation broke.

## Observed
- Cycle 131 changelog explicitly flagged: "Add Claim 74 to `falsify.sh` covering
  `const LIMIT: i32 = i32::MAX` resolving correctly (not failing silently)."
- Claim 73 covered runtime `i32::MAX` expressions but NO claim protected the
  distinct code path: `eval_const_expr` with `assoc_known` threaded into the const
  fixed-point evaluator.
- The failure mode: removing `assoc_known` from `eval_const_expr` would leave
  `const LIMIT = i32::MAX` resolving to `None` → fixed-point loop silently skips
  LIMIT → all uses produce 0 → programs that test `LIMIT > 0` return exit 0
  with no compiler error.

## Applied
- **`tests/e2e.rs`**: Added `claim_74_const_chain_through_builtin_assoc_not_zero`:
  - Assembly inspection on a leaf function `fn main() -> i32 { DERIVED }` (no
    parameter spilling, so the negative assertion `!ldr x0, [sp` is unambiguous).
  - Positive assertion: `#0xfffe` appears (2147483646 = 0x7FFFFFFE low-half).
  - Negative assertion: no `ldr x0, [sp` (constant must be LoadImm, not stack slot).
  - Runtime check (separate source with `check(DERIVED)` fn): must return 1, not 0.
- **`.lathe/claims.md`**: Added Claim 74 with full violation conditions, ARM64
  implementation notes, and FLS citations.
- **`.lathe/falsify.sh`**: Added Claim 74 block running the new test plus four
  milestone 188 tests that cover adjacent paths.

## Validated
- `cargo test` — 1625 passed (1 new), 0 failed
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- **FLS §7.1:10**: Const items must be fully evaluated before first use. The
  fixed-point evaluator ordering between `const A = i32::MAX` and `const B = A-1`
  is an implementation choice; the FLS only requires the result to be available.
- **FLS §10.3**: Associated constant paths are valid in all constant expression
  contexts, including const item initializers.
- **FLS §4.1 AMBIGUOUS**: `MAX`/`MIN` are language convention, not enumerated
  by name in the FLS. Already noted in Claim 73; Claim 74 inherits this caveat.

## Next
- The next adversarial target: narrow integer types in const items. Does
  `const X: u8 = 200; const Y: u8 = X + 100;` wrap to 44? Currently galvanic's
  const evaluator uses `i32` arithmetic throughout — it likely does NOT apply
  truncation for `u8` const items. This would be Claim 75 (and a real bug to fix).
