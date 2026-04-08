# Changelog — Cycle 130

## Who This Helps
- **William (researcher)**: After 5 cycles of hardening narrow integer normalisation,
  the project advances into new FLS territory. Built-in integer type constants (`i32::MAX`,
  `i32::MIN`, `u8::MAX`, `i8::MAX`, etc.) are now recognised. Programs that use boundary
  values for comparisons, guards, and type-cast checks now compile correctly.
- **Compiler Researchers**: The FLS §4.1 AMBIGUOUS note documents that the spec specifies
  value ranges but does not enumerate `MAX`/`MIN` by name. This is a genuine spec gap.

## Observed
- After 5 cycles of defensive work on narrow integer normalisation (Claims 66–72), the
  falsification suite was complete and all tests were passing. The next cycle called for
  advancing to new FLS territory rather than more red-team work on existing features.
- `i32::MAX` in a runtime expression would previously fall through to the enum variant
  lookup in `lower.rs` and fail with "unknown path `i32::MAX` (not an enum variant)".
- This is a real capability gap: boundary-value comparisons (`if x == i32::MAX`) are
  common in correct Rust programs.

## Applied
- **`src/lower.rs`**: Before the loop that populates `assoc_const_vals` from user-defined
  impl blocks, pre-seed the map with 10 built-in integer type constants:
  - `i8::MAX = 127`, `i8::MIN = -128`
  - `u8::MAX = 255`, `u8::MIN = 0`
  - `i16::MAX = 32767`, `i16::MIN = -32768`
  - `u16::MAX = 65535`, `u16::MIN = 0`
  - `i32::MAX = 2147483647`, `i32::MIN = -2147483648`
  - `u32::MAX` and larger types deferred (don't fit in galvanic's i32 IR).
  No other files changed — the existing two-segment path lowering already handles
  `"TypeName::CONST_NAME"` lookups via `assoc_const_vals`.
- **`tests/e2e.rs`**: Added 9 milestone 187 tests (8 compile-and-run + 1 assembly inspection):
  - `milestone_187_i32_max_is_positive`, `milestone_187_i32_min_is_negative`
  - `milestone_187_u8_max_as_i32`, `milestone_187_i8_max_as_i32`, `milestone_187_i8_min_as_i32`
  - `milestone_187_u16_max_as_i32`, `milestone_187_i16_min_as_i32`
  - `milestone_187_i32_max_passed_to_fn` (through a helper fn — not constant-foldable)
  - `runtime_i32_max_emits_loadimm` (assembly inspection: movz+movk, no stack load)
- **`.lathe/claims.md`**: Added Claim 73 documentation.
- **`.lathe/falsify.sh`**: Added Claim 73 (5 tests).

## Validated
- `cargo build` — clean
- `cargo test` — 1616 passed (9 new tests), 0 failed
- `cargo clippy -- -D warnings` — clean
- `bash .lathe/falsify.sh` — 72 passed, 0 failed

## FLS Notes
- **FLS §4.1 AMBIGUOUS**: The spec defines value ranges of primitive integer types
  but does not enumerate `MAX` and `MIN` as named associated constants. These are
  a language convention, not mandated by the FLS itself. Galvanic derives them from
  the stated range bounds.
- **FLS §10.3**: The substitution mechanism is the same for user-defined and built-in
  associated constants. No new IR instructions were needed.
- **Limitation not yet covered**: `i32::MAX` in const item initializers
  (`const LIMIT: i32 = i32::MAX;`) is not yet handled — `eval_const_expr` only handles
  single-segment paths. This is documented in the "Next" section.

## Next
- Extend `eval_const_expr` to handle two-segment built-in constants so that
  `const LIMIT: i32 = i32::MAX;` works in const initializers.
- Or advance to slice references (`&[i32]`) as function parameters with `.len()` access
  — a significant real-Rust capability requiring parser and lowering changes.

---

# Changelog — Cycle 129

## Who This Helps
- **William (researcher)**: Claim 66 previously only tested compound `+=` for narrow
  integer types. Compound `*=` was untested, creating a blind spot: a future refactor
  removing the TruncU8/SextI8/TruncU16/SextI16 normalisation from the mul branch of
  `CompoundAssign` lowering would have silently passed all existing tests while producing
  wrong runtime behavior for `x *= b` where x is a narrow integer.
- **CI / Validation Infrastructure**: Claim 66 now covers all four narrow types (u8, i8,
  u16, i16) for compound mul, with both assembly inspection (normalisation instruction
  present) and compile-and-run (mid-body comparison sees wrapped value) checks.

## Observed
- Claim 66 in `claims.md` promised coverage of `*=` in its title and description
  ("compound assignment wraps correctly mid-body") but its test list only contained
  compound-add tests: `milestone_178_u8/i8_compound_add_wraps_mid_body`.
- Same asymmetry pattern found and fixed last cycle for basic arithmetic (Claims 65/69
  only had add, now have add+mul): compound assignment had the same gap.
- The implementation at `lower.rs:14527-14539` already handles all compound operators
  through the same `BinOp` dispatch — but untested paths are unguarded paths.

## Applied
- **`tests/e2e.rs`**: Added 8 tests (milestone 186):
  - `runtime_u8_compound_mul_emits_trunc_u8`: assembly inspection — `and #255` + `mul` present, no fold
  - `runtime_i8_compound_mul_emits_sext_i8`: assembly inspection — `sxtb` + `mul` present
  - `milestone_186_u8_compound_mul_wraps_mid_body`: 15*20=300 → 44; 44 < 50 → 1
  - `milestone_186_i8_compound_mul_wraps_mid_body`: 12*15=180 → -76 as i8; -76 < 0 → 1
  - `runtime_u16_compound_mul_emits_trunc_u16`: assembly inspection — `and #65535` + `mul` present
  - `runtime_i16_compound_mul_emits_sext_i16`: assembly inspection — `sxth` + `mul` present
  - `milestone_186_u16_compound_mul_wraps_mid_body`: 300*300=90000 → 24464; 24464 < 30000 → 1
  - `milestone_186_i16_compound_mul_wraps_mid_body`: 200*200=40000 → -25536 as i16; -25536 < 0 → 1
- **`.lathe/claims.md`**: Extended Claim 66 title, description, and test list to cover mul
- **`.lathe/falsify.sh`**: Added 8 new tests to Claim 66 block

## Validated
- `cargo build` — clean
- `cargo test` — 1607 passed (8 new tests), 0 failed
- `cargo clippy -- -D warnings` — clean
- `bash .lathe/falsify.sh` — 71 passed, 0 failed

## FLS Notes
- No new ambiguities. FLS §6.23 and §4.1 already covered by existing commentary.
  The wrapping requirement applies uniformly to all arithmetic operators in compound
  assignment — the spec does not distinguish `+=` from `*=` in this regard.

## Next
- The compound-assignment wrapping story is now symmetric for mul across all narrow
  types. Next candidate: advance to FLS §6.8 array expressions with non-const lengths,
  or §6.12.2 method calls on slice references (`s.len()`). These would move the frontier
  forward from the current milestone 185 position.
