# Changelog — Cycle 106

## Who This Helps
- **William (researcher)**: Programs with large negative integer constants (e.g.,
  `-100000`) now produce correct results in comparisons and pattern matches. Without
  this fix, `match x { -100000 => 1, _ => 0 }` where `x` is a parameter carrying
  `-100000` would always take the wildcard arm — a silent correctness failure.
- **Compiler Researchers**: The FLS §2.4.4.1 / §5.2 / §6.5.7 interaction (integer
  literal materialization → pattern comparison → signed 64-bit cmp) is now
  documented and tested. The specific sign-extension invariant is captured in Claim 59.

## Observed
- Cycle 105 changelog noted: "Sign extension (`sxtw`) would be needed for correct
  behavior in comparisons or right-shifts that observe the sign bit — pre-existing
  gap, not introduced here."
- The root cause: `emit_imm32` uses MOVZ+MOVK for values outside `[-65536, 65535]`,
  which zero-extends the 32-bit two's complement bit pattern to 64 bits.
  - `-100000` as i32 → bits = 0xFFFE7960.
  - MOVZ+MOVK produces `x{reg} = 0x00000000FFFE7960` (zero-extended).
  - A function parameter carrying `-100000` was loaded via `neg` (a 64-bit op),
    giving `x{reg} = 0xFFFFFFFFFFFE7960` (sign-extended).
  - `cmp` comparing these two 64-bit values returns NOT-EQUAL — wrong!
- Affected paths: negative literal patterns (`Pat::NegLitInt`), range pattern bounds
  (`LoadImm(*lo as i32)` in `lower.rs`), and const items with large negative values.
- No existing tests covered large negative constants — all 1482 tests passed despite
  the bug being present.

## Applied
- **`src/codegen.rs`**: In `emit_imm32`, added `sxtw x{reg}, w{reg}` after the
  MOVZ+MOVK sequence. This sign-extends the 32-bit bit pattern in `w{reg}` to 64
  bits. For positive large values (bit 31 of the 32-bit pattern = 0), `sxtw` is
  a no-op. For negative large values, it corrects the sign.
  - Cache-line note: the MOVZ+MOVK case grows from 2 instructions (8 bytes) to 3
    (12 bytes), still within one 64-byte cache line.
  - Removed the stale `FLS §6.23: AMBIGUOUS` comment that documented the bug.
  - Updated the `Instr::LoadImm` comment to reflect the fix.
- **`tests/e2e.rs`**: Added 10 M175 tests:
  - 8 compile-and-run tests: negative pattern match (taken/not-taken), if-equality,
    arithmetic, lt/gt comparisons, const item equality, range pattern.
  - 2 assembly inspection tests: `sxtw` is present for large negative constants;
    the match is not constant-folded.
- **`.lathe/claims.md`**: Added Claim 59.
- **`.lathe/falsify.sh`**: Added Claim 59 check.

## Validated
- `cargo build` — clean
- `cargo test --quiet` — 1492 passed; 0 failed (10 new M175 tests all pass)
- `cargo clippy -- -D warnings` — clean
- The new `milestone_175_neg_large_pattern_match_taken` test confirms the bug is
  fixed: it would return 0 (wrong arm) without `sxtw`, returns 1 (correct) with it.

## FLS Notes
- **FLS §2.4.4.1**: The spec specifies that integer literals have i32 type but is
  silent on ARM64 encoding. The MOVZ+MOVK+sxtw sequence is an implementation choice
  required by the 64-bit register model. No ambiguity in the spec — the behavior
  follows from the type system.
- **FLS §5.2**: Literal patterns must match values of the same bit pattern. The spec
  assumes correct value representation — this fix ensures galvanic satisfies it.
- **FLS §6.5.7**: Comparison operators are defined over the value, not the encoding.
  galvanic's use of 64-bit `cmp` requires 64-bit-correct values.

## Next
- Claim 59 closes the large-value integer story (Claims 57+58+59 cover all four
  arithmetic operators with large positives, and Claim 59 covers large negatives).
- The `runtime_rem_emits_sdiv_and_msub` test still uses inline literals without
  function parameters and lacks a negative "not folded" assertion. Claim 60 should
  tighten this, mirroring the parameter-based pattern from Claims 57/58.
- Or: look at the next FLS section after §6.23 — §6.5.3 Error Propagation (`?`
  operator) is the most-used missing operator, but requires Result/Option types.
  A simpler option: §4.1 additional integer types (i8, i16, u8, u16) since only
  i32, u32, i64, usize are currently tested.
