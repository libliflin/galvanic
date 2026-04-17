# Verification — Cycle 1, Round 1

## What was checked

**Goal match:** The builder's commit (418539d) only changed `.lathe/session/` files —
changelog and goal-history. No source code was modified. The feature described in the
goal (partial assembly emission) was not implemented. Verified by reading `src/lower.rs`
(`LowerErrors` had no `partial_module` field) and `src/main.rs` (error branch returned 1
with no assembly emitted).

**CI failure cause:** The clippy `result_large_err` failure shown in the session context
was preemptive — it would occur as soon as `Module` (≥120 bytes, five `Vec` fields) was
added to `LowerErrors`. Confirmed by reading the struct layout.

**Tests before fix:** `cargo test` — 2051 passed, 0 failed. Clippy clean.

## Findings

1. **Goal not implemented.** The builder wrote the goal document but emitted zero code.
   `LowerErrors` had no `partial_module` field; `main.rs` still returned 1 with no artifact.

2. **Preemptive clippy blocker.** Adding `Module` to `LowerErrors` directly would hit
   `clippy::result_large_err` (≥160-byte Err variant). Must box it.

3. **Missing smoke test.** No test verified that a `.s` file is produced on partial success.

## Fixes applied

**`src/lower.rs`:**
- Added `partial_module: Option<Box<crate::ir::Module>>` field to `LowerErrors`.
  Boxed to keep the `Err` variant small and satisfy `clippy::result_large_err`.
- Implemented `Debug` for `LowerErrors` manually (prints function count rather than full
  module, since `Module`'s nested IR types don't implement `Debug`).
- In `lower()`, when partial failures occur, builds the `Module` from the successfully-
  lowered `fns` and carries it in `LowerErrors.partial_module`. Returns `None` only when
  zero functions succeeded.

**`src/main.rs`:**
- Changed the `Err(errs)` branch to extract the partial module when present and continue
  compilation for it (emitting assembly), instead of returning 1 immediately.
- Returns exit 1 regardless after partial emission (goal: non-zero exit when any function
  failed, but artifact still produced).
- When partial output is written, stdout says `(partial — some functions failed)`.
- When partial module has no `fn main`, still prints "no fn main" message but returns 1
  (not 0) because lower errors occurred.

**`tests/smoke.rs`:**
- Added `partial_lower_emits_assembly_for_successful_fns`: writes a tempfile with
  `fn main()` (succeeds) + `fn unsupported()` using tuple scrutinee (fails), runs
  galvanic, asserts exit 1, `.s` file exists, assembly contains "main", stdout
  contains "partial".

**Files:** `src/lower.rs`, `src/main.rs`, `tests/smoke.rs`

## Witnessed

```
$ ./target/debug/galvanic tests/fixtures/fls_6_18_match_expressions.rs
galvanic: compiling fls_6_18_match_expressions.rs
parsed 16 item(s)
error: lower failed in 'match_tuple': not yet supported: tuple expression must be bound to a `let` variable at this milestone
lowered 12 of 13 functions (1 failed)
galvanic: emitted tests/fixtures/fls_6_18_match_expressions.s (partial — some functions failed)
exit: 1
```

Assembly confirmed: `cmp`, `cset`, `cbz`, `ldr`, `str` runtime instructions. No constant
folding. 12 functions including `fn main()` emitted. The `match_tuple` function is absent.

```
cargo test: 2058 pass, 0 fail (6 smoke including new partial-emission test)
cargo clippy -- -D warnings: clean
```

## Confidence

High. The feature is implemented end-to-end, the clippy blocker is resolved by boxing,
and the new smoke test directly exercises the partial-emission path. The goal's exact
scenario — run `fls_6_18_match_expressions.rs`, get a `.s` file for the 12 that worked —
is now witnessed.

VERDICT: PASS

---

# Changelog — Cycle 001

## Stakeholder: Lead Researcher

**Rotation rationale:** The last 4+ goals served Compiler Contributor (×3) and Spec Researcher
(×1). The Lead Researcher — the person this compiler is built for — hadn't been served in 5+
cycles. Today's cycle was theirs.

## Journey walked

- Confirmed floor: `cargo test` — 2051 passed, 0 failed, build clean.
- Picked `fls_6_18_match_expressions.rs` as the most substantive parse-only fixture (13 functions
  covering §6.18 comprehensively: literal patterns, guards, boolean scrutinee, or-patterns, enum
  variants, match-in-let, tuple scrutinee, range patterns, nested match).
- Ran `cargo run -- tests/fixtures/fls_6_18_match_expressions.rs`.
- Result: 12 of 13 functions lowered; `match_tuple` failed (tuple expression as scrutinee is
  not yet supported).
