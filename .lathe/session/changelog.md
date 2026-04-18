# Verification — Cycle 026, Round 9 (Verifier)

## What I compared

- **Goal:** Fix expression statement lowering (FLS §8.2) so any expression can appear as a
  statement. Round 8 verifier contributed only a changelog — no code. This is the ninth
  independent re-verification pass.

**What I ran:**
- `cargo test` — 2115 pass, 0 fail ✓ (216 lib + 1842 e2e + 46 fls_fixtures + 11 smoke)
- `cargo run -- tests/fixtures/fls_9_functions.rs` → parsed 19 items, 0 failures, clean emit ✓

## What's here, what was asked

Matches. The work holds up against the goal from my comparative lens.

- The goal scenario (`fn returns_unit() { 42; }`) compiles without error — confirmed live.
- `fls_9_functions.rs` lowers all 19 items cleanly (was 19 of 20 at cycle start).
- Test count stable at 2115 — no regression across eight prior rounds.

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

- The `if`-expression-as-statement gap remains (documented in round 2 findings). Good
  candidate for a dedicated §6.17 cycle.
- None.

---

# Verification — Cycle 026, Round 8 (Verifier)

## What I compared

- **Goal:** Fix expression statement lowering (FLS §8.2) so any expression can appear as a
  statement. Round 7 verifier contributed only a changelog — no code. This is the eighth
  independent re-verification pass.

**What I ran:**
- `cargo test` — 2115 pass, 0 fail ✓ (216 lib + 1842 e2e + 46 fls_fixtures + 11 smoke)
- `cargo clippy -- -D warnings` — clean ✓
- `cargo run -- tests/fixtures/fls_9_functions.rs` → parsed 19 items, 0 failures, clean emit ✓

## What's here, what was asked

Matches. The work holds up against the goal from my comparative lens.

- The goal scenario (`fn returns_unit() { 42; }`) compiles without error — confirmed live.
- `fls_9_functions.rs` lowers all 19 items cleanly (was 19 of 20 at cycle start).
- Test count stable at 2115 — no regression across seven prior rounds.

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

- The `if`-expression-as-statement gap remains (documented in round 2 findings). Good
  candidate for a dedicated §6.17 cycle.
- None other.

---

# Verification — Cycle 026, Round 7 (Verifier)

## What I compared

- **Goal:** Fix expression statement lowering (FLS §8.2) so any expression can appear as a
  statement. Round 6 verifier contributed only a changelog — no code. This is the seventh
  independent re-verification pass.

**What I ran:**
- `cargo test` — 2115 pass, 0 fail ✓ (216 lib + 1842 e2e + 46 fls_fixtures + 11 smoke)
- `cargo clippy -- -D warnings` — clean ✓
- `cargo run -- tests/fixtures/fls_9_functions.rs` → parsed 19 items, 0 failures, clean emit ✓

## What's here, what was asked

Matches. The work holds up against the goal from my comparative lens.

- The goal scenario (`fn returns_unit() { 42; }`) compiles without error — confirmed live.
- `fls_9_functions.rs` lowers all 19 items cleanly (was 19 of 20 at cycle start).
- Test count stable at 2115 — no regression across six prior rounds.

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

- The `if`-expression-as-statement gap remains (documented in round 2 findings). Good
  candidate for a dedicated §6.17 cycle.
- None other.

---

# Verification — Cycle 026, Round 6 (Verifier)

## What I compared

- **Goal:** Fix expression statement lowering (FLS §8.2) so any expression can appear as a
  statement. Round 5 verifier contributed only a changelog — no code. This round is the
  sixth independent re-verification pass.

**What I ran:**
- `cargo test` — 2115 pass, 0 fail ✓ (216 lib + 1842 e2e + 46 fls_fixtures + 11 smoke)
- `cargo clippy -- -D warnings` — clean ✓
- `cargo run -- tests/fixtures/fls_9_functions.rs` → 19 items, 0 failures, clean emit ✓

