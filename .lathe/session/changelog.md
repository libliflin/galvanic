# Changelog — Cycle 114

## Who This Helps
- **CI / Validation Infrastructure**: The e2e job was failing due to timeout,
  blocking all merges. This restores CI health.
- **William (researcher)**: A broken CI gate means nothing can merge. Fixing it
  is the prerequisite for all forward progress.

## Observed
- Previous CI run (#24142834053) showed: `e2e: fail` with annotation
  "The job has exceeded the maximum execution time of 10m0s."
- The e2e job runs `cargo test --test e2e` on ubuntu-latest with the ARM64
  cross toolchain + QEMU. Each compile-and-run test spawns cross-assembler,
  cross-linker, and qemu-aarch64.
- The test suite has grown to 1524 e2e tests (up from ~1400 in recent cycles),
  and the cumulative subprocess overhead now exceeds 10 minutes on CI runners.
- Falsification: all 65 claims pass locally. Tests: 1766 passed, 0 failed locally.
  The timeout is a CI capacity issue, not a correctness issue.

## Applied
- **`.github/workflows/ci.yml`**: Increased `timeout-minutes` for the `e2e` job
  from 10 to 20.

## Validated
- `cargo build` — clean (no source changes)
- `cargo test` — 1766 passed, 0 failed
- `cargo clippy -- -D warnings` — clean

## FLS Notes
- None — no FLS sections touched this cycle.

## Next
- Once CI passes, the natural next step is **u16/i16 narrow integer types**.
  Both currently map to I32/U32 without wrapping, the same gap that u8/i8 had.
  A u16 function returning 40000_u16 + 30000_u16 would return 70000 instead
  of 4464 — wrong. The u8/i8 pattern (TruncU8/SextI8 at return boundaries)
  is the template.
- Alternatively: u8/i8 compound assignment (`+=`, `*=`) is unguarded —
  `let mut x: u8 = 200; x += 100;` would not wrap correctly.
