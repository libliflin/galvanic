# Changelog — Cycle 109

## Who This Helps
- **William (researcher)**: Bitwise operators (`&`, `|`, `^`) now have adversarial
  parameter-based falsification coverage (Claim 62). The existing tests used inline
  literals with weak or missing negative assertions — a constant-folding interpreter
  could have passed them undetected.
- **Compiler Researchers**: The bitwise operator coverage now matches the rigor
  established for arithmetic (Claims 57–60) and shift operators (Claim 61). All
  binary operators with computable results now have a load-bearing claim.

## Observed
- Cycle 108 closed the gap for shift operators (Claim 61). The "Next" section
  identified bitwise operators as the immediate follow-on.
- `runtime_or_emits_orr_instruction`: `fn main() -> i32 { 5 | 3 }` — positive
  assertion only, no negative assertion, no Claim entry.
- `runtime_xor_emits_eor_instruction`: `fn main() -> i32 { 5 ^ 3 }` — positive
  assertion only, no negative assertion, no Claim entry.
- `runtime_and_emits_and_instruction`: `fn main() -> i32 { 5 & 3 }` — had a
  negative assertion for `mov x0, #1`, but still used inline literals (not
  function parameters) and had no Claim entry.

## Applied
- **`tests/e2e.rs`**: Added three adversarial tests:
  - `runtime_and_emits_and_not_folded`: `fn bitand(x: i32, y: i32) -> i32 { x & y }`
    called as `bitand(5, 3)`. Asserts `and` in body, `bl      bitand` at call site.
    Negative: `mov     x0, #1` must be absent.
  - `runtime_or_emits_orr_not_folded`: `fn bitor(x: i32, y: i32) -> i32 { x | y }`
    called as `bitor(5, 3)`. Asserts `orr` in body, `bl      bitor` at call site.
    Negative: `mov     x0, #7` must be absent.
  - `runtime_xor_emits_eor_not_folded`: `fn bitxor(x: i32, y: i32) -> i32 { x ^ y }`
    called as `bitxor(5, 3)`. Asserts `eor` in body, `bl      bitxor` at call site.
    Negative: `mov     x0, #6` must be absent.
- **`.lathe/claims.md`**: Added Claim 62 with full rationale, attack vector, and
  FLS citations.
- **`.lathe/falsify.sh`**: Added Claim 62 check running all three new tests.

## Validated
- `cargo test --quiet` — 1498 passed; 0 failed (3 new tests)
- `cargo clippy -- -D warnings` — clean
- All three adversarial tests pass: correct instructions emitted, folded constants absent

## FLS Notes
- **FLS §6.5.6**: Bitwise expression operators. The spec defines `&`, `|`, `^`
  without specifying the ARM64 mnemonics (`and`, `orr`, `eor`). No ambiguity in
  semantics — the ARM64 encoding choice is implementation-defined within the spec.
- **FLS §6.1.2 Constraint 1**: Confirmed — function parameters are not const contexts;
  bitwise expressions with parameter operands must emit runtime instructions.

## Next
- All five arithmetic operators (+, -, *, /, %) have Claims 57–60.
- Both shift operators (<<, >>) have Claim 61.
- All three bitwise operators (&, |, ^) now have Claim 62.
- The next natural gap: boolean NOT (`!`) and unary negation (`-`) are tested by
  `runtime_bool_not_emits_eor_instruction` and `runtime_neg_emits_neg_instruction`
  respectively — both use inline literals with no parameter-based tests and no Claims.
- After unary operators, FLS §4.2 additional integer types (i8, i16, u8, u16)
  are the next feature-level target.
