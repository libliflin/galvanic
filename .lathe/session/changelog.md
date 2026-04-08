# Changelog — Cycle 116

## Who This Helps
- **William (researcher)**: A real latent codegen bug is fixed and fenced. Programs
  using `u8`/`i8` compound assignment (`+=`, `*=`, etc.) now produce correct
  wrapped values when the variable is read back mid-body — not only at function
  return boundaries.
- **CI / Validation Infrastructure**: Claim 66 and its four tests are now part of
  `falsify.sh`, so this class of bug cannot silently regress.

## Observed
- Previous cycle's changelog noted: "u8/i8 compound assignment (`+=`, `*=`) —
  TruncU8/SextI8 is not emitted for compound-assignment paths."
- Inspected `ExprKind::CompoundAssign` in `src/lower.rs`. The handler emits
  `Load → BinOp → Store` with no narrow-type normalization. TruncU8/SextI8
  was only applied at function return boundaries.
- Adversarial test constructed: `fn test(a: u8, b: u8) -> i32 { let mut x: u8 = a; x += b; if x < 50 { 1 } else { 0 } }` called with (200, 100).
  - Without fix: `x` slot held 300; `300 < 50` → false → returns 0.
  - After fix: `x` slot holds 44 (300 & 255); `44 < 50` → true → returns 1.
- Same bug affected `i8`: `100_i8 += 50_i8` left the slot with 150 (unsigned),
  making `x < 0` false when it should be true (wrapped to -106).

## Applied
- **`src/lower.rs`**:
  - Added `u8_locals: HashSet<&str>` and `i8_locals: HashSet<&str>` to `LowerCtx`.
  - In parameter registration: u8/i8 parameters are now registered in the respective
    set alongside `locals`, parallel to the existing `local_fn_ptr_slots` tracking.
  - In let binding handler: detects `u8` and `i8` type annotations and registers
    variable names. Falls through to existing init lowering (no duplicate code).
  - In `ExprKind::CompoundAssign`: after `Instr::BinOp`, checks if the target
    variable is in `u8_locals` (emit `TruncU8`) or `i8_locals` (emit `SextI8`)
    before `Instr::Store`. Applies only to simple path targets (the field-access
    and deref compound-assignment paths are handled separately above).
- **`tests/e2e.rs`**:
  - `runtime_u8_compound_add_emits_trunc_u8`: assembly inspection — `and #255`
    must be present in the compound-assignment function body.
  - `runtime_i8_compound_add_emits_sext_i8`: assembly inspection — `sxtb` must
    be present in the compound-assignment function body.
  - `milestone_178_u8_compound_add_wraps_mid_body`: compile-and-run — adversarial
    mid-body comparison after u8 compound add.
  - `milestone_178_i8_compound_add_wraps_mid_body`: compile-and-run — adversarial
    mid-body comparison after i8 compound add.
- **`.lathe/claims.md`**: Claim 66 added.
- **`.lathe/falsify.sh`**: Claim 66 check added.

## Validated
- `cargo build` — clean
- `cargo test` — 1770 passed; 0 failed (1528 e2e + 211 unit + 30 fls_fixtures + 1 smoke)
- `cargo clippy -- -D warnings` — clean
- `bash .lathe/falsify.sh` — 65 passed, 0 failed

## FLS Notes
- **FLS §6.23**: The spec says wrapping semantics apply at runtime (in the Rust
  release-mode sense). The spec does not explicitly say "at every store" vs "at
  the point of use" — but the semantics require the variable's VALUE to be in
  range whenever it is read. Galvanic's approach of normalizing at the store
  (compound-assignment path) matches this: the slot always holds an in-range value.
- **FLS §4.1, §6.23 AMBIGUOUS**: The spec describes the type's value range but
  does not specify the implementation mechanism (normalize on write vs. normalize
  on read). Galvanic chooses normalize-on-write for consistency with the existing
  function-return-boundary approach.

## Next
- Regular assignment `x = expr` where `x: u8` does NOT apply TruncU8 at the
  store site. This is a known remaining gap: `let mut x: u8 = 255; x = 300;`
  would leave 300 in the slot. Fixing this requires the same `u8_locals` lookup
  in the `Assign` handler.
- The `ptr_capture_slots` compound-assignment path (mutable closure captures)
  also does not apply narrow-type wrapping. This path is unlikely to be exercised
  with u8/i8 captures at current milestones.
