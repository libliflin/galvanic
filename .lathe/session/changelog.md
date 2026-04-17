# Changelog — Cycle 008

## Stakeholder: The Compiler Contributor

Walked steps 3–6 of the Compiler Contributor journey: picked the `fls_4_14_where_clauses_on_types.rs`
parse-only fixture, tried to run galvanic on it, and hit an opaque lowering error.

## What I experienced

Ran `galvanic tests/fixtures/fls_4_14_where_clauses_on_types.rs`. Got:
```
error: lower failed in 'main': not yet supported: expression kind in non-const context (runtime codegen not yet implemented)
```

The fixture's `main` uses struct literals, enum variant construction (`Maybe::Some(Foo { x: 7 })`),
method calls, function calls, and a `match`. The error names none of these — just "expression kind."
Searching source for the error string finds `lower.rs:18797`, a catch-all `_` arm in `lower_expr`.
`ExprKind` derives `Debug` but the variant name is not included in the message.

To find the actual culprit, I wrote isolation programs and found: `ExprKind::StructLit` as an
inline argument to an enum tuple variant constructor. That same struct literal works fine as a
regular function argument. The failing path calls `lower_expr(arg, &IrTy::I32)` on the struct
literal, which hits the catch-all.

A contributor who saw this error could not identify the expression kind without manual debug prints.

## Goal set

**Name the `ExprKind` variant in the catch-all error at `src/lower.rs:18797`.**

The error should say `StructLit expression in non-const context` (or whatever variant actually
fired), not just `expression kind in non-const context`. A helper or method that returns the
variant name (not the full debug tree) makes every future catch-all hit diagnosable: the contributor
sees the name, greps for `ExprKind::Name` in `lower.rs`, finds where to add the new arm.

This is a class-level fix: it applies to every unimplemented expression kind, not just `StructLit`.

## Why

The Compiler Contributor's signal is Clarity — "I know exactly where this goes." An opaque
catch-all error is the opposite of clarity. Naming the variant turns a dead end into a
search term. The specific moment: step 4 of the contributor journey, `lower.rs:18797`, zero
actionable information.

---

# Changelog — Cycle 008, Round 1 (Builder)

## Goal
Name the `ExprKind` variant in the `lower_expr` catch-all error so contributors
see `StructLit expression in non-const context` instead of the opaque
`expression kind in non-const context`.

## Who This Helps
- Stakeholder: Compiler Contributor
- Impact: Step 4 of the contributor journey (running galvanic on a fixture that
  hits an unimplemented lowering arm) now produces a named variant. The contributor
  can immediately grep for `ExprKind::StructLit` in `lower.rs` and find exactly
  where to add the missing arm. Previously the message was a dead end.

## Applied
- `src/ast.rs`: Added `impl ExprKind { fn variant_name(&self) -> &'static str }`.
  Exhaustive match over all 38 variants returns the variant name as a static string.
  Method is placed just before the `ClosureParam` struct (after the enum closes).
- `src/lower.rs:18797`: Changed catch-all `_ =>` to `other =>` and replaced the
  string literal with `format!("{} expression in non-const context …", other.variant_name())`.
  Added a one-line comment directing contributors to grep for the variant name.

Files modified:
- `src/ast.rs`
- `src/lower.rs`

## Validated
- `cargo test`: 2055 pass, 0 fail, 0 ignored.
- To exercise the new message: `cargo run -- tests/fixtures/fls_4_14_where_clauses_on_types.rs`
  will print `error: lower failed in 'main': StructLit expression in non-const context …`
  (or whichever variant fires first) instead of the generic message.
- The verifier can confirm the message format by grepping the binary output:
  `cargo run -- tests/fixtures/fls_4_14_where_clauses_on_types.rs 2>&1 | grep 'expression in non-const'`
