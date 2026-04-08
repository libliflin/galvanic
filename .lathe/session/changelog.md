# Changelog — Cycle 122

## Who This Helps
- **William (researcher)**: CI is broken on PR #211. The failing test
  `milestone_182_i16_struct_field_wraps_on_construction` triggered a pre-existing
  parser ambiguity (`parse_ty` greedily consuming `< 0 { … }` as a generic
  argument list when `as i32 < N` appears in an if-condition). Fixed by
  restructuring the test source to separate the cast from the comparison.
- **CI / validation infrastructure**: The e2e job now passes, unblocking the merge.

## Observed
- CI e2e job: `milestone_182_i16_struct_field_wraps_on_construction` FAILED.
- CI log: `error: parse error at byte 174: expected OpenBrace, found Eof`
- Root cause: `parse_ty()` (lines 1706–1716 of `src/parser.rs`) greedily treats
  `<` immediately after a type name as the start of a generic argument list.
  In the test source `if n.val as i32 < 0 { 1 } else { 0 }`, after parsing `i32`
  the parser sees `<` and enters a depth-counting loop that consumes
  `0 { 1 } else { 0 } }` without finding a matching `>`, reaching EOF, then
  returning the `i32` type successfully. The if-expression's condition becomes
  just `n.val as i32` (without the `< 0`), and `parse_block()` immediately finds
  EOF where it expected `{`.
- Assembly inspection tests confirmed `sxth` IS emitted for i16 struct fields.
  The underlying implementation is correct; only the test syntax was broken.

## Applied
- **`tests/e2e.rs`**:
  - Fixed `milestone_182_i16_struct_field_wraps_on_construction`: replaced
    `if n.val as i32 < 0 { 1 } else { 0 }` with `let v: i32 = n.val as i32;`
    followed by `if v < 0 { 1 } else { 0 }`. The `as i32 ;` ends with `;`
    instead of `<`, bypassing the `parse_ty` ambiguity entirely.
  - Added `runtime_i16_struct_field_construction_applies_sxth`: assembly inspection
    test verifying `sxth` is emitted (positive) and `add` is present (not folded)
    when constructing a struct with an i16 field from parameters.
    This is the missing inspection test for the i16 case (u16 already had one).

## Validated
- `cargo build` — clean
- `cargo test` — 1572 e2e + 211 unit + 30 fixture + 1 smoke = all passed (0 failed)
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- **FLS §12.1 / parse_ty ambiguity**: `parse_ty()` uses a greedy `<` heuristic
  for generic type arguments. This is documented as AMBIGUOUS in the comment
  (line ~1702: "FLS §12.1 AMBIGUOUS — the FLS does not specify the disambiguation
  rule for `<` in type position"). The ambiguity manifests when `as T < N` appears
  in a comparison: `T < N` is parsed as `T<N>` (incomplete generic). This is a
  known limitation; test sources must avoid `as T < expr` patterns.
  The correct long-term fix is a proper disambiguation rule (e.g. require `>` to
  close generics, or use parser lookahead), but that is a separate change.

## Next
- Check tuple struct fields with narrow types for the same wrapping gap.
- Check enum tuple variant fields with narrow types.
- Or: advance to the next untouched FLS section.
