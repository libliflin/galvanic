# Changelog — Cycle 018, Round 1 (Builder)

## Goal
Fix wrong FLS §6.17 citations on loop control flow in emitted assembly. The
`Label`, `Branch`, and `CondBranch` IR instructions were introduced for
if-expression control flow and permanently annotated §6.17, even when used
for while loops (§6.15.3), infinite loops (§6.15.2), break (§6.15.6),
continue (§6.15.7), for loops (§6.15.1), while-let (§6.15.4), and match
expressions (§6.18).

## Who This Helps
- **Stakeholder:** Lead Researcher — traces §6.15 implementation through the
  emitted assembly
- **Impact:** The loop expressions fixture previously showed `FLS §6.17` on
  every `cbz`, `b`, and label — a fixture explicitly exercising §6.15. After
  this change, the assembly comments cite the actual originating FLS section
  at each site. The research artifact is now accurate.

## Applied
Added `fls: &'static str` provenance field to the `Label`, `Branch`, and
`CondBranch` IR instruction variants. Each emission site in `lower.rs` passes
the correct FLS section string. `codegen.rs` uses the field in the assembly
comment instead of the hardcoded `§6.17`.

**Structural property:** Since `Instr` is already 80 bytes (driven by `Call`
with `String` + `Vec<u8>`), adding `&'static str` (16 bytes) to the small
control-flow variants does not change the enum size. The `instr_size_is_documented`
test stays green at 80.

**FLS section mapping applied:**
- `§6.15.1` — for loop control flow (condition, back-edge, exit labels)
- `§6.15.2` — infinite loop header, back-edge, exit labels
- `§6.15.3` — while loop header, condition branch, back-edge, exit labels
- `§6.15.4` — while-let loop header, pattern checks, back-edge, exit labels
- `§6.15.6` — break branches
- `§6.15.7` — continue branches
- `§6.17` — if/if-let condition checks, else/end labels (unchanged role)
- `§6.18` — match expression arm pattern checks
- `§8.1` — let-else pattern checks

**Files modified:**
- `src/ir.rs` — changed `Label(u32)` → `Label { id: u32, fls: &'static str }`,
  `Branch(u32)` → `Branch { target: u32, fls: &'static str }`, added `fls` to
  `CondBranch`; updated docstrings and milestone comment
- `src/codegen.rs` — updated the 3 match arms to use struct destructuring and
  emit `// FLS {fls}: branch target/branch/branch if false`
- `src/lower.rs` — updated all 202 emission sites with correct FLS section strings

## Validated
- `cargo test`: 2084 pass, 0 fail
- `cargo clippy -- -D warnings`: clean
- `cargo test --lib -- instr_size_is_documented`: ok (still 80 bytes)
- `cargo run -- tests/fixtures/fls_6_15_loop_expressions.rs` emits the assembly;
  the loop expressions fixture now shows:
  - `.L0: // FLS §6.15.3: branch target` (while header)
  - `cbz x4, .L1 // FLS §6.15.3: branch if false` (while condition)
  - `b .L0 // FLS §6.15.3: branch` (while back-edge)
  - `b .L3 // FLS §6.15.6: branch` (break)
  - `b .L29 // FLS §6.15.7: branch` (continue)
  - `b .L2 // FLS §6.15.2: branch` (infinite loop back-edge)
  - `b .L10 // FLS §6.15.1: branch` (for loop back-edge)

The verifier should run `cargo run -- tests/fixtures/fls_6_15_loop_expressions.rs`
and inspect `tests/fixtures/fls_6_15_loop_expressions.s` — grep for `§6.15`
to confirm all loop branches now carry the correct citation.
