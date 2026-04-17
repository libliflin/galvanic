# Changelog — Customer Champion Cycle 005

## Stakeholder: The Compiler Contributor

Walked steps 1–7 of the Compiler Contributor journey. Picked `fls_12_1_generic_trait_impl.rs`
(parse-only, no `fn main`). Ran `galvanic` on it: output was `parsed 4 item(s)`, exit 0,
no output file, no explanation. Could not tell whether `use_it` was successfully lowered
or silently skipped.

**Goal:** When galvanic lowers a file successfully but has no `fn main`, print:
`galvanic: lowered N function(s) — no fn main, no assembly emitted`.
One new `println!` in `src/main.rs` at the early-return branch (line ~108).
Eliminates the class of "did it work?" confusion for every library-only fixture.

---

# Verification — Cycle 4, Round 1

## What was checked

**Goal match:** The builder added `**Minimal reproducer:**` blocks to all 46
entries in `refs/fls-ambiguities.md`. Goal asked for exactly this. 25 entries
received working reproducers; 21 marked "not yet demonstrable". Documentation-
only change — no source code touched.

**Tests:** `cargo test` — 2050 pass, 0 fail. Confirmed.

**Spot-checked reproducers by compiling them:**
- §6.5.3 NaN: `fn main() -> i32 { let x: f64 = 0.0_f64/0.0_f64; if x != x { 1 } else { 0 } }` → emits `fdiv d2, d0, d1 / fcmp d3, d4 / cset x5, ne`. Signature matches.
- §6.23 div guard: `fn div(x: i32, y: i32) -> i32 { x / y }` → emits `cbz x1, _galvanic_panic / sdiv`. Signature matches.
- §6.1.2 const eval: `const C: i32 = 1 + 2; fn main() -> i32 { C }` → emits `mov x0, #3`. Signature matches.
- §7.2 static alignment: `static X: i32 = 42; fn main() -> i32 { X }` → emits `.align 3` before `X:`. Matches.
- §6.5.9 narrowing: `fn narrow(x: i32) -> i32 { (x as u8) as i32 }` → emits `and w0, w0, #255`. Matches.
- §2.4.4.1 large int: `fn main() -> i32 { 65536 }` → emits `movz x0, #0x0001, lsl #16`. Matches (same value as `#1, lsl #16`).
- §6.5.7 shift: `fn shl(x: i64, n: i64) -> i64 { x << n }` → emits `cmp x1, #64 / b.hs _galvanic_panic / lsl x2, x0, x1`. **Mismatch found** (see below).

## Findings

**§6.5.7 reproducer was inaccurate.** The entry's `**Galvanic's choice:**` says
"No explicit masking instruction is emitted; the ARM64 hardware behavior
(implicit mod 64) satisfies the spec requirement." The builder's reproducer
faithfully echoed this: "confirms the shift amount is not explicitly masked and
the ARM64 hardware's implicit mod-64 is relied upon."

But the actual codegen emits:
```
cmp  x1, #64              // range check
b.hs _galvanic_panic       // panic if shift >= 64
lsl  x2, x0, x1           // shift
```

Galvanic does NOT rely on hardware mod-64 — it panics for shifts >= 64. The
`**Galvanic's choice:**` description is stale. A Spec Researcher following the
reproducer would see unexpected `cmp`/`b.hs` instructions and an incorrect
behavioral claim.

**§6.5.7 source citation is also stale.** `src/codegen.rs:594` points to the
division `cbz` guard, not the shift. Shift is at lines ~722–736. (Pre-existing
issue, not introduced this round.)

No other reproducers were observed to be inaccurate.

## Fixes applied

Fixed `refs/fls-ambiguities.md` §6.5.7 reproducer: corrected the assembly
signature to show `cmp x1, #64 / b.hs _galvanic_panic / lsl x2, x0, x1`, and
added a note that the `**Galvanic's choice:**` description above it is stale
(galvanic now panics rather than relying on hardware wrap).

**Files:** `refs/fls-ambiguities.md`

## Witnessed

- `cargo test`: 2050 pass, 0 fail (before and after fix).
- Compiled 7 of the reproducers directly via `cargo run -- /tmp/*.rs` and
  inspected the emitted `.s` files. Assembly signatures match for all checked
  entries except §6.5.7, which was corrected.