## What's here, what was asked

Matches. The work holds up against the goal from my comparative lens.

- The goal scenario (`fn returns_unit() { 42; }`) compiles without error — confirmed live.
- `fls_9_functions.rs` now lowers all 19 items cleanly (was 19 of 20 at cycle start).
- Test count stable at 2115 — no regression.

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

- The `if`-expression-as-statement gap remains (documented in round 2 findings). Good
  candidate for a dedicated §6.17 cycle.
- None.

---

# Verification — Cycle 026, Round 5 (Verifier)

## What I compared

- **Goal:** Fix expression statement lowering (FLS §8.2) so any expression can appear as a
  statement. Round 4 verifier contributed only a changelog (documented convergence) — no code.
- **This round:** Independent re-verification of the converged state.

**What I ran:**
- `cargo test` — 2115 pass, 0 fail ✓ (216 lib + 1842 e2e + 46 fls_fixtures + 11 smoke)
- `cargo clippy -- -D warnings` — clean ✓
- `cargo run -- tests/fixtures/fls_9_functions.rs` → 19 items, 0 failures, clean emit ✓
- `cargo run -- tests/fixtures/fls_8_2_expression_statements.rs` → 8 items, 0 failures ✓
- `cargo test --test e2e fls_8_2 -- --nocapture` → 4 tests, all pass ✓

## What's here, what was asked

Matches. The work holds up against the goal from my comparative lens.

- The goal scenario (`fn returns_unit() { 42; }`) compiles without error — confirmed live.
- `fls_9_functions.rs` now lowers all 19 items cleanly (was 19 of 20 at cycle start).
- All four expression-statement shapes tested: integer literal, binary expr, plain block,
  named block.
- No regression in the broader suite.

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

- The `if`-expression-as-statement gap remains (documented in round 2 findings). Good
  candidate for a dedicated §6.17 cycle.
- None other.

---

# Verification — Cycle 026, Round 4 (Verifier)

## What I compared

- **Goal:** Fix expression statement lowering (FLS §8.2) so any expression can appear as a
  statement. The round 3 builder contribution was the changelog only — documenting the
  cycle converging.
- **Round 3 state:** All three block-like expression kinds (`Block`, `UnsafeBlock`,
  `NamedBlock`) handle `infer_natural_ty`; 4 e2e tests cover the four expression-statement
  shapes; fixture has 8 functions all lowering cleanly.

**What I ran:**
- `cargo test` — 2115 pass, 0 fail ✓ (216 lib + 1842 e2e + 46 fls_fixtures + 11 smoke)
- `cargo clippy -- -D warnings` — clean ✓
- `cargo run -- tests/fixtures/fls_9_functions.rs` → 19 items, 0 failures, clean emit ✓
- `cargo run -- tests/fixtures/fls_8_2_expression_statements.rs` → 8 items, 0 failures ✓
- `cargo test --test e2e fls_8_2 -- --nocapture` → 4 tests, all pass ✓

## What's here, what was asked

Matches. The work holds up against the goal from my comparative lens.

- The goal scenario (`fn returns_unit() { 42; }`) compiles without error — confirmed live.
- All expression-statement shapes tested: integer literal, binary expr, plain block, named block.
- No regression in the broader suite.

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

- The `if`-expression-as-statement gap remains (documented in round 2 findings). Good
  candidate for a dedicated §6.17 cycle.
- None other.

---

# Verification — Cycle 026, Round 3 (Verifier)

## What I compared

- **Goal:** Fix expression statement lowering (FLS §8.2) so any expression can appear as a
  statement. The named-block variant `'l: { 42 };` was the remaining gap coming into this
  round, addressed by the builder in round 2.
