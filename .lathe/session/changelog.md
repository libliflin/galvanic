# Changelog — Cycle 128 (Red-Team)

## Who This Helps
- **William (researcher)**: The narrow integer wrapping story was asymmetric — u8 had
  both add AND mul tested (Claim 64), but i8 only had add (Claim 65), and i16 only had
  add (Claim 69). A future refactor that accidentally dropped SextI8/SextI16 from the
  mul lowering path would have passed every existing test while silently producing wrong
  runtime behavior for i8 * i8 and i16 * i16 operations.
- **CI / Validation Infrastructure**: Claims 65 and 69 now cover the full arithmetic
  surface (add, sub, mul) for i8 and i16. The falsification fence is symmetric with the
  u8 coverage in Claim 64.

## Observed
- Claims 64, 65, 69 cover narrow integer arithmetic, but asymmetrically:
  - Claim 64 (u8): tests `runtime_u8_add_emits_and_truncation` AND `runtime_u8_mul_emits_and_truncation`
  - Claim 65 (i8): tests only `runtime_i8_add_emits_sxtb_sign_extension` — no mul test
  - Claim 69 (u16/i16): tests u16/i16 add — no i16 mul test (u16 mul existed as a milestone
    test but was not wired into the falsification claim)
- `milestone_176_u8_mul_wraps` exists; `milestone_181_u16_mul_wraps` exists. But
  `milestone_177_i8_mul_wraps` and `milestone_181_i16_mul_wraps` did not exist.
- The lowering code in `lower.rs:5427` shows `BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div`
  in the narrow-type dispatch, so the implementation covers mul — but without tests, a
  future refactor could silently drop mul from the pattern match.

## Applied
- **`tests/e2e.rs`**: Added 4 tests:
  - `milestone_177_i8_mul_wraps`: compile-and-run, `15_i8 * 20_i8 = 300 → 44`
  - `runtime_i8_mul_emits_sxtb_sign_extension`: assembly inspection — mul + sxtb present, no folding
  - `milestone_181_i16_mul_wraps`: compile-and-run, `200_i16 * 200_i16 = 40000 → -25536`
  - `runtime_i16_mul_emits_sxth_sign_extension`: assembly inspection — mul + sxth present, no folding
- **`.lathe/claims.md`**: Extended Claims 65 and 69 with mul violation cases and updated test lists
- **`.lathe/falsify.sh`**: Extended Claim 65 to include `runtime_i8_mul_emits_sxtb_sign_extension`
  and `milestone_177_i8_mul_wraps`; extended Claim 69 to include `runtime_i16_mul_emits_sxth_sign_extension`,
  `milestone_181_u16_mul_wraps`, and `milestone_181_i16_mul_wraps`

## Validated
- `cargo build` — clean
- `cargo test` — 1603 passed (4 new tests added); 0 failed
- `cargo clippy -- -D warnings` — clean
- `bash .lathe/falsify.sh` — 71 passed, 0 failed

## FLS Notes
- No new ambiguities. The narrow-integer mul path was already implemented correctly
  (FLS §6.23, §4.1 coverage unchanged) — this cycle proves it with tests.
- The asymmetry between u8 (mul tested) and i8/i16 (mul untested) was a process gap,
  not an implementation gap. But process gaps become implementation gaps after refactors.

## Next
- Consider adding i8/i16 **compound assignment wrapping** for mul (`x *= b` where x: i8)
  — Claim 66 only tests compound add, not compound mul. Same asymmetry pattern.
- Or advance to next FLS section: candidates are §6.8 array expressions with non-const
  length, or §6.12.2 method calls on slice references.

---

# Changelog — Cycle 127

## Who This Helps
- **William (researcher)**: Cycle 126 claimed to implement narrow integer normalisation
  for named enum variant fields, but added zero tests for that path. Milestone 185
  closes the gap: 9 adversarial tests now guard the named-variant construction path.
  A regression in named-variant normalisation would now be caught immediately.
- **CI / Validation Infrastructure**: Claim 72 added to falsify.sh. The full narrow
  integer normalisation story (named struct → tuple struct → enum tuple variant →
  named enum variant) is now covered end-to-end in the falsification suite.

## Observed
- Cycle 126 implemented `enum_variant_narrow_field_types` for both
  `EnumVariantKind::Tuple` and `EnumVariantKind::Named`, but milestone 184 only
  added tests for **tuple** variants. Named variants had no coverage.
- The assembly inspection test `runtime_u8_enum_variant_field_construction_applies_trunc`
  (added in cycle 126) uses a tuple variant (`Wrap::Val(a + b)`), not a named field.
- The `enum_variant_field_narrow_ty` method and all 4 construction paths in lower.rs
  correctly handle named variants — but without tests, a future refactor could silently
  break them.

## Applied
- **`tests/e2e.rs`**: Added 9 milestone 185 tests:
  - 2 assembly inspection tests: `runtime_u8_named_enum_variant_field_construction_applies_trunc`
    (checks `and w` emitted) and `runtime_i16_named_enum_variant_field_construction_applies_sxth`
    (checks `sxth` emitted, no constant folding)
  - 7 compile-and-run tests: u8/u16/i16 wrap-on-construction, u8 identity, u8 passed-to-fn,
    u16 field in arithmetic, u16 two narrow fields
- **`.lathe/falsify.sh`**: Added Claim 72 (5 tests).
- **`.lathe/claims.md`**: Added Claim 72 documentation.
- Also: rebased branch onto origin/main (squash-resolved divergence from auto-merged M182 PR)
  and applied M183+M184 work as a single clean commit.

## Validated
- `cargo build` — clean
- `cargo test` — 1599 e2e + 211 unit + 30 fixture + 1 smoke, all passed
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- FLS §15: Named-field and tuple-field enum variant constructors must behave identically
  with respect to narrow integer normalisation. The spec says field values are stored
  "as the declared field type" — this applies regardless of whether fields are named or positional.
- No new ambiguities discovered.

## Next
- The narrow integer normalisation story is now complete (Claims 70, 71, 72 cover all
  three construction forms). Next priority: which fls_fixture parse-only tests can now
  be compiled end-to-end? Or: what's the next untouched FLS section? Candidate:
  `§6.8 Array Expressions` (repeat expressions with non-const length) or
  `§6.12.2 Method Call` on slice references (`s.len()` where s: &[T]).
