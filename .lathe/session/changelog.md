# Changelog — Cycle 103

## Who This Helps
- **William (researcher)**: M174 documents a genuine FLS §6.23 conformance gap.
  Galvanic uses ARM64 64-bit registers for i32 arithmetic, so overflow behavior
  differs from both Rust debug mode (panic) and release mode (32-bit wrap). This
  is the first cycle to explicitly document this as a research output. The §6.23
  AMBIGUOUS annotations in `lower.rs` and `codegen.rs` are now permanent spec notes.
- **FLS / Ferrocene Ecosystem**: One new documented ambiguity: §6.23 does not
  specify the debug/release mode distinction in terms observable from the ARM64
  instruction set. Galvanic's 64-bit register usage makes it non-conforming with
  Rust's wrapping semantics at the 32-bit boundary.
- **Compiler Researchers**: The §6.23 annotation in codegen.rs now explains exactly
  why the `add x{dst}, x{lhs}, x{rhs}` instruction is used and what its overflow
  semantics are relative to what the spec requires.

## Observed
- All 1470 e2e tests pass, 56 claims hold, CI clean on previous PR (M173).
- Previous cycle's "Next" explicitly pointed to §6.23 Arithmetic Overflow.
- The §6.23 issue is genuinely interesting: reading lower.rs and codegen.rs revealed
  that galvanic's 64-bit register usage means overflow at i32::MAX doesn't wrap —
  it just produces a large positive 64-bit value. This is different from both Rust
  modes and is a real research finding worth documenting.

## Applied
- **`src/lower.rs`**: Added FLS §6.23 AMBIGUOUS block to the runtime arithmetic
  lowering section. Documents the 64-bit register vs i32 semantic mismatch.
- **`src/codegen.rs`**: Updated `add`/`sub`/`mul` instruction comments to reference
  §6.23 and the 64-bit wrapping (not 32-bit) behavior.
- **`tests/fixtures/fls_6_23_overflow.rs`**: New fixture with programs using large
  integer arithmetic, derived from §6.23 semantics.
- **`tests/fls_fixtures.rs`**: Added `fls_6_23_overflow` parse acceptance test.
- **`tests/e2e.rs`**: Added 8 compile-and-run M174 tests + 2 assembly inspection:
  - `runtime_large_int_add_emits_add_not_folded`: large-arg add emits `add`, not constant
  - `runtime_large_int_mul_emits_mul_not_folded`: large-arg mul emits `mul`, not constant
- **`.lathe/claims.md`**: Added Claim 57 documenting the §6.23 overflow behavior.
- **`.lathe/falsify.sh`**: Added Claim 57 adversarial check.

## Validated
- `cargo test --test e2e -- milestone_174 runtime_large_int` — 10 passed
- `cargo test` — 1480 e2e (was 1470), 211 unit, 30 fixture (was 29), 1 smoke; 0 failed
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- **FLS §6.23 AMBIGUOUS**: The spec specifies integer overflow as debug-mode panic
  or release-mode 32-bit two's complement wrap. Galvanic uses ARM64 64-bit registers
  for all i32 arithmetic. A value exceeding i32::MAX stays positive in a 64-bit
  register rather than wrapping to i32::MIN. This is non-conforming with both modes.
  The spec does not say anything about the 32-bit vs 64-bit register width used by
  the implementation — this is an implementation detail not covered by FLS §6.23.
- **FLS §6.23 AMBIGUOUS**: The spec does not specify the mechanism for raising a
  runtime panic (required in debug mode). In a no_std ARM64 environment without
  libc, there is no standard panic runtime. FLS §6.23 is silent on this.

## Next
- M175: `unsafe impl<T> where T: Bound` — where-clause form of bounded unsafe impl
  (parser already supports `parse_where_clause` in impl blocks, so pure tests).
- Or: §6.5.3 Negation operator for non-i32 integer types (u32, i64, usize) —
  documenting operator coverage gaps for unsigned negation (FLS §6.5.3 says
  negation is only defined for signed integers; u32 negation is wrapping).
- Or: Deepening existing milestone assembly inspection coverage for milestones
  that only have compile-and-run tests (e.g., early milestones M1–M30).