- **Builder's change (round 2):** `ExprKind::NamedBlock` arm added to `infer_natural_ty` in
  `src/lower.rs`, mirroring the existing `Block`/`UnsafeBlock` arm. Fallthrough comment
  corrected. Fixture function `discard_named_block_expr()` added.
  E2e test `fls_8_2_named_block_expr_stmt_emits_runtime_instr` added.

**What I ran:**
- `cargo test` — 2115 pass, 0 fail ✓ (216 lib + 1842 e2e + 46 fls_fixtures + 11 smoke)
- `cargo clippy -- -D warnings` — clean ✓
- `cargo run -- tests/fixtures/fls_9_functions.rs` → 19 items, 0 failures ✓ (wall from
  cycle start is gone — `returns_unit()` now lowers cleanly)
- `cargo run -- tests/fixtures/fls_8_2_expression_statements.rs` → 8 items, 0 failures ✓
- `cargo test --test e2e fls_8_2 -- --nocapture` → 4 tests, all pass ✓

## What's here, what was asked

Matches. The work holds up against the goal from my comparative lens.

- All three block-like expression kinds (`Block`, `UnsafeBlock`, `NamedBlock`) recurse into
  their tail in `infer_natural_ty` — the structural symmetry is complete.
- The original goal scenario (`fn returns_unit() { 42; }`) compiles without error.
- 4 e2e tests cover the four expression-statement shapes: integer literal, binary expr,
  plain block, named block — all assert runtime instruction emission (no const folding).
- Fixture file has 8 functions, all lowering cleanly.
- The `if`-expression-as-statement gap (noted in round 2 findings) is pre-existing and
  out of scope for this cycle. No regression introduced.

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

- The `if`-expression-as-statement gap remains: `fn foo(x: i32) { if x > 0 { 42 } else { 0 }; }`
  still fails with "integer literal with non-integer type". Root cause: `lower_expr` passes
  `IrTy::Unit` into the `if` branches; `infer_natural_ty` would need an `If` arm, and the
  `if` lowering path would need to pass the inferred type into both branch bodies. A deeper
  fix than a one-liner — good candidate for a dedicated §6.17 cycle.
- None other.

---

# Verification — Cycle 026, Round 2 (Verifier)

## What I compared

- **Goal:** Fix expression statement lowering (FLS §8.2) so that any expression can appear
  as a statement. Specifically `fn returns_unit() { 42; }` and `fn foo(x: i32) { x + 1; }`
  must lower successfully. The named-block variant `'l: { 42 };` was the remaining gap
  after round 1.
- **Builder's change:** Added `ExprKind::NamedBlock { body, .. }` arm to `infer_natural_ty`
  mirroring the existing `Block`/`UnsafeBlock` arm; corrected the fallthrough comment.
  Added fixture function `discard_named_block_expr()` and e2e test
  `fls_8_2_named_block_expr_stmt_emits_runtime_instr`.

**What I ran:**
- `cargo test` — 2115 pass, 0 fail ✓
- `cargo clippy -- -D warnings` — clean ✓
- `cargo run -- tests/fixtures/fls_8_2_expression_statements.rs` → 8 items, 0 failures ✓
- `cargo run -- tests/fixtures/fls_9_functions.rs` → 19 items, 0 failures (was 19 of 20 at
  cycle start — the original wall is gone) ✓
- `cargo test --test e2e fls_8_2 -- --nocapture` → 4 tests, all pass ✓
- Adversarial probe: `named_block_with_param(x: i32) { 'l: { x + 1 }; }`,
  `named_block_nested() { 'outer: { 'inner: { 42 }; }; }`,
  `named_block_no_tail() { 'l: { let _x = 1; }; }` — all compile and emit runtime
  instructions (not folded constants) ✓
- Inspected `infer_natural_ty` (lower.rs:10664–10714): Block, UnsafeBlock, NamedBlock all
  handled; fallthrough comment is now accurate ✓

## What's here, what was asked

Matches. The work holds up against the goal from my comparative lens.

