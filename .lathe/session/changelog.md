# Verification — Cycle 8, Round 3

## What was checked
- Read builder diff: only `.lathe/goal.md` was changed (goal renamed from 4q to 4s description)
- Confirmed `src/codegen.rs` `IrBinOp::Add/Sub/Mul` arms still had single unguarded writeln! (no overflow guard)
- Confirmed no `claim_4s_*` tests in `tests/e2e.rs` (still 1784 tests)
- Confirmed `claim_4o_galvanic_panic_absent_without_division` tested `2 + 3 * 4 - 1` — would break once guards added
- Confirmed `refs/fls-ambiguities.md` §4.9 still said "No bounds check is emitted" (stale since Claim 4p)
- Confirmed §6.9/§6.23 entry listed `+/-/*` as "no overflow check; known deviation"
- Ran `cargo build`, `cargo test`, `cargo clippy` after full implementation

## Findings
**Builder made no implementation changes this round.** Only `.lathe/goal.md` was updated.

Specific gaps:
1. `IrBinOp::Add`, `IrBinOp::Sub`, `IrBinOp::Mul` in `codegen.rs` still emitted single unguarded instructions
2. No `claim_4s_*` tests in `tests/e2e.rs`
3. `claim_4o_galvanic_panic_absent_without_division` tested `2 + 3 * 4 - 1` — would fail after guard is added
4. `refs/fls-ambiguities.md` §4.9 still said "No bounds check is emitted at this milestone" (stale since Claim 4p)
5. §6.9/§6.23 entry listed `+/-/*` as "no overflow check; known deviation"

## Fixes applied
Implemented the complete Claim 4s:

1. **`src/codegen.rs`**: Replaced single-writeln arms for `IrBinOp::Add`, `Sub`, `Mul` with 4-instruction guarded sequences: primary op + `sxtw x9, w{dst}` + `cmp x{dst}, x9` + `b.ne _galvanic_panic`.
2. **`tests/e2e.rs`**: Fixed `claim_4o_galvanic_panic_absent_without_division` to use `fn main() -> i32 { 42 }`. Added 10 Claim 4s tests: 3 assembly inspection, 4 runtime panic, 3 runtime success (all using qemu-skip pattern).
3. **`refs/fls-ambiguities.md`**: Fixed §4.9 (removed stale "no bounds check" text, described Claim 4p guard). Updated §6.9/§6.23 `+/-/*` bullet with Claim 4s sxtw/cmp guard and AMBIGUOUS annotation.

- Files: `src/codegen.rs`, `tests/e2e.rs`, `refs/fls-ambiguities.md`
- Test count: 1784 → 1794 (all pass)
- PR: libliflin/galvanic#334

VERDICT: PASS
