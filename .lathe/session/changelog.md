# Verification — Cycle 017, Round 3 (Verifier)

## What I compared

- Goal: Fix the stale §4.9 entry so a Spec Researcher reads a clean, citable finding.
  Builder's Round 3 addressed `StoreIndexed` in `ir.rs` and `codegen.rs` — the parallel
  defect to the `LoadIndexed` fixes from Rounds 1–2.
- Code: `src/ir.rs` (`StoreIndexed` AMBIGUOUS comment), `src/codegen.rs` (StoreIndexed block),
  and a scan of all four files that carry §6.9 annotations: `ast.rs`, `ir.rs`, `lower.rs`, `codegen.rs`.
- Ran: `cargo test` (2084 pass), `cargo clippy -- -D warnings` (clean).
- Witnessed: grepped `FLS §6.9 AMBIGUOUS` across `src/`. Found the builder's changes in
  `ir.rs:788` and `codegen.rs:1179` — both correctly state the `cmp`/`b.hs` resolution with
  the deferred-slice caveat. Confirmed `lower.rs` was already current.

## What's here, what was asked

Builder's changes match the goal for `ir.rs` and `codegen.rs`. One instance missed:

`src/ast.rs:1385–1388` still read:
> "Bounds checking is not yet emitted — this is FLS §6.9 AMBIGUOUS: the spec does not
> specify the panic mechanism for out-of-bounds access without the standard library."

This is the same stale-claim defect the builder fixed in `ir.rs` (Round 1). `ast.rs` is the
AST definition file; a Compiler Contributor reading it before touching `lower.rs` or
`codegen.rs` would hit the identical wall the Spec Researcher hit in the original
`fls-ambiguities.md` entry. Four files carry §6.9 annotations; three were updated; one remained.

## What I added

Fixed `src/ast.rs:1385–1388`: replaced "Bounds checking is not yet emitted" with the
resolution-form comment consistent with `ir.rs`, `lower.rs`, and `codegen.rs`:

**Before:**
```
/// At this milestone the index is treated as an `i32` (runtime value).
/// Bounds checking is not yet emitted — this is
/// FLS §6.9 AMBIGUOUS: the spec does not specify the panic mechanism for
/// out-of-bounds access without the standard library.
```

**After:**
```
/// At this milestone the index is treated as an `i32` (runtime value).
/// FLS §6.9 AMBIGUOUS: The spec requires a panic on out-of-bounds access
/// but does not specify the panic mechanism. Galvanic's resolution: codegen
/// emits `cmp x{index}, #{len}; b.hs _galvanic_panic` before each indexed
/// load/store when the array length is statically known (`len > 0`).
/// Slice parameters with unknown length receive no check (deferred).
```

- **Files:** `src/ast.rs`
- All tests pass (2084), clippy clean.

## Notes for the goal-setter

- The four-file consistency (ast.rs / ir.rs / lower.rs / codegen.rs) for §6.9 AMBIGUOUS
  now holds across all sites. The builder's three-file check (ir.rs / codegen.rs /
  fls-ambiguities.md) was the right scope; `ast.rs` was the one site not in scope.
- The "at this milestone" sweep (§4.14, §5.1.3, §6.22) flagged in earlier rounds remains
  a candidate for the next Spec Researcher cycle.