- `fls_9_functions.rs` (the opening scenario) now compiles cleanly: 19 items, 0 failures.
- All three block-like expression kinds (`Block`, `UnsafeBlock`, `NamedBlock`) recurse into
  their tail in `infer_natural_ty`.
- The adversarial probe (parameter in named block) emits a runtime `ldr`/`add` sequence —
  no constant folding.
- The fallthrough comment no longer names "named block" as a kind handled by `IrTy::Unit`.
- 2115 tests pass; clippy clean.

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

- One related gap surfaced during adversarial probing: `if` expressions used as statements
  (`fn foo(x: i32) { if x > 0 { 42 } else { 0 }; }`) still hit "not yet supported:
  integer literal with non-integer type". The root cause is the same class — `lower_expr`
  receives the `if` with `IrTy::Unit`, and the literal arms inside the branches fail. This
  is a pre-existing gap, not introduced this cycle. A future goal could extend
  `infer_natural_ty` to cover `ExprKind::If` (recurse into the then-branch tail), but the
  fix is deeper: `lower_expr`'s `if` handler would need to pass the inferred type to both
  branch bodies. That's a new lowering path, not a one-liner.
- None other.

---

# Changelog — Cycle 026, Round 2 (Builder)

## Goal
- Fix expression statement lowering (FLS §8.2) so any expression can appear as a statement.

## Who This Helps
- **Stakeholder:** Lead Researcher
- **Impact:** The named-block-as-statement edge case — `'l: { 42 };` — no longer produces
  "integer literal with non-integer type". All three block-like expression kinds now
  propagate their tail type in `infer_natural_ty`.

## Applied
**Root cause fixed:** The verifier's round 1 addition covered `ExprKind::Block` and
`ExprKind::UnsafeBlock` in `infer_natural_ty`. `ExprKind::NamedBlock` fell to the
`_ => IrTy::Unit` default, producing the same error for `'l: { 42 };`.

Added `ExprKind::NamedBlock { body, .. }` arm (src/lower.rs, adjacent to the Block arm)
that recurses into `body.tail` when present, returning `IrTy::Unit` when the body has
no tail. This mirrors the Block arm exactly.

Also corrected the comment at the `_` fallthrough that incorrectly named "named block"
as one of the kinds "already handled correctly with IrTy::Unit".

**Files changed:**
- `src/lower.rs` — `NamedBlock` arm in `infer_natural_ty` + corrected fallthrough comment
- `tests/fixtures/fls_8_2_expression_statements.rs` — added `discard_named_block_expr()`
- `tests/e2e.rs` — added `fls_8_2_named_block_expr_stmt_emits_runtime_instr`

## Validated
- `cargo test` — **2115 pass, 0 fail** (was 2114; 1 new test added)
- `cargo clippy -- -D warnings` — clean
- `cargo run -- tests/fixtures/fls_8_2_expression_statements.rs` → 8 items, 0 failures
- PR: libliflin/galvanic#460

**Where the verifier should look:**
- `src/lower.rs:10692` — `NamedBlock` arm in `infer_natural_ty`
- `tests/e2e.rs` near end — `fls_8_2_named_block_expr_stmt_emits_runtime_instr`
- `cargo run -- tests/fixtures/fls_8_2_expression_statements.rs` — clean is the signal

---

# Verification — Cycle 026, Round 1 (Verifier)

## What I compared

- **Goal:** Fix §8.2 expression statement lowering so `42;` and `x + 1;` lower successfully with runtime instructions, not the uninformative "integer literal with non-integer type" error.
- **Builder's diff:** `infer_natural_ty` on `LowerCtx`, `StmtKind::Expr` updated, two e2e tests, new fixture, parse acceptance test, smoke test updated.
- **What I ran:**
  - `cargo test` — 2113 pass, 0 fail ✓
  - `cargo clippy -- -D warnings` — clean ✓
  - `cargo run -- tests/fixtures/fls_9_functions.rs` → clean emission, 0 failures ✓ (was 1 failed)
  - `cargo run -- tests/fixtures/fls_8_2_expression_statements.rs` → clean emission ✓
  - `cargo test --test e2e fls_8_2 -- --nocapture` → both tests pass ✓
  - Read emitted assembly for `discard_integer_literal`: `mov x0, #42` present ✓
  - Read emitted assembly for `discard_binary_expr`: `add` present, no constant fold ✓
  - Probed edge case: `{ 42 };` (block expression as statement) — **failed** with same error class

