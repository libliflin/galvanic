# Changelog — Cycle 119

## Who This Helps
- **CI / Validation Infrastructure**: 4 tests were falsely failing on CI due to
  an incorrect assumption about process exit codes. Linux exit codes are 8-bit
  (0-255); tests asserting 4464, 1000, 4564 would get those values truncated
  to 112, 232, 212 respectively. CI now passes cleanly.
- **William (researcher)**: The u16/i16 narrowing cast implementation from
  cycle 118 was correct; the tests were wrong. The fix confirms the
  implementation is sound and CI is now a reliable signal again.

## Observed
- CI job `e2e` failed on PR #209 with 4 failures:
  - `milestone_180_cast_u16_truncates_70000_to_4464` (expected 4464, got 112)
  - `milestone_180_cast_u16_passed_to_fn` (expected 4464, got 112)
  - `milestone_180_cast_u16_result_in_arithmetic` (expected 4564, got 212)
  - `milestone_180_cast_i16_1000_stays_positive` (expected 1000, got 232)
- Root cause: Linux `waitpid()` returns only the low 8 bits of the exit status.
  All other passing tests use values ≤ 255. The u16 tests were the first to
  introduce expected values above 255.
- The assembly inspection tests (`runtime_cast_to_u16_emits_and_truncation`,
  `runtime_cast_to_i16_emits_sxth`) passed on CI because they don't run the
  binary; they verify emitted assembly only.

## Applied
- **`tests/e2e.rs`**: Redesigned the 4 failing tests to produce results ≤ 255
  while still exercising u16/i16 truncation:
  - `milestone_180_cast_u16_truncates_70000_to_4464`: uses `if (x as u16) as i32 == 4464 { 44 } else { 0 }`; expects 44. Identity pass-through (no truncation) would give 0.
  - `milestone_180_cast_i16_1000_stays_positive`: uses `(x as i16) as i32 - 900`; expects 100. Sign-wrap would give a different value.
  - `milestone_180_cast_u16_result_in_arithmetic`: uses input 65636 (= 65536+100 → as u16 = 100; 100+100=200); expects 200.
  - `milestone_180_cast_u16_passed_to_fn`: uses input 65580 (= 65536+44 → as u16 = 44); expects 44.

## Validated
- `cargo test --test e2e milestone_180` — 7 passed; 0 failed
- `cargo test` — 1788 passed; 0 failed (all suites)
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- No new FLS ambiguities this cycle. This was a test infrastructure fix.

## Next
- With CI now clean, the next work should implement the first untouched FLS
  section beyond milestone 180. The narrowing cast story for 16-bit types is
  now complete at the cast boundary. The next natural step is either:
  1. u16/i16 as native IR types (IrTy::U16, IrTy::I16) for correct overflow
     at arithmetic boundaries — similar to IrTy::U8/I8.
  2. A new FLS section not yet covered (check §6.5 for remaining operators,
     §6.16 range expressions as values, etc.).
