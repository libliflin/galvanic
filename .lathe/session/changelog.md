# Changelog — Cycle 009

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
