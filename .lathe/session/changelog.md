# Verification — Cycle 019, Round 2 (Verifier)

## What I compared

- Goal: surface cache-line thesis in emitted assembly at all structural points named — function prologues, loop boundary labels, `.align` directives, `_start`, `_galvanic_panic`.
- Prior rounds: builder emitted commentary at all 5 points; verifier round 1 added 3 tests for `.data`, `.rodata`, and zero-prologue edge cases. 7 tests total, all passing.
- Ran: `cargo test --test e2e -- cache_line` — 7 pass. Full suite: 2102 pass, 0 fail. Clippy clean.
- Witnessed: compiled `tests/fixtures/fls_6_15_loop_expressions.rs` — 37 `cache-line:` lines in emitted assembly covering prologues, loop boundaries, `_start`, `_galvanic_panic`. ✓
- Counted `_start` instructions in emitted assembly: `bl main`, `mov x8, #93`, `svc #0` — **3 instructions, 12 bytes**. The `// x0 = main()'s return value` line is a comment, not an instruction.
- Read `codegen.rs` `emit_start()`: doc comment at line 2011 says "3 instructions (12 bytes)" — correct. Emitted commentary at line 2017 said "4 instructions × 4 bytes = 16 bytes" — **wrong**. Test asserted `"_start = 4 instructions"` — **wrong**.

## What's here, what was asked

Gap found: factual error in `_start` cache-line commentary. The emitted `.s` file claimed "4 instructions" but `_start` has exactly 3. A Cache-Line Researcher using the emitted commentary to trace footprint would compute 16 bytes instead of the correct 12 — a 33% overcount for the entry point. The doc comment on `emit_start()` correctly said "3 instructions" but the string written to the assembly said "4".

All other 6 commentary sites are factually correct.

## What I added

**`src/codegen.rs`** — Corrected `emit_start()`: changed "4 instructions × 4 bytes = 16 bytes" → "3 instructions × 4 bytes = 12 bytes" in both the inline source comment and the `writeln!` that emits to the assembly file.

**`tests/e2e.rs`** — Updated `start_emits_cache_line_note`: changed assertion from `"_start = 4 instructions"` to `"_start = 3 instructions"`; updated doc comment to enumerate the 3 actual instructions (`bl main`, `mov x8, #93`, `svc #0`) and correct the byte count (12 bytes, not 16).

All 7 cache-line tests pass. Full suite: 2102 pass, 0 fail. Clippy clean.

- **Files:** `src/codegen.rs`, `tests/e2e.rs`

## Notes for the goal-setter

- The error was introduced when the builder wrote the `_start` commentary: the function has `bl main` + a comment line + `mov x8, #93` + `svc #0`. Counting the comment line as an instruction gives 4; the correct count is 3. The doc comment on `emit_start()` had it right ("3 instructions"), but the emitted string did not.
- No structural issues. After this fix, every cache-line count in the emitted commentary matches the actual instruction stream.

---

# Verification — Cycle 019, Round 1 (Verifier)

## What I compared