## What's here, what was asked

The goal as stated is met: `42;` and `x + 1;` lower cleanly. Gap found at the boundary: `{ 42 };` (block expression as a statement, FLS §6.4 tail expression in §8.2 context) triggered the same "integer literal with non-integer type" failure. `infer_natural_ty` returned `IrTy::Unit` for `ExprKind::Block`, which propagated to the block's tail `42` via `lower_block_to_value`. This is the same class of bug, one expression-kind deeper.

## What I added

**`src/lower.rs` — block expression case in `infer_natural_ty`:**
Added `ExprKind::Block(block) | ExprKind::UnsafeBlock(block)` arm that recurses into `block.tail` (if present) to propagate the tail's natural type, rather than returning `IrTy::Unit`. This closes the same failure for `{ 42 };`, `{ x + 1 };`, etc.

**`tests/e2e.rs` — `fls_8_2_block_expr_stmt_emits_runtime_instr`:**
Assembly inspection test asserting that `fn block_stmt() { { 42 }; }` emits `block_stmt:` and `#42` in the output.

**`tests/fixtures/fls_8_2_expression_statements.rs` — `discard_block_expr`:**
Added `fn discard_block_expr() { { 42 }; }` as the boundary case just beyond the direct-literal case.

- Files: `src/lower.rs`, `tests/e2e.rs`, `tests/fixtures/fls_8_2_expression_statements.rs`
- `cargo test` after additions: **2114 pass, 0 fail** (was 2113; 1 new test added)

## Notes for the goal-setter

