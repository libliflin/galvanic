# Verification — Cycle 018, Round 3 (Verifier)

## What I compared

- Goal: fix FLS citations on `Label`/`Branch`/`CondBranch` so each instruction cites the spec section of the construct being lowered, not the borrowed if-expression technique.
- Builder's Round 2 change: 5 let-else control-flow instructions corrected from `§6.17` → `§8.1` (OR-pattern CondBranch, @-binding CondBranch, skip-else Branch, else-entry Label, end Label); 2 assembly inspection tests added.
- Ran: `cargo test --test e2e -- let_else_or_pattern_branches_cite_fls_8_1 let_else_bound_pattern_branches_cite_fls_8_1` — both pass. Full suite: 2095 pass, 0 fail.
- Clippy: clean.
- Witnessed: grepped all `§6.17` citations remaining in `lower.rs` — 54 entries. Classified every one by surrounding context (going up to 120 lines above each site for the enclosing `ExprKind::` arm).

## What's here, what was asked

Matches: the work holds up against the goal from my lens.

All 54 remaining `§6.17` citations in `lower.rs` are inside `ExprKind::If` or `ExprKind::IfLet` match arms — genuine if / if-let lowering. The full sweep is:

| Construct | Rounds fixed | Remaining §6.17 |
|---|---|---|
| While loops (§6.15.3) | Round 1 (Verifier) | 0 |
| Infinite loops (§6.15.2) | Round 1 (Verifier) | 0 |
| Break/continue (§6.15.6–7) | Round 1 (Verifier) | 0 |
| Match arms (§6.18) | Round 1 (Verifier) | 0 |
| `&&`/`\|\|` short-circuit (§6.5.8) | Round 2 (Builder) | 0 |
| let-else OR/@ patterns (§8.1) | Round 2 (Verifier) → Round 3 (Builder) | 0 |
| If / If-let expressions (§6.17) | — (correct) | 54 |

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

- The §6.17 citation sweep is complete. Every `CondBranch`/`Branch`/`Label` in `lower.rs` now cites the FLS section of the construct it implements. A researcher tracing §6.15.3, §6.18, §6.5.8, or §8.1 through emitted assembly will find the relevant instructions; §6.17 entries are exclusively genuine if / if-let lowering.
- None of this cycle's changes touch `codegen.rs` — the FLS citations in emitted assembly comments come from the `fls:` field on each IR instruction, which is sourced from `lower.rs`. No codegen change needed.

---

# Verification — Cycle 018, Round 2 (Verifier)

## What I compared

- Goal on one side: fix FLS citations on `Label`/`Branch`/`CondBranch` so each instruction cites the spec section of the construct being lowered.
- Builder's change: 8 emission sites in `BinOp::And` / `BinOp::Or` lowering corrected from `§6.17` → `§6.5.8`, with 2 assembly inspection tests added.
- Ran: `cargo test --test e2e lazy_and_branches_cite_fls_6_5_8` and `lazy_or_branches_cite_fls_6_5_8` — both pass. Full suite: 2093 pass, 0 fail.
- Witnessed: grepped all remaining `§6.17` citations on `CondBranch`/`Branch`/`Label` in `lower.rs` (60 total). Classified each by surrounding context.

## What's here, what was asked

Builder's change is correct and complete for `§6.5.8`. One category of misclassification remained unaddressed:

**`src/lower.rs`, let-else lowering (`StmtKind::Let` at line 7917)**: 5 instructions still cited `§6.17`:
- Line 8096: `CondBranch` in OR-pattern let-else
- Line 8125: `CondBranch` in `@`-binding (Bound) pattern let-else
- Lines 8145, 8150, 8154: shared `Branch` (skip-else), `Label` (else entry), `Label` (end)

The TupleStruct pattern at line 8031 was already correctly cited `§8.1`. The OR-pattern and Bound-pattern paths were missed. The comment at line 8148 correctly says "FLS §8.1: The else block must be a diverging expression" — the adjacent Branch/Label instructions cited the wrong section.

All 60 remaining `§6.17` citations are either genuine if-expression (`ExprKind::If`) or if-let lowering — correctly classified.

## What I added

Fixed `src/lower.rs` lines 8096, 8125, 8145, 8150, 8154: corrected `fls: "§6.17"` → `fls: "§8.1"` on the 5 misclassified let-else control-flow instructions.

Added 2 assembly inspection tests in `tests/e2e.rs`:
- `let_else_or_pattern_branches_cite_fls_8_1`: compiles a function with only an OR-pattern let-else, asserts `§8.1` present and `§6.17` absent.
- `let_else_bound_pattern_branches_cite_fls_8_1`: same for `@`-binding let-else.

Both tests pass. Full suite: 2095 pass (up from 2093), 0 fail.

- **Files:** `src/lower.rs`, `tests/e2e.rs`

## Notes for the goal-setter

- The `§6.17` sweep is now complete for the constructs this cycle touched: loops (§6.15.x), match arms (§6.18), lazy booleans (§6.5.8), and let-else (§8.1). All remaining `§6.17` citations in `lower.rs` are genuine if / if-let lowering.
- The `fls-ambiguities.md` has no entries for these misclassifications — they were annotation errors, not design choices, so no entry is needed.
- None.

---

# Changelog — Cycle 018, Round 2 (Builder)

## Goal
Fix wrong FLS citations on control-flow IR instructions: `Label`, `Branch`, and `CondBranch` should cite the FLS section of the construct being implemented, not the control-flow technique borrowed from if expressions.

## Who This Helps
- **Stakeholder:** Spec Researcher
- **Impact:** A researcher tracing §6.5.8 (Lazy Boolean Expressions) through emitted assembly can now find the `&&` and `||` short-circuit branches by section number. Previously every `&&`/`||` branch was annotated `§6.17` — invisible from a §6.5.8 search.

## Applied

The verifier's round 1 fixed loop constructs (§6.15.x) and match expression lowering (§6.18). They flagged two open items:
1. `&&`/`||` short-circuit branches citing `§6.17` — noted as "worth verifying separately"
2. let-else `§6.17` citations — assessed as correctly classified

Item 1 is the same class of misclassification. The `&&`/`||` lowering borrows the phi-slot pattern from if-expression codegen and its FLS comments correctly cite `§6.5.8` — but the IR emission sites still said `fls: "§6.17"`.

**`src/lower.rs`** — Corrected 8 emission sites in the `BinOp::And` and `BinOp::Or` lowering blocks (lines ~17045–17115):
- `&&`: CondBranch (skip-RHS), Branch (to-end), Label (false branch), Label (end)
- `||`: CondBranch (skip-RHS), Branch (to-end), Label (rhs branch), Label (end)

All changed from `fls: "§6.17"` → `fls: "§6.5.8"`.

**`tests/e2e.rs`** — Added 2 assembly inspection tests:
- `lazy_and_branches_cite_fls_6_5_8`
- `lazy_or_branches_cite_fls_6_5_8`

- **Files:** `src/lower.rs`, `tests/e2e.rs`
- **PR:** libliflin/galvanic#425

## Validated

- `cargo test` — 2093 pass, 0 fail (up from 2091)
- `cargo clippy -- -D warnings` — clean
- Verifier: run `cargo test --test e2e lazy_and_branches_cite_fls_6_5_8 lazy_or_branches_cite_fls_6_5_8`

---

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