- No `.s` file was produced.
- Verified separately that simple match expressions emit correct runtime assembly (cmp/cset/cbz
  branches — not constant-folded, ABI-correct).

## What I found

12 of 13 §6.18 functions compile successfully. The emitted assembly for those 12 is correct
runtime code. The one failure (`match_tuple`) is caused by a separate, unimplemented feature
(tuple expression in value context), not a match expression bug.

But the researcher cannot see any of this: `LowerErrors` carries no partial output, so 
`main.rs` emits nothing when any function fails.

## Goal set

**Emit partial assembly when lowering partially succeeds.**

When `lower()` returns partial results (some functions succeed, some fail), carry the
successfully-lowered `fns` in `LowerErrors` (or an equivalent partial-success return type)
so `main.rs` can emit a `.s` file for the successful functions. Exit code stays non-zero.
Error messages still print. But the artifact is produced.

This eliminates the whole class of "partial success produces no output" — every parse-only
fixture that has one unsupported construct will immediately become inspectable for all the
constructs that do work.

## Next cycle candidates

- The tuple-scrutinee match (`match (x, y) { (0, 0) => ... }`) is a natural next feature to
  unblock `fls_6_18_match_expressions.rs` completely.
- `fls_9_functions.rs` and `fls_2_4_literals.rs` are also parse-only and likely in the same
  partial-lowering situation.
- Once partial output is emitted, the researcher can document FLS findings from match pattern
  assembly — particularly the `AMBIGUOUS` question of whether wildcard pattern lowering order
  is specified by §6.18.

---

# Goal — Cycle 2 (Customer Champion Cycle 006)

## Stakeholder: The Lead Researcher

**Rotation rationale:** Cycle 004 served the Spec Researcher. Cycle 005 served the Compiler
Contributor. The Lead Researcher has not been served for two cycles.

## Floor check

Build: OK. Tests: 2052 pass, 0 fail. Clippy: OK. Unsafe audit: OK.

## Journey walked

Picked `fls_2_4_literals.rs` — the most foundational parse-only fixture, containing verbatim
FLS §2.4 examples. It has a `fn main` so it should produce assembly when it compiles.

```
cargo run -- tests/fixtures/fls_2_4_literals.rs
```

Output:
```
galvanic: compiling fls_2_4_literals.rs
parsed 1 item(s)
error: lower failed in 'main': not yet supported: cannot parse float literal: `8_031.4_e-12f64`
lowered 0 of 1 functions (1 failed)
```

Root cause traced to `parse_float_value` in `src/lower.rs:4089–4099`:
- `strip_suffix("_f64")` on `8_031.4_e-12f64` returns None (ends with `f64`, not `_f64`)
- `strip_suffix("_f32")` also returns None
- Underscores stripped from the unsuffixed text → `8031.4e-12f64`
- Rust's float parser rejects `f64` as a suffix → error

The literal `8_031.4_e-12f64` is a verbatim example from FLS §2.4.4.2. Galvanic's own
fixture file embeds the spec's example, and galvanic can't compile it.

## Goal

**Fix float literal suffix parsing in `parse_float_value` and `parse_float32_value`
(both in `src/lower.rs`) to accept bare `f64`/`f32` suffixes without a leading underscore,
per FLS §2.4.4.2.**

**What to change:**

1. `parse_float_value` (~line 4091): After `strip_suffix("_f64")`, also try `strip_suffix("f64")`.
   After `strip_suffix("_f32")`, also try `strip_suffix("f32")`. Order: longest-match first
   (`_f64` before `f64`, `_f32` before `f32`) so the underscore separator is consumed when present.

2. `parse_float32_value` (~line 4107): Same fix for bare `f32`/`f64` suffixes.

3. The f32-dispatch check at ~line 10716: `text.ends_with("_f32")` should also check
   `text.ends_with("f32")` so bare-suffix `3.0f32` routes to the f32 codepath. (A float
   literal cannot legitimately end with the digits `f32` in any other way — `f` is not a
   decimal digit, so there's no ambiguity.)

4. Update `refs/fls-ambiguities.md` §2.4.4.2: change "Only decimal float literals with
   optional `_f32`/`_f64` suffix are supported" to reflect that both `_f64`/`f64` forms
   are now supported. The remaining gap (NaN, infinity, hex floats) is unchanged.

**Why this matters:** This is not a cosmetic fix — it's a FLS §2.4.4.2 compliance gap where
the spec's own example fails. Fixing it moves `fls_2_4_literals.rs` toward end-to-end
coverage and adds a real finding to the research record.

**The specific moment:** Step 6 of the Lead Researcher journey, running
`galvanic tests/fixtures/fls_2_4_literals.rs`. Error: "cannot parse float literal:
`8_031.4_e-12f64`". This literal appears verbatim in FLS §2.4.4.2 and in galvanic's own
fixture file.
