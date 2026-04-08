# Changelog — Cycle 133

## Who This Helps
- **William (researcher)**: The const evaluator was silently wrong for narrow integer
  const items. `const X: u8 = 200 + 100` was storing 300 instead of 44. This bug
  would have been invisible to exit-code tests (OS truncates exit codes to 8 bits,
  so exit(300) == exit(44)). Only assembly inspection exposes it. Claim 75 now
  permanently guards this behaviour with both assembly inspection and a comparison-based
  runtime test that can actually distinguish the two.
- **CI / Validation Infrastructure**: Claim 75 in `falsify.sh` ensures narrow-type
  const wrapping is permanently on the falsification fence.

## Observed
- Cycle 132 changelog's Next section flagged: "narrow integer types in const items.
  Does `const X: u8 = 200 + 100` wrap to 44? galvanic's const evaluator uses i32
  arithmetic throughout — it does NOT apply truncation for u8 const items."
- Confirmed: `eval_const_expr` returns `Option<i32>` with all arithmetic in i32.
  `const Z: u8 = 200 + 100` stored `300` in `const_vals`, emitting `LoadImm(300)`.
- Bug was invisible to exit-code runtime tests: `exit(300) == exit(44)` at the OS
  level. Assembly inspection (`#0x12c` vs `#0x2c`) and comparison-based runtime
  tests (`if v == 44 { 1 } else { 0 }`) are the only reliable detectors.

## Applied
- **`src/lower.rs`**: Added `narrow_const_value(raw: i32, ty: &Ty, source: &str) -> i32`
  that casts through the declared narrow type before storage (`u8`, `i8`, `u16`,
  `i16`; `i32` and wider pass through unchanged). Called from the const fixed-point
  loop immediately after `eval_const_expr` resolves a value.
- **`tests/e2e.rs`**: Added 11 new tests:
  - 8 milestone_189 tests (u8/i8/u16 const wrapping, adversarial comparison logic)
  - 2 `runtime_*_wraps_emits_correct_loadimm` assembly inspection tests
  - 1 `claim_75_u8_const_item_wraps_at_8_bits_not_i32` (assembly + runtime)
- **`.lathe/claims.md`**: Added Claim 75 with violation conditions and FLS citations.
- **`.lathe/falsify.sh`**: Added Claim 75 block.

## Validated
- `cargo clippy -- -D warnings` — clean
- `cargo test` — 1636 passed (11 new), 0 failed

## FLS Notes
- **FLS §6.23 AMBIGUOUS**: The FLS says overflow in const contexts is a compile-time
  error, but does not enumerate which narrowing rules apply to non-i32 const items
  specifically. Galvanic wraps silently (rather than erroring) as a pragmatic choice.
- **FLS §4.1**: Narrow integer types have specific bit-widths. `narrow_const_value`
  enforces this at const item evaluation time.

## Next
- The falsification fence covers narrow integer const items. Next adversarial target:
  do narrow integer const items referenced by name work correctly as operands in OTHER
  const items? `const X: u8 = 200; const Y: u8 = X + 100` — does Y correctly wrap
  to 44? The implementation stores the narrowed value of X (200), then computes
  `200 + 100 = 300` in i32, then narrows Y to 44. Should work, but a test
  verifying it explicitly would be Claim 76.
  Alternatively: check which `fls_fixtures` parse-only tests could now be promoted
  to end-to-end milestone tests — that forward progress is higher value than more
  red-team coverage in the same area.
