# Changelog — Cycle 117

## Who This Helps
- **William (researcher)**: A real correctness bug is fixed. Programs that use
  narrowing casts (`x as u8`, `x as i8`) now produce the correct truncated value
  rather than silently returning the untruncated source. `300_i32 as u8` now gives
  44 instead of 300. The fix is fenced by Claim 67 and 9 new tests.
- **FLS / Ferrocene ecosystem**: §6.5.9 is now correctly implemented — narrowing
  integer casts truncate to the low N bits. The previous "truncation deferred"
  comment was masking a real spec violation.

## Observed
- The previous cycle's work on u8/i8 arithmetic and compound-assignment wrapping
  prompted a review of all places where u8/i8 values are produced.
- The `Cast` handler in `lower.rs` had an explicit "truncation deferred" comment
  for `as u8` (line ~15982), treating it as identity — the inner expression was
  lowered as U32 with no masking instruction emitted.
- Adversarial test: `fn f(x: i32) -> i32 { (x as u8) as i32 }; f(300)` — without
  the fix, returns 300. With the fix, returns 44 (300 & 255).
- `fn f(x: i32) -> i32 { if (x as i8) < 0 { 1 } else { 0 } }; f(200)` — without
  fix, 200 is not < 0 (no sign extension), returns 0. With fix, `sxtb` sign-extends
  to -56, returns 1.

## Applied
- **`src/lower.rs`**:
  - Split `"u8"` out of the unsigned integer cast arm (`"u16" | "u32" | ...`) as
    a separate match arm. After lowering the inner expression, emits
    `Instr::TruncU8 { dst: r, src: r }` → `and w{dst}, w{dst}, #255`.
  - Split `"i8"` out of the signed integer cast arm (`"i16" | "i32" | ...`) as a
    separate match arm. For float sources (f64/f32 → i8), converts to integer first
    then emits `SextI8`. For integer sources, emits `Instr::SextI8 { dst: r, src: r }`
    → `sxtb x{dst}, w{dst}`.
  - Updated comments: removed "truncation deferred" language; replaced with
    accurate FLS §6.5.9 citation noting that `u8` and `i8` are handled above.
- **`tests/e2e.rs`**:
  - `runtime_cast_to_u8_emits_and_truncation`: assembly inspection.
  - `runtime_cast_to_i8_emits_sxtb`: assembly inspection.
  - `milestone_179_cast_u8_truncates_300_to_44`: 300 as u8 = 44.
  - `milestone_179_cast_u8_truncates_256_to_0`: 256 as u8 = 0.
  - `milestone_179_cast_i8_sign_extends_200_to_negative`: 200 as i8 < 0.
  - `milestone_179_cast_i8_127_stays_positive`: 127 as i8 >= 0.
  - `milestone_179_cast_u8_result_in_arithmetic`: result used in arithmetic.
  - `milestone_179_cast_u8_passed_to_fn`: result passed to function.
  - `milestone_179_cast_i8_in_arithmetic`: i8 cast in arithmetic.
- **`.lathe/claims.md`**: Claim 67 added.
- **`.lathe/falsify.sh`**: Claim 67 check added.

## Validated
- `cargo build` — clean
- `cargo test` — 1779 passed; 0 failed (1537 e2e + 211 unit + 30 fls_fixtures + 1 smoke)
- `cargo clippy -- -D warnings` — clean
- `bash .lathe/falsify.sh` — 66 passed, 0 failed

## FLS Notes
- **FLS §6.5.9**: The spec says narrowing integer casts truncate to the target
  type's bit width. The previous comment called this "AMBIGUOUS" about the
  mechanism, but the spec is clear: the result is the low N bits. The ambiguity
  was about implementation mechanism, not semantics. Removed the misleading comment.
- **FLS §6.5.9**: Signed narrowing (`as i8`) requires sign-extension from 8 bits,
  not just masking. ARM64 `sxtb` handles this correctly in one instruction.

## Next
- `as u16` / `as i16` casts are still identity. Adding u16/i16 narrowing follows
  the same pattern: `and #65535` for u16 (TruncU16), `sxth` for i16 (SextI16).
- Regular assignment `x = expr` where `x: u8` — now that `as u8` truncates, most
  paths that produce u8 values do so correctly. The remaining gap is narrow.
