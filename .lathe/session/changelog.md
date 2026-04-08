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
