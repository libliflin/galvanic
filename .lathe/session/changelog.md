# Changelog — Cycle 115

## Who This Helps
- **CI / Validation Infrastructure**: CI was failing on `milestone_177_i8_add_wraps`
  and `milestone_177_i8_sub_wraps`. This fix unblocks all merges.
- **William (researcher)**: i8 wrapping semantics now actually correct on real
  ARM64 hardware (not just in the assembly text). The compile-and-run tests were
  catching a real codegen bug.

## Observed
- CI run #24143573639 reported two failures:
  ```
  milestone_177_i8_add_wraps: left: 0, right: 1
  milestone_177_i8_sub_wraps: left: 0, right: 1
  ```
- The assembly inspection tests (`runtime_i8_add_emits_sxtb_sign_extension`) passed
  locally and on CI — the `sxtb` instruction was present. But the runtime execution
  gave the wrong result.
- Root cause: `sxtb w{dst}, w{src}` sign-extends from byte to **32 bits**. Writing
  to a w-register on AArch64 zeroes the upper 32 bits of the corresponding x-register.
  So after `sxtb w2, w2` with input 150 (0x96), x2 = `0x00000000FFFFFF96`.
- When main compared this with `-106` via `neg x4, x3` (64-bit operation), x4 =
  `0xFFFFFFFFFFFFFF96`. The 64-bit `cmp x2, x4` found `0x00000000FFFFFF96 ≠
  0xFFFFFFFFFFFFFF96` → condition false → returned 0 instead of 1.
- The assembly inspection test only checked that `sxtb` was present — not that the
  sign extension reached 64 bits. A valid way for a CI failure to slip through.

## Applied
- **`src/codegen.rs`**: Changed `Instr::SextI8` codegen from `sxtb w{dst}, w{src}`
  to `sxtb x{dst}, w{src}`. The x-register destination causes sign-extension to fill
  all 64 bits with the sign bit, giving `0xFFFFFFFFFFFFFF96` for 150 (0x96 → -106).
- Updated the comment to explain why the 64-bit destination is required.

## Validated
- `cargo build` — clean
- `cargo test` — 1766 passed; 0 failed
- `cargo clippy -- -D warnings` — clean
- `bash .lathe/falsify.sh` — 64 passed, 0 failed
- Assembly for `fn add_i8(a: i8, b: i8) -> i8 { a + b }` now emits
  `sxtb x2, w2` — sign-extends 150 to `0xFFFFFFFFFFFFFF96` (-106 as 64-bit).

## FLS Notes
- **FLS §4.1**: Confirmed no ambiguity in value semantics. The AArch64 ABI issue
  (32-bit vs 64-bit sign extension) is implementation-defined, not spec-defined.
- No new FLS ambiguities discovered.

## Next
- u16/i16 narrow integer types — both map to I32/U32 without wrapping. The same
  class of bug (wrong truncation/extension) could exist there.
- u8/i8 compound assignment (`+=`, `*=`) — TruncU8/SextI8 is not emitted for
  compound-assignment paths, which go through the stack directly.
