# Changelog — Cycle 136

## Who This Helps
- **William (researcher)**: Claim 75 (cycle 133) only tested addition wrapping for
  u8 const items. Subtraction underflow and multiplication overflow were explicitly
  flagged in the cycle 133 "Next" section as untested. Claim 77 now closes that gap
  permanently. Any future refactor of `narrow_const_value` that accidentally breaks
  subtraction or multiplication narrowing will be caught immediately — before CI.
- **CI / Validation Infrastructure**: 8 new tests and 1 new falsification block.
  The fence now covers all three arithmetic directions for narrow const items.

## Observed
- Cycle 133 "Next" section explicitly flagged: "The underflow analog is also untested:
  `const X: u8 = 10; const Y: u8 = X - 20` should wrap to 246."
- Claim 75 (`falsify.sh`) only runs `claim_75_u8_const_item_wraps_at_8_bits_not_i32`
  which uses `200 + 100 = 300 → 44`. No subtraction or multiplication falsification.
- The adversarial failure mode: if `narrow_const_value` were changed to clamp
  negative raw values to 0 (e.g., `if raw >= 0 { raw as u8 as i32 } else { 0 }`),
  `const X: u8 = 5 - 10` would yield 0 instead of 251. Claim 75 would still pass
  (it only tests addition). This cycle adds the falsification fence for that scenario.

## Applied
- **`tests/e2e.rs`**: Added 8 new tests:
  - `milestone_191_u8_const_sub_underflow` — 5-10 wraps to 251 (compile_and_run)
  - `milestone_191_u8_const_mul_wraps` — 100*3 wraps to 44 (compile_and_run)
  - `milestone_191_u8_const_chained_sub_underflow` — X=10; Y=X-20 → 246 (compile_and_run)
  - `milestone_191_i8_const_sub_underflow` — -100-50 wraps to 106 (compile_and_run)
  - `runtime_u8_const_sub_underflow_emits_loadimm_251` — assembly inspection (#251 not #0)
  - `runtime_u8_const_mul_wrap_emits_loadimm_44` — assembly inspection (#44 not #300)
  - `runtime_u8_const_chained_sub_emits_loadimm_246` — assembly inspection (#246 not #0)
  - `claim_77_u8_const_sub_and_mul_wrap_not_saturate` — adversarial (asm + runtime)
- **`.lathe/claims.md`**: Added Claim 77 with three adversarial failure modes and
  FLS citations.
- **`.lathe/falsify.sh`**: Added Claim 77 block covering all 6 key tests.

## Validated
- `cargo clippy -- -D warnings` — clean
- `cargo test --test e2e` — 1651 passed (8 new), 0 failed
- `.lathe/falsify.sh` — 76 passed, 0 failed

## FLS Notes
- **FLS §6.23 AMBIGUOUS**: Underflow in const contexts should be a compile-time error.
  Galvanic wraps instead (`-5 as u8 = 251`). The spec does not enumerate which
  narrowing/wrapping rules apply to non-i32 const items specifically.
- **FLS §4.1**: Narrow integer types have specific bit-widths. `narrow_const_value`
  uses Rust's two's-complement `as u8` cast which handles both positive overflow
  (300→44) and negative underflow (-5→251) correctly and uniformly.

## Next
- The const item falsification fence now covers add/sub/mul wrapping. Division and
  bitwise operations cannot produce out-of-range results for u8/i8 inputs, so
  add/sub/mul are the only operations that matter here.
- The natural next gap: promote `fls_fixtures` parse-only tests to end-to-end
  milestone tests. The frontier is currently at milestone 191 (red-team cycles).
  Looking at the FLS TOC, §6.16 (range expressions), §6.18 (match exhaustiveness),
  or §15 (ownership/drop) could be the next feature work cycle.
