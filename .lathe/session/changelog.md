# Changelog — Cycle 112

## Who This Helps
- **William (researcher)**: Claim 64's falsification coverage was limited to
  `add`. A regression where `TruncU8` stops being emitted for `mul` (but not
  `add`) would have slipped through the falsification suite silently. Now the
  fence prevents operator-specific regressions in u8 wrapping.
- **Compiler Researchers**: The assembly inspection test for `mul_u8` provides
  the same two-layer guarantee that `add_u8` has: runtime codegen (mul instruction
  present) AND wrapping semantics (and #255 present).

## Observed
- Claim 64 in `claims.md` described the violation condition for `add` and `sub`
  but not `mul`. The falsify.sh check only ran `runtime_u8_add_emits_and_truncation`
  — if `TruncU8` was disabled for `mul` while remaining for `add`, the claim
  would report OK but `15_u8 * 20_u8` would return 300 instead of 44.
- `milestone_176` had `u8_add_wraps` and `u8_sub_wraps` compile-and-run tests
  but no `u8_mul_wraps` test and no assembly inspection for mul.
- The lower.rs handler at line 10068 (`IrTy::U32 | IrTy::U8`) uses `IrBinOp::Mul`
  for u8 mul. TruncU8 is emitted at the return boundary. The code is correct —
  but the fence wasn't watching it.

## Applied
- **`tests/e2e.rs`**: Added two tests:
  - `milestone_176_u8_mul_wraps` (compile-and-run): verifies `mul_u8(15, 20) == 44`
    (300 mod 256 = 44)
  - `runtime_u8_mul_emits_and_truncation` (assembly inspection): verifies `mul`
    instruction emitted, `and ... #255` emitted, and result NOT constant-folded
    to `mov x0, #44`
- **`.lathe/claims.md`**: Extended Claim 64 to explicitly document the `mul`
  violation condition alongside `add`. Added `mul`-specific falsify conditions.
- **`.lathe/falsify.sh`**: Extended Claim 64 check to include
  `runtime_u8_mul_emits_and_truncation` and `milestone_176_u8_mul_wraps`.

## Validated
- `cargo test` — 1513 passed (was 1511); 0 failed
- `cargo clippy -- -D warnings` — clean
- `bash .lathe/falsify.sh` — 63 passed, 0 failed (Claim 64 now runs 4 tests)

## FLS Notes
- No new ambiguities. The existing §4.1 / §6.23 notes apply to mul equally:
  the spec requires all u8 arithmetic to wrap at 256 at runtime.

## Next
- i8 support (sign-extending `sxtb` at return boundaries instead of `and #255`).
- u8 compound assignment (`*=` for u8) is also unguarded.
- u16 / i16 are similarly unguarded in the falsification suite.