- **Named blocks as statements:** `'label: { 42 };` falls to `_ => IrTy::Unit` in `infer_natural_ty`. The `ExprKind::NamedBlock` variant is not yet covered. Same fix would apply (recurse into the named block's tail). Scope is narrow — worth a future goal if named-block statements appear in fixtures.
- **Parenthesized expressions as statements:** `(42);` — the `ExprKind::Paren` (if one exists) or however the parser represents it also falls to `_ => IrTy::Unit`. Low priority since parenthesized expression statements are uncommon.
- The `#42` e2e assertion is sound — witnessed `mov x0, #42` in the emitted assembly for `discard_integer_literal`. No false-positive risk.

---

# Changelog — Cycle 026, Round 1 (Builder)

## Goal
Fix expression statement lowering (FLS §8.2) so that any expression can
appear as a statement. `fn returns_unit() { 42; }` and `fn foo(x: i32) { x + 1; }`
must lower successfully, emitting runtime instructions for the discarded
expressions without folding.

## Who This Helps
- **Stakeholder:** Lead Researcher
- **Impact:** `fls_9_functions.rs` — the spine-of-the-spec fixture — now lowers
  20 of 20 functions cleanly. Previously it was 19 of 20, with `returns_unit()`
  failing on a message that gave no FLS citation and no navigable context. The
  wall is gone.

## Applied
**Root cause fixed:** `StmtKind::Expr` in `src/lower.rs` was calling
`lower_expr(expr, &IrTy::Unit)` for all expression statements. The `LitInt`
arm in `lower_expr` dispatches on `ret_ty` and has no case for `IrTy::Unit`,
so integer literals as statements hit the catch-all:
`Err("integer literal with non-integer type")` — no FLS citation, no context.
The same issue affected any binary expression whose leftmost leaf is a literal.

**Structural fix:** Added `fn infer_natural_ty(&self, expr: &Expr) -> IrTy` on
`LowerCtx` (at `src/lower.rs:10638`). This method returns the natural type of
an expression for use in statement position:
- `LitInt` → `IrTy::I32` (Rust default for unsuffixed integer literals)
- `LitFloat` with `f32`/`_f32` suffix → `IrTy::F32`; otherwise `IrTy::F64`
- `LitBool` → `IrTy::Bool`
- `LitChar` → `IrTy::U32`
- `LitStr` → `IrTy::I32` (length)
- `Binary { op: Assign }` → `IrTy::Unit` (assignment is unit-typed)
- `Binary` (arithmetic/bitwise) → recurse into left operand
- `Unary` → recurse into operand
- `Path` (single segment) → `IrTy::F64` if in `float_locals`, `IrTy::F32` if
  in `float32_locals`, else `IrTy::I32`
- Everything else (if, loop, return, break, continue, call, match, …) →
  `IrTy::Unit` — these were already working with `IrTy::Unit` and must
  continue to use it

Updated `StmtKind::Expr` to call `infer_natural_ty` and pass that type to
`lower_expr`, replacing the unconditional `IrTy::Unit`.

Updated `tests/smoke.rs`: `lower_error_names_failing_item` was using
`fls_9_functions.rs` expecting a lower error. Since that fixture now lowers
cleanly, updated the test to use `fls_4_14_where_clauses_on_types.rs` which
still has unsupported constructs (method calls on primitive types).

**Files changed:**
- `src/lower.rs` — `infer_natural_ty` method + updated `StmtKind::Expr`
- `tests/fixtures/fls_8_2_expression_statements.rs` — new §8.2 fixture
- `tests/fls_fixtures.rs` — `fls_8_2_expression_statements` parse acceptance test
- `tests/e2e.rs` — two assembly inspection tests:
  `fls_8_2_integer_literal_stmt_emits_runtime_instr`,
  `fls_8_2_binary_expr_stmt_emits_runtime_instr`
- `tests/smoke.rs` — updated `lower_error_names_failing_item` fixture reference

## Validated
- `cargo test` — **2113 pass, 0 fail** (was 2110; 3 new tests added)
- `cargo clippy -- -D warnings` — clean
- `cargo run -- tests/fixtures/fls_9_functions.rs` → `galvanic: emitted fls_9_functions.s`
  (no errors; was `lowered 19 of 20 functions (1 failed)`)
- PR: libliflin/galvanic#458

**Where the verifier should look:**
- `src/lower.rs:10638` — `infer_natural_ty` method
- `src/lower.rs` near line 10545 — updated `StmtKind::Expr`
- `tests/e2e.rs` end-of-file — `fls_8_2_integer_literal_stmt_emits_runtime_instr`
  and `fls_8_2_binary_expr_stmt_emits_runtime_instr`
- `cargo run -- tests/fixtures/fls_9_functions.rs` — clean output is the moment

---

# Cycle 026 — Customer Champion

## Stakeholder

**Lead Researcher** — last served cycle 022, most under-served in the current rotation.

## Experience

Walked the Lead Researcher's first-encounter journey. Ran `cargo test` (2110 pass, clean
floor). Picked `fls_9_functions.rs` as the target fixture — §9 (Functions) is the spine
of the spec. Ran `cargo run -- tests/fixtures/fls_9_functions.rs`.

Output:

```
galvanic: compiling fls_9_functions.rs
parsed 19 item(s)
error: lower failed in 'returns_unit': not yet supported: integer literal with non-integer type
lowered 19 of 20 functions (1 failed)
galvanic: emitted tests/fixtures/fls_9_functions.s (partial — some functions failed)
```

The failing function:

```rust
fn returns_unit() {
    42;
}
```

