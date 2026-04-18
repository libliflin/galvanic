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
