# Verification — Cycle 018, Round 1 (Verifier)

## What I compared

Goal: Fix wrong FLS §6.17 citations on loop control flow — `Label`, `Branch`, and `CondBranch` IR instructions should carry the FLS section of their originating lowering site (§6.15.x for loops, §6.17 for if/if-let, §6.18 for match).

Builder's approach: Added `fls: &'static str` field to `Label`, `Branch`, `CondBranch` in `ir.rs`; updated 202 emission sites in `lower.rs`; updated 3 match arms in `codegen.rs`.

What I ran:
- `cargo test` — 2084 pass
- `cargo run -- tests/fixtures/fls_6_15_loop_expressions.rs` — confirmed loop fixture now shows `§6.15.3`, `§6.15.2`, `§6.15.6`, `§6.15.7` on the correct instructions
- Grepped `§6.17` vs `§6.18` in `tests/fixtures/fls_6_18_match_expressions.s` — found **59 §6.17 citations, 16 §6.18** in the match fixture assembly
- Audited the `lower.rs` emission sites for the match expression lowering section (lines 12260–13780)

## What's here, what was asked

Gap found. The builder correctly tagged:
- All loop header/back-edge/exit labels with §6.15.x ✓
- The obvious match arm transitions (guard CondBranch, Body→Branch, next_label Label) with §6.18 ✓
- All if/if-let sites with §6.17 ✓

But missed 47 sites inside match expression lowering where **inner pattern check branches** — the `CondBranch` instructions for `RangeExclusive`, `Path` (enum variant), `TupleStruct`, `Struct`, and `@ binding` sub-patterns, plus their guard checks and default arm exits and the exit label — were left as `fls: "§6.17"`. These are in the match expression lowering paths for both scalar (i32) and unit-returning match blocks (lower.rs lines 12518–13780).

The assembly-level symptom: `fls_6_18_match_expressions.s` had 59 `§6.17` citations vs 16 `§6.18`. A Spec Researcher tracing §6.18 implementation through the assembly would miss the majority of match arm control flow.

## What I added

**`src/lower.rs`** — Fixed all 47 misclassified `fls: "§6.17"` sites inside the match expression lowering (lines 12518–13780) to `fls: "§6.18"`. Affected sites: RangeExclusive pattern checks, Path/enum-variant checks, TupleStruct field checks, Struct field checks, @ binding sub-pattern checks, guard CondBranch, arm exit Branch, next_label Label, and exit_label Label — in both the scalar-returning and unit-returning match lowering blocks.

After the fix: `fls_6_18_match_expressions.s` has 75 `§6.18` citations and **0 §6.17**.

**`tests/e2e.rs`** — Added 7 assembly inspection tests (cycle 018 section):

| Test | What it asserts |
|---|---|
| `while_loop_branches_cite_fls_6_15_3` | While loop branches cite §6.15.3; zero §6.17 in while-only function |
| `infinite_loop_branches_cite_fls_6_15_2` | Infinite loop back-edge/header cite §6.15.2 |
| `break_branch_cites_fls_6_15_6` | Break branch cites §6.15.6 |
| `continue_branch_cites_fls_6_15_7` | Continue branch cites §6.15.7 |
| `for_loop_branches_cite_fls_6_15_1` | For loop branches cite §6.15.1; zero §6.17 in for-only function |
| `if_expression_branches_cite_fls_6_17` | If expression branches cite §6.17; zero §6.15 in if-only function |
| `match_arm_branches_cite_fls_6_18` | Match arm branches cite §6.18; zero §6.17 in match-only function |

Total tests: 2091 (up from 2084). All pass. Clippy clean.

- **Files:** `src/lower.rs`, `tests/e2e.rs`
- **PR:** libliflin/galvanic#424

## Notes for the goal-setter

- The `§6.17` citations inside the let-else lowering (lines ~8093–8154) use `§6.17` for some CondBranch/Branch/Label — these involve if-let-style pattern matching within let-else context (FLS §8.1). The let-else else block is a §6.17-adjacent construct; those sites appear correctly classified.
- The `§6.5.2` logical `&&`/`||` short-circuit lowering also cites `§6.17` — that is the correct section per FLS §6.5.2 (the short-circuit emits if-expression-style branches). Worth verifying against the FLS separately, but not a regression from this cycle.
- The sweep covered all match expression lowering contexts (scalar, unit, tuple-returning, struct-returning, enum-returning). The tuple/struct/enum match paths were already correct at §6.18 from the builder's round; only the scalar and unit paths had the inner-pattern misclassification.