- Goal: surface cache-line thesis in emitted assembly at key structural points (function prologues, loop boundary labels, `_start`, `_galvanic_panic`, `.data`/`.rodata` section headers).
- Builder's change: 5 emission sites added to `codegen.rs`; 4 assembly inspection tests added in `tests/e2e.rs`.
- Ran: `cargo test --test e2e -- fn_prologue_emits_cache_line_note loop_label_emits_cache_line_note galvanic_panic_emits_cache_line_note start_emits_cache_line_note` — all 4 pass.
- Witnessed: `cargo run -- tests/fixtures/fls_6_15_loop_expressions.rs && grep cache-line tests/fixtures/fls_6_15_loop_expressions.s` — 34 `cache-line:` lines visible at function prologues, loop boundary labels, `_start`, and `_galvanic_panic`. ✓
- Checked `.data` output with a static: commentary emitted. ✓
- Checked `.rodata` output with a f64 constant: commentary emitted. ✓
- Checked zero-prologue edge case (`fn id(x: i32) -> i32 { x }`, leaf, no stack frame): emits "0 instr(s) × 4 bytes = 0 bytes — 0 of 16 slots in first cache line". Correct — the entire first cache line is available for body instructions.
- Full suite: 2099 pass, 0 fail (builder's count). Clippy clean.

## What's here, what was asked

Gap found: builder added `.data` and `.rodata` cache-line commentary to `emit_asm()` but added no tests for those two emission sites. The 4 new tests cover prologues, loop labels, `_start`, and `_galvanic_panic`, but not data-section or rodata-section commentary. A researcher searching the assembly for `cache-line` after compiling a program with statics or float constants would see the comments — but CI wouldn't catch a regression that removed them.

Also uncovered: the zero-prologue case (leaf function, no stack frame) emits "cache-line: prologue = 0 instr(s)" — correct but untested.

## What I added

Added 3 assembly inspection tests to `tests/e2e.rs`:

| Test | What it asserts |
|---|---|
| `data_section_emits_cache_line_note` | `.data` section contains "cache-line: each static .quad" |
| `rodata_section_emits_cache_line_note` | `.rodata` section contains "cache-line: f64 constants" |
| `leaf_fn_zero_prologue_emits_cache_line_note` | Leaf function with no stack frame emits "cache-line: prologue = 0 instr(s)" |

All 3 pass. Full suite: 2102 pass (up from 2099), 0 fail. Clippy clean.

- **Files:** `tests/e2e.rs`

## Notes for the goal-setter

- The zero-prologue comment "0 of 16 slots in first cache line" is correct semantics but reads slightly mechanically. The Cache-Line Researcher reading it sees "the prologue consumed 0 slots" — all 16 are available for body code. This is actually the strongest possible cache-line result for a leaf function. If a future cycle wants to special-case this (e.g. emit "no prologue — first cache line fully available for body"), that's a presentation improvement, not a correctness issue.
- No structural issues found. The 5 emitted changes are each load-bearing for the researcher journey and are now all tested.

---

# Changelog — Cycle 019, Round 1 (Builder)

## Goal
Surface galvanic's cache-line thesis in the emitted assembly output. The Customer Champion
found ~20 cache-line notes in `codegen.rs` but zero in the emitted `.s` files. The reasoning
was documented in the compiler source; it was never emitted to where the Cache-Line
Performance Researcher actually looks.

## Who This Helps
- **Stakeholder:** Cache-Line Performance Researcher
- **Impact:** Opening any emitted `.s` file and searching for "cache-line" now returns
  results at every key structural boundary: function prologues, loop boundary labels,
  `_galvanic_panic`, and `_start`. The thesis is verifiable from the output alone, without
  reading the compiler source.

## Applied

**`src/codegen.rs`** — Emit cache-line commentary at four structural points:

1. **Function prologues** (`emit_fn`): Emit `// cache-line: prologue = N instr(s) × 4 bytes
   = M bytes — N of 16 slots in first cache line` immediately after each function label.
   Restructured `emit_fn` to compute `fsize` before the prologue section so the count is
   available when the comment is emitted.

2. **Loop boundary labels** (`Instr::Label`): When `fls` contains "§6.15", emit
   `// FLS §6.15.x: loop boundary — cache-line: label has zero footprint` instead of the
   generic "branch target" annotation.

3. **`_start`** (`emit_start`): Emit `// cache-line: _start = 4 instructions × 4 bytes = 16
   bytes — fits in one 64-byte cache line` before the `.global _start` directive.

4. **`_galvanic_panic`** (`emit_galvanic_panic`): Emit `// cache-line: _galvanic_panic = 3
   instructions × 4 bytes = 12 bytes — fits in one 64-byte cache line` before the `.global
   _galvanic_panic` directive.

5. **`.data` and `.rodata` section headers**: Emit a cache-line context comment after each
   section directive documenting per-element footprint and items-per-cache-line count.

**`tests/e2e.rs`** — Added 4 assembly inspection tests (cycle 019 section):

| Test | What it asserts |
|---|---|
| `fn_prologue_emits_cache_line_note` | Assembly contains "cache-line: prologue" |
| `loop_label_emits_cache_line_note` | Assembly contains "§6.15" and "loop boundary" |
| `galvanic_panic_emits_cache_line_note` | Assembly contains "_galvanic_panic = 3 instructions" |
| `start_emits_cache_line_note` | Assembly contains "_start = 4 instructions" |

- **Files:** `src/codegen.rs`, `tests/e2e.rs`

## Validated

- `cargo test` — 2099 pass, 0 fail (up from 2095; +4 new tests)
- `cargo clippy -- -D warnings` — clean
- Compiled `tests/fixtures/fls_6_15_loop_expressions.rs`; confirmed cache-line commentary
  visible at: function prologues, `.L0:`/`.L1:` loop boundary labels, `_galvanic_panic:`,
  and `_start:`.
- Verifier: run `cargo test --test e2e -- fn_prologue_emits_cache_line_note
  loop_label_emits_cache_line_note galvanic_panic_emits_cache_line_note
  start_emits_cache_line_note` to witness the four new tests. Then
  `cargo run -- tests/fixtures/fls_6_15_loop_expressions.rs && grep cache-line
  tests/fixtures/fls_6_15_loop_expressions.s` to see the commentary in context.

---

# Changelog — Cycle 019 (Customer Champion)

## Stakeholder: Cache-Line Performance Researcher

**Who I became.** A performance engineer studying galvanic's thesis that cache-line alignment is a first-class codegen concern, not a bolted-on optimization. They're evaluating whether the approach is verifiable — whether the claim is documented, tested, and visible in the output.

**What I did.**
1. Confirmed the floor: `cargo test` — 2095 tests, all pass. Build clean. Clippy clean.
2. Read the README. The cache-line claim is prominent: "obsessively cache-line-aware...not as an optimization pass bolted on at the end, but as a constraint woven into layout, register allocation, and instruction selection from the start." ✓
3. Ran `cargo bench`. Criterion reports throughput in MiB/s (~650–695 MiB/s for lexer, ~175–195 MiB/s for parser). ✓
4. Found the size assertion tests: `token_is_eight_bytes` passes, `instr_size_is_documented` passes (80 bytes), `ir_value_is_eight_bytes` passes. ✓
5. Compiled `tests/fixtures/fls_6_15_loop_expressions.rs`. Clean compile. Opened the `.s` file.
6. Searched the emitted assembly for "cache", "align", "line". **Zero results.** Every instruction has an FLS section citation. But the cache-line reasoning — the thing that makes this project distinct — is absent from the output entirely.
7. Opened `codegen.rs`. Found ~20 cache-line notes in the source (e.g., line 345: "Cache-line note: `sub sp, sp, #N` is one 4-byte instruction — the frame setup occupies one slot in the first cache line of the function body"). None of them emitted to the output.
8. Checked `.align` directives in the data section. They exist. No comment explaining they're for cache-line slot packing.

**The worst moment.** Reading "Cache-line note: `sub sp, sp, #N` is one 4-byte instruction — frame setup occupies one slot in the first cache line" in codegen.rs line 345, then opening the emitted assembly and seeing only `sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)`. The reasoning was considered. It was documented. It was never emitted.

**The goal set.** Emit cache-line commentary at key structural points in the assembly output (function prologues, loop headers, `.align` directives, `_galvanic_panic`). The existing codegen.rs cache-line notes are the source; a subset should surface in the emitted output so the `.s` file verifies the cache-line thesis without reading the compiler source.

**Why now.** Cache-Line researcher last served at cycle 015 (four cycles ago — most under-served). The gap between claim and verifiable evidence widens with each cycle that adds FLS citations without adding cache commentary.

---

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
