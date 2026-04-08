# Changelog — Cycle 108

## Who This Helps
- **William (researcher)**: The shift operators (`<<`, `>>`) now have adversarial
  parameter-based falsification coverage. The existing tests used inline literals
  (`1 << 3`, `16 >> 2`) with no negative assertions — a weak fence. Claim 61 closes
  the same gap that Claims 57–60 closed for arithmetic operators.
- **Compiler Researchers**: The bitwise/shift operator coverage now has a load-bearing
  claim documenting the signed/unsigned correctness invariant: `i32 >> n` must use
  `asr` (arithmetic, sign-extending), never `lsr` (logical, zero-filling). This
  invariant was enforced in production code but never tested adversarially.

## Observed
- Claims 57–60 (cycles 100–107) added parameter-based falsification for all five
  arithmetic operators (+, *, -, /, %). Shift operators (`<<`, `>>`) were the natural
  next gap: tested by `runtime_shl_emits_lsl_instruction` and `runtime_shr_emits_asr_instruction`,
  but both used inline literals only, and neither had a negative assertion.
- `runtime_shl_emits_lsl_instruction`: `fn main() -> i32 { 1 << 3 }`, positive assertion
  only, no Claim entry. A constant-folding interpreter producing `mov x0, #8` would
  fail the positive assertion, but the gap is not documented in the falsification suite.
- `runtime_shr_emits_asr_instruction`: `fn main() -> i32 { 16 >> 2 }`, positive assertion
  only, no Claim entry. No guard against `lsr` vs `asr` confusion for signed types.

## Applied
- **`tests/e2e.rs`**: Added two adversarial tests:
  - `runtime_shl_emits_lsl_not_folded`: uses `fn shl(x: i32, n: i32) -> i32 { x << n }`
    called as `shl(1, 3)`. Asserts `lsl` in body, `bl shl` at call site. Negative
    assertion: `mov x0, #8` must not appear.
  - `runtime_shr_emits_asr_not_folded`: uses `fn shr_i32(x: i32, n: i32) -> i32 { x >> n }`
    called as `shr_i32(16, 2)`. Asserts `asr` in body, `bl shr_i32` at call site. Negative
    assertion: `mov x0, #4` must not appear. Additional guard: `lsr` must not appear
    (signed shift must be arithmetic, not logical).
- **`.lathe/claims.md`**: Added Claim 61 entry with full rationale, attack vector,
  and FLS citations.
- **`.lathe/falsify.sh`**: Added Claim 61 check running both new tests.

## Validated
- `cargo test --quiet` — 1495 passed; 0 failed (2 new tests)
- `cargo clippy -- -D warnings` — clean
- `runtime_shl_emits_lsl_not_folded` passes: `lsl` present, `bl shl` present, no `mov x0, #8`
- `runtime_shr_emits_asr_not_folded` passes: `asr` present, `bl shr_i32` present, no `mov x0, #4`, no `lsr`

## FLS Notes
- **FLS §6.5.7**: Shift operators. The spec specifies that right-shift on signed
  integers is arithmetic (preserves sign bit). ARM64 `asr` implements this correctly.
  ARM64 `lsr` would zero-fill from the high bit, which is incorrect for signed types.
  The spec is clear here — no ambiguity.
- **FLS §6.1.2 Constraint 1**: Function parameters are not known at compile time;
  shift expressions with parameter operands must emit runtime `lsl`/`asr` instructions.

## Next
- Bitwise operators (`|`, `^`, `&`) and their existing tests also lack negative
  assertions and falsification claims. `runtime_or_emits_orr_instruction` and
  `runtime_xor_emits_eor_instruction` are the next natural targets for Claim 62.
- After closing the bitwise claim gap, the next feature-level target is FLS §4.2
  additional integer types (i8, i16, u8, u16) — byte literals (milestones 90/92)
  already provide the infrastructure.
