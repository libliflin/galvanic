# Changelog — Cycle 104

## Who This Helps
- **William (researcher)**: Claim 58 closes the arithmetic-operator coverage gap in
  the falsification suite. Previously Claim 57 only defended `add` and `mul` for
  large-value non-folding; `sub` and `div` were tested only with tiny literals in
  `fn main()` (no function parameters), which is a weaker adversarial pattern.
  A folding interpreter that special-cased addition and multiplication but evaluated
  subtraction or division at compile time would pass all prior claims. It cannot
  pass Claim 58.
- **Compiler Researchers**: The four basic arithmetic operators now have symmetric
  adversarial coverage. The pattern (function-parameter input → assert instruction
  emitted + assert result not folded) is consistently applied across `add`, `mul`,
  `sub`, and `sdiv`.

## Observed
- Claim 57 (cycle 103) tested large-value `add` and `mul` with the parameter-pattern.
- `runtime_sub_emits_sub_instruction` used `fn main() -> i32 { 10 - 3 }` — no
  function parameters, smaller values. A constant-folding pass would fold this but
  there's no negative assertion on the folded literal.
- `runtime_div_emits_sdiv` used `fn main() -> i32 { 10 / 2 }` — also no parameters,
  only one negative assertion (`!asm.contains("mov x0, #5")`), but still weaker than
  the parameter-pattern used for Claim 57.
- Division is the most expensive arithmetic operator and the one most tempting to
  evaluate at compile time when inputs are statically known. It had the weakest
  non-folding assertion.

## Applied
- **`tests/e2e.rs`**: Added two assembly inspection tests:
  - `runtime_large_int_sub_emits_sub_not_folded`: `fn f(x: i32, y: i32) -> i32 { x - y }`
    called as `f(2000000000, 1)`; asserts `sub` emitted, `bl` emitted, `1999999999` not present.
  - `runtime_large_int_div_emits_sdiv_not_folded`: `fn f(x: i32, y: i32) -> i32 { x / y }`
    called as `f(2000000000, 4)`; asserts `sdiv` emitted, `bl` emitted, `500000000` not present.
- **`.lathe/claims.md`**: Added Claim 58 with full rationale.
- **`.lathe/falsify.sh`**: Added Claim 58 adversarial check.

## Validated
- `cargo test --test e2e -- runtime_large_int_sub_emits_sub_not_folded runtime_large_int_div_emits_sdiv_not_folded` — 2 passed
- `cargo test --quiet` — 1484 e2e, all pass; 211 unit; 30 fixture; 1 smoke; 0 failed
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- No new FLS ambiguities discovered. Claim 58 exercises the same FLS §6.1.2 and §6.5.5
  constraints as Claim 57, applied to the remaining operators.

## Next
- The four basic arithmetic operators now have symmetric adversarial coverage.
- Remaining potential gap: `rem` (`%`) with large values — `runtime_rem_emits_sdiv_and_msub`
  uses inline literals (no parameter pattern). Could add Claim 59.
- Or: M175 — `unsafe impl<T> where T: Bound` where-clause form (parser already
  supports where clauses in impl blocks; would be a pure test milestone).
- Or: §6.5.3 Negation operator for u32 (unsigned negation; FLS §6.5.3 is silent).
