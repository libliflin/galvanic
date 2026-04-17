# Changelog — Cycle 009, Round 1

## Goal
Support struct literal arguments in enum tuple variant constructor calls in
`lower_stmt`. When an arg is `ExprKind::StructLit`, call `store_nested_struct_lit`
instead of routing through `lower_expr` (which hits the catch-all error). Add
an `AMBIGUOUS: §4.2` annotation and a matching `refs/fls-ambiguities.md` entry
documenting galvanic's variant field layout choice.

## Who This Helps
- **Stakeholder:** Lead Researcher
- **Impact:** `fls_4_14_where_clauses_on_types.rs` moves from hard failure
  ("not yet supported: StructLit expression in non-const context") to partial
  compilation — 5 of 6 functions now lower successfully. The remaining error
  (`v.get()` on match-bound generic `T`) is a new, distinct problem that can
  be tracked as the next goal. Momentum is restored.

## Applied
- **`src/lower.rs`** — In the enum tuple variant constructor loop (the `_`
  arm of the float-type dispatch), added a check: if `arg` is `ExprKind::StructLit`
  and the struct name is known, call `store_nested_struct_lit(arg, slot, struct_name)`
  and register `slot_generic_type` for downstream method dispatch. Falls through
  to the original `lower_expr` path for non-struct args.
  Added `AMBIGUOUS: §4.2` annotation documenting the inline field layout choice.
- **`refs/fls-ambiguities.md`** — Added `§4.2 — Struct-Typed Enum Variant Field
  Layout` entry (ToC + body) with gap description, galvanic's resolution, source
  pointer, and minimal reproducer. Updated §4.14 entry: fixture is now partially
  compiling rather than parse-only.
- **`tests/smoke.rs`** — Updated `lower_error_names_expr_kind_variant` test:
  the original test checked for the "StructLit expression" error message (which
  no longer fires). Updated to verify the construct now compiles successfully
  with exit code 0 and no "not yet supported" in stderr. The fix is in the code;
  the test now validates the new correct behavior.

## Validated
- `cargo test`: 2056 passed, 0 failed (all three suites: smoke, fls_fixtures, e2e).
- `cargo run -- tests/fixtures/fls_4_14_where_clauses_on_types.rs`: now lowers
  5 of 6 functions; the StructLit error is gone. Remaining error is in
  `get_maybe__i32` (`v.get()` on match-bound `T`) — a separate, new problem.
- Verifier: run `cargo run -- tests/fixtures/fls_4_14_where_clauses_on_types.rs`
  and confirm the error is no longer "StructLit expression in non-const context"
  but instead about method calls on generic-typed match bindings.
- Verifier: `grep 'AMBIGUOUS.*4\.2' src/lower.rs` finds the new annotation.
- Verifier: `grep 'Struct-Typed Enum Variant' refs/fls-ambiguities.md` finds the ref entry.

---

## Stakeholder: The Lead Researcher

Walked steps 3–9 of the Lead Researcher journey. Floor intact (2056 tests pass).

Picked `fls_4_14_where_clauses_on_types.rs` — the parse-only fixture with a `fn main`.
Ran `cargo run -- tests/fixtures/fls_4_14_where_clauses_on_types.rs`.
Error: "not yet supported: StructLit expression in non-const context."

Traced the failure to `lower_stmt` line 9487: the enum variant constructor path
(`let m = Maybe::Some(Foo { x: 7 })`) calls `lower_expr(arg, &IrTy::I32)` for each
field argument. When the arg is a `StructLit`, `lower_expr` hits the catch-all. The
`store_nested_struct_lit` helper already exists and does exactly what's needed — it's
just not called from the enum variant constructor path.

**Goal set:** Support struct literal arguments in enum variant constructor calls in
`lower_stmt`. When an arg is `ExprKind::StructLit`, call `store_nested_struct_lit`
instead of `lower_expr`. Add an `AMBIGUOUS: §4.2` annotation and a matching
`refs/fls-ambiguities.md` entry documenting galvanic's variant field layout choice.

**Why now:** Moving `fls_4_14_where_clauses_on_types.rs` from parse-only to partially
compiling gives the Lead Researcher momentum and captures a genuine FLS ambiguity
(variant field layout is unspecified). The fix reuses an existing helper — no new
infrastructure needed, just a missing call site.
