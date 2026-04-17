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
