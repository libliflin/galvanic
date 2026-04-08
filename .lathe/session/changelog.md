# Changelog — Cycle 110

## Who This Helps
- **William (researcher)**: Unary operators (`-`, integer `!`, bool `!`) now have
  adversarial parameter-based falsification coverage (Claim 63). The existing tests
  for neg and bitwise NOT lacked call-site `bl` assertions and had no Claim entries —
  a constant-folding interpreter that inlined and eliminated the call could pass them.
- **Compiler Researchers**: The unary operator coverage now matches the rigor
  established for binary operators (Claims 57–62). Every operator class with a
  computable result now has a load-bearing claim.

## Observed
- Cycle 109 "Next" identified boolean NOT and unary negation as the next gap.
- `runtime_neg_emits_neg_instruction` (line 1185): uses `fn negate(x: i32) -> i32 { -x }` —
  already parameter-based, has positive assertion, but NO `bl` call-site check and
  NO negative assertion for the folded result. No Claim entry.
- `runtime_not_emits_mvn_instruction` (line 1706): uses `fn main() -> i32 { !5 }` —
  inline literal in main, not a function parameter. Weak fence.
- `runtime_bool_not_emits_eor_instruction` (line 1729): uses a parameter, has a
  partial negative assertion (`!mvn`), but no `bl` call-site check and no Claim entry.

## Applied
- **`tests/e2e.rs`**: Added three adversarial tests (after line 1740):
  - `runtime_neg_emits_neg_not_folded`: `fn neg_i32(x: i32) -> i32 { -x }` called
    as `neg_i32(5)`. Asserts `neg` in body, `bl neg_i32` at call site. Negative:
    `mov x0, #-5` must be absent.
  - `runtime_not_emits_mvn_not_folded`: `fn bitwise_not(x: i32) -> i32 { !x }` called
    as `bitwise_not(5)`. Asserts `mvn` in body, `bl bitwise_not` at call site. Negative:
    `mov x0, #-6` must be absent.
  - `runtime_bool_not_emits_eor_not_folded`: `fn bool_not(b: bool) -> bool { !b }` called
    via `if bool_not(true) { 1 } else { 0 }`. Asserts `eor #1` in body, `bl bool_not`
    at call site. Negative: `mvn` must be absent.
- **`.lathe/claims.md`**: Added Claim 63 with full rationale, attack vector, and
  FLS citations.
- **`.lathe/falsify.sh`**: Added Claim 63 check running all three new tests.

## Validated
- `cargo test --test e2e -- runtime_neg_emits_neg_not_folded runtime_not_emits_mvn_not_folded runtime_bool_not_emits_eor_not_folded` — 3 passed
- `cargo test --quiet` — 1501 passed; 0 failed (3 new tests)
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- **FLS §6.5.4**: Negation operator expressions. The spec defines `-` on integers
  and `!` on integers and bools but is silent on ARM64 mnemonics (`neg`, `mvn`, `eor`).
  The ARM64 encoding choice is implementation-defined within the spec. No ambiguity.
- **FLS §6.1.2 Constraint 1**: Confirmed — function parameters are not const contexts;
  unary expressions with parameter operands must emit runtime instructions.

## Next
- All five arithmetic operators, both shift operators, all three bitwise operators,
  and all three unary operators now have load-bearing claims (57–63).
- The next natural gap: FLS §4.2 additional integer types (i8, i16, u8, u16).
  Only i32, u32, i64, usize are currently fully tested. Byte literal infrastructure
  (milestones 90/92) already exists. u8 arithmetic would be a natural next milestone.