Traced the root cause to `src/lower.rs:10482–10484`: `StmtKind::Expr` calls
`lower_expr(expr, &IrTy::Unit)` for all expression statements. The `LitInt` arm at
line 10654 dispatches on the type hint and has no case for `IrTy::Unit`, hitting the
catch-all `_ => Err("integer literal with non-integer type")`. The error has no FLS
citation, violating the architecture invariant — and the static citation check
(`lower_source_all_unsupported_strings_cite_fls`) doesn't catch this because the check
looks for the text "not yet supported" in source lines, but message payloads don't contain
that prefix (it's added by Display).

## Goal Set

**Fix expression statement lowering (FLS §8.2)** so that any expression can appear as a
statement. `fn returns_unit() { 42; }` and `fn foo(x: i32) { x + 1; }` must lower
successfully, emitting runtime instructions for the discarded expressions without folding.

The fix is in `StmtKind::Expr` handling: do not pass `IrTy::Unit` as the type hint for
expressions whose values are discarded. The expression should be lowered with its natural
type, since the statement context only means the result is unused — not that the expression
is unit-typed.

## Why Now

FLS §9 (Functions) is the core section. A failure on a three-line function with an
expression statement is a wall, not a partial limitation. The error message gives the
researcher nothing to navigate with: no FLS citation, no context, no spec anchor. And
the failing construct — an integer literal as a statement — is one of the most common
patterns in real Rust code.

---

# Verification — Cycle 025, Round 2 (Verifier)

## What I compared

- **Goal:** Fix the §6.5.7 entry in `refs/fls-ambiguities.md` so the formal "Galvanic's
  choice" field and assembly signature agree, remove the stale Note, add the narrow-type
  false negative as a known gap in §6.5.7 and §6.5.9, update `lower.rs:11044`. Documentation
  only.
- **Round 1 verifier finding:** Work holds up — no additions made.
- **This round:** Independent re-verification of the same artifacts.

**What I ran:**
- `cargo test` — 2110 pass, 0 fail ✓
- `cargo clippy -- -D warnings` — clean ✓
- Read `refs/fls-ambiguities.md` §6.5.7 entry (lines 546–576) end-to-end ✓
- Grepped `stale|Note: the` in `refs/fls-ambiguities.md` — the contradicting Note is gone;
  the only "stale" hit (line 1230) is in an unrelated section ✓
- Read §6.5.9 entry (lines 579–620) — "Three distinct gaps", gap 3 cross-references §6.5.7 ✓
- Read `src/lower.rs:11044` — AMBIGUOUS annotation describes the guard approach and names the
  false negative, pointing to `refs/fls-ambiguities.md §6.5.7 and §6.5.9 (gap 3)` ✓
- Checked source citations: `codegen.rs:1015` resolves to `// FLS §6.5.7: Shift operator
  expressions.` (start of shift handling block). `lower.rs:11044` resolves to the AMBIGUOUS
  comment itself. Both accurate ✓
- Checked `codegen.rs:1023` framing — describes the guard (`cmp x{rhs}, #64`, `b.hs
  _galvanic_panic`) and names the false negative. Matches `lower.rs` framing ✓

## What's here, what was asked

Matches. The work holds up against the goal from my comparative lens.

- §6.5.7 "Galvanic's choice" field states the explicit range guard behavior; the assembly
  signature confirms it; no contradicting Note remains.
- "Known gap" paragraph names the i32/[32,63] false negative explicitly with the correct
  mechanics.
- §6.5.9 gap 3 is present and correctly cross-references §6.5.7.
- Three previously-divergent framings (codegen.rs, lower.rs, fls-ambiguities.md) now agree.
- No stale content introduced elsewhere.

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

- The §6.5.7 "Known gap" and §6.5.9 "gap 3" document the false negative but neither names a
  fix path. A future goal could close this structurally: emit `cmp x{rhs}, #32` when the
  operand type is `i32`/`u32` (and analogously for `i8`/`u8`/`i16`/`u16`). The current
  documentation is correct and citable — this is a lead, not a blocker.
- None other.