- The 21 "not yet demonstrable" entries are for parse-only fixtures
  (`fls_4_14_where_clauses_on_types.rs`, `fls_12_1_generic_trait_impl.rs`,
  etc.) — confirmed these are still parse-only by `cargo test --test fls_fixtures`.

VERDICT: PASS

---

# Changelog — Cycle 5, Round 1

## Goal
Print `galvanic: lowered N function(s) — no fn main, no assembly emitted` when
lowering succeeds but no `fn main` is present, so contributors can confirm
a library-only fixture was processed rather than silently ignored.

## Who This Helps
- **Stakeholder:** Compiler Contributor
- **Impact:** Eliminates "did it work?" ambiguity for every parse-only / library
  fixture. The contributor now sees a positive confirmation that lowering ran
  successfully, with a count of functions lowered.

## Applied
- `src/main.rs`: Added `println!` at the early-return branch (line 108–113).
  The message uses `module.fns.len()` for the count.
- `tests/smoke.rs`: Added `no_main_prints_lowered_note` test — creates a
  tempfile with `fn helper(x: i32) -> i32 { x + 1 }` (no main), runs
  galvanic, asserts exit 0 and the expected stdout note.

**Pipeline steps:** No new AST nodes, tokens, IR types, or codegen paths —
no cache-line notes or FLS citations needed. Change is purely in the CLI
driver (`main.rs`).

## Validated
- `cargo test`: 2055 pass, 0 fail (2050 existing + 5 smoke, including the new test).
- Smoke test `no_main_prints_lowered_note` passes — confirms exact stdout
  fragments `lowered`, `function(s)`, `no fn main`.
- Verifier can confirm: run
  `echo 'fn helper(x: i32) -> i32 { x + 1 }' > /tmp/nomain.rs && cargo run -- /tmp/nomain.rs`
  and check stdout contains `lowered 1 function(s) — no fn main, no assembly emitted`.

---

# Verification — Cycle 5, Round 1

## What was checked

**Goal match:** Builder added one `println!` in `src/main.rs` at the early-return
branch (lines 109–112) and one smoke test `no_main_prints_lowered_note` in
`tests/smoke.rs`. Exactly what the goal prescribed.

**Tests:** `cargo test` — all pass. Smoke suite: 5 passed (including new test).
e2e suite: 212 passed. fls_fixtures: 42 passed.

**End-to-end witness:**
```
$ echo 'fn helper(x: i32) -> i32 { x + 1 }' > /tmp/nomain.rs && cargo run -- /tmp/nomain.rs
galvanic: compiling nomain.rs
parsed 1 item(s)
galvanic: lowered 1 function(s) — no fn main, no assembly emitted
```
Matches the goal's "After" output exactly (with `function(s)` plural form).

**Original fixture from the goal:**
```
$ cargo run -- tests/fixtures/fls_12_1_generic_trait_impl.rs
galvanic: compiling fls_12_1_generic_trait_impl.rs
parsed 4 item(s)
galvanic: lowered 2 function(s) — no fn main, no assembly emitted
```
The Compiler Contributor now sees confirmation that lowering ran on both functions
in the fixture.

**Edge case — empty file (0 functions):**
```
$ cargo run -- /tmp/empty.rs
galvanic: compiling empty.rs
parsed 0 item(s)
galvanic: lowered 0 function(s) — no fn main, no assembly emitted
```
Exit 0. Existing `empty_file_exits_zero` smoke test still passes — it only asserts
`stdout.contains("galvanic: compiling")`, not exclusivity of output.

**Regression check — file with fn main is unaffected:** The new `println!` is
inside the `if !module.fns.iter().any(|f| f.name == "main")` branch. Normal
compilation paths are untouched.

## Findings

No issues. The change is minimal and correct:
- One `println!` at the right branch.
- Smoke test exercises the exact path.
- No pipeline stages touched — no FLS citations, cache-line notes, or IR changes needed.
- No constant-folding risk (CLI driver only).
- No `unsafe` introduced.
- Existing `empty_file_exits_zero` test unaffected.

## Fixes applied

None. The builder's work is solid.

VERDICT: PASS
