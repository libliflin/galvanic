# Changelog — Cycle 135

## Who This Helps
- **CI / Validation Infrastructure**: The e2e job was failing on Ubuntu because
  `eval_const_expr` had no arm for `ExprKind::Cast`. `compile_and_run` is skipped
  on macOS (no cross tools), so the bug was invisible locally. On Ubuntu with QEMU,
  it actually ran and crashed galvanic. Fixed immediately to unblock the PR.
- **William (researcher)**: `const Y: u8 = X as u8 + 200` now evaluates correctly
  in const item initializers. Cast expressions in const contexts are a natural
  extension of what was already working.

## Observed
- PR #218 CI: `e2e: fail`, `build: pass`, `audit: pass`. The current branch had one
  commit since the previous successful main merge.
- `milestone_190_i32_const_used_in_u8_chain` uses `const Y: u8 = X as u8 + 200`.
  `eval_const_expr` had no `Cast` arm → returned `None` → Y unresolved.
- On macOS: `compile_and_run` returns early (`tools_available()` is false) → test
  passes vacuously. On Ubuntu CI with ARM64 toolchain: `compile_and_run` actually
  assembles and runs → galvanic fails to compile the source → test panics.
- Root cause: `eval_const_expr` coverage gap introduced when the i32-const-in-u8-chain
  test was written with a cast expression.

## Applied
- **`src/lower.rs`**: Added `ExprKind::Cast` arm to `eval_const_expr` (FLS §6.5.10).
  Evaluates the inner expression, then applies narrowing for u8/i8/u16/i16 target
  types (same logic as `narrow_const_value`); wider types are identity in the i32 IR.

## Validated
- `cargo clippy -- -D warnings` — clean
- `cargo test --test e2e` — 1643 passed, 0 failed
- Pushed to `lathe/20260408-144839`; CI re-triggered.

## FLS Notes
- **FLS §6.5.10**: Type cast expressions (`expr as Ty`) are valid in const contexts
  for numeric types. The FLS does not explicitly enumerate which cast forms are
  const-evaluable, but numeric narrowing/widening is the natural interpretation.

## Next
- Promote `fls_fixtures` parse-only tests to end-to-end milestone tests to advance
  through new FLS sections (e.g., slice references, closures with lifetime bounds).

---

# Changelog — Cycle 134

## Who This Helps
- **William (researcher)**: The chained-const case was flagged as untested in the
  previous cycle's "Next" section. `const X: u8 = 200; const Y: u8 = X + 100` must
  yield Y = 44 — but `exit(300) == exit(44)` at the OS level, making this invisible
  to compile-and-run tests. Claim 76 now permanently enforces this with assembly
  inspection and comparison-based runtime tests.
- **CI / Validation Infrastructure**: Claim 76 joins the falsification fence. Any
  future refactor of `narrow_const_value` or the const fixed-point loop that breaks
  chained narrowing will be caught immediately.

## Observed
- Previous cycle's "Next" explicitly flagged: "does `const X: u8 = 200; const Y: u8 = X + 100`
  correctly wrap Y to 44?" The implementation stores the narrowed value of X (200)
  in `const_vals`, then evaluates Y's initializer: fetches X=200, adds 100, gets 300
  in i32, then `narrow_const_value` wraps 300 to 44. This path was correct but untested.
- The failure mode is adversarially subtle: `exit(300) == exit(44)` at the OS level,
  so only assembly inspection (#0x2c vs #0x12c) or comparison-based runtime tests can
  distinguish the correct from buggy behavior.

## Applied
- **`tests/e2e.rs`**: Added 7 new tests:
  - `milestone_190_u8_const_ref_chain_wraps` — Y=44 via check()
  - `milestone_190_u8_const_ref_chain_no_wrap` — Y=15 (no wrap needed)
  - `milestone_190_u8_three_level_const_chain` — X=200→Y=44→Z=94
  - `milestone_190_u16_const_ref_chain_wraps` — 70000 wrapped to 4464
  - `milestone_190_i32_const_used_in_u8_chain` — i32 feeding u8 narrow
  - `runtime_u8_const_chain_ref_emits_correct_loadimm` — assembly inspection
  - `claim_76_u8_chained_const_ref_wraps_at_8_bits` — adversarial (assembly + runtime)
- **`.lathe/claims.md`**: Added Claim 76 with violation conditions and FLS citations.
- **`.lathe/falsify.sh`**: Added Claim 76 block testing the four key tests.

## Validated
- `cargo clippy -- -D warnings` — clean
- `cargo test --test e2e` — 1643 passed (7 new), 0 failed

## FLS Notes
- **FLS §7.1:10**: Every use of a constant is replaced with its value. This applies
  per-item: each const's stored value must be the narrowed result, not the raw i32.
- **FLS §4.1**: Narrow integer types have specific bit-widths; values must be representable.
- **FLS §6.23 AMBIGUOUS**: Overflow in const contexts should be a compile-time error.
  Galvanic wraps instead. The FLS does not explicitly enumerate per-item narrowing
  rules for non-i32 types in const item initializers.

## Next
- Claim 76 closes the chained-narrow-const overflow gap. The underflow analog is also
  untested: `const X: u8 = 10; const Y: u8 = X - 20` should wrap to 246. If
  `narrow_const_value` uses `as u8` cast it handles both directions — but needs a test.
- Alternatively: promote fls_fixtures parse-only tests to end-to-end milestone tests
  to advance through FLS sections.

---

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
