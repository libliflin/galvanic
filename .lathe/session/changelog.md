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

---

# Verification — Cycle 8, Round 1

## What was checked

- Read the full diff: `ExprKind::variant_name()` exhaustive match in `src/ast.rs`, catch-all updated in `src/lower.rs`.
- Confirmed the match is exhaustive — the code compiles; a non-exhaustive match would be a compile error.
- Ran `cargo test`: 2055 pass, 0 fail, 0 ignored.
- Ran `./target/debug/galvanic tests/fixtures/fls_4_14_where_clauses_on_types.rs` and observed the exact error the goal named: `error: lower failed in 'main': not yet supported: StructLit expression in non-const context (runtime codegen not yet implemented)`.
- Checked that the variant count in `variant_name()` (38 arms) matches what compiles cleanly — confirmed by green build.

## Findings

- **Missing regression test.** The builder added no test that asserts the variant name appears in the error. The existing `lower_error_names_failing_item` test checks for `lower failed in '` (item name) but not for the new format. If `variant_name()` were accidentally removed, the test suite would still pass. Added a focused test.
- No cache-line notes needed — `variant_name()` is a method on an existing type, not a new type.
- No FLS citation needed — this is a pure diagnostic helper with no spec-mandated behavior.
- No constant-folding risk — change is in AST diagnostics only.

## Fixes applied

Added `lower_error_names_expr_kind_variant` to `tests/smoke.rs`: writes a temp file containing a `StructLit` argument inside an `EnumVariantLit` constructor (the exact case from the goal), runs galvanic, asserts `StructLit expression in non-const context` appears in stderr. Test passes.

**Files:** `tests/smoke.rs`

## Witnessed

```
$ ./target/debug/galvanic tests/fixtures/fls_4_14_where_clauses_on_types.rs
error: lower failed in 'main': not yet supported: StructLit expression in non-const context (runtime codegen not yet implemented)
lowered 2 of 3 functions (1 failed)
galvanic: lowered 2 function(s) — no fn main, no assembly emitted
exit: 1
```

`cargo test --test smoke`: 7 passed, 0 failed (including the new `lower_error_names_expr_kind_variant` test).

## Confidence

High. The goal asked for one thing — the variant name in the catch-all error — and it's delivered. The match is exhaustive (compiler-enforced), the message format is exactly what the goal specified, and the new smoke test locks in the regression guard.

VERDICT: PASS
