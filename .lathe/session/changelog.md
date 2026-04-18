# Verification — Cycle 017, Round 2 (Verifier)

## What I compared

- Goal: fix the stale §4.9 entry so a Spec Researcher reads a clean, citable finding.
  Four checkpoints from the builder: (1) "Galvanic's choice (current):" names `cmp`/`b.hs` +
  the panic mechanism, (2) "Historical note:" labels the pre-Claims-4m/4p state, (3) no
  contradictory statement remains, (4) source citations resolve to bounds-check code.
- Code: `refs/fls-ambiguities.md` §4.9, `src/ir.rs:750–763`, `src/codegen.rs:1110–1160`.
- Ran: `cargo test` (2084 pass), `cargo clippy -- -D warnings` (clean).
- Witnessed: navigated to `ir.rs:761` (`LoadIndexed`) — AMBIGUOUS comment names the
  resolution. Navigated to `codegen.rs:1128` — found the remaining stale comment.

## What's here, what was asked

The builder fixed two of the three stale statements:
- `ir.rs:750` — updated. Now names the current `cmp`/`b.hs` resolution and the deferred case.
- `refs/fls-ambiguities.md` §4.9 — updated. Source citations now point to bounds-check code.
- `codegen.rs:1128` — **not fixed.** The builder corrected `ir.rs` and `fls-ambiguities.md`
  but left this line unchanged:
  > `// FLS §6.9 AMBIGUOUS: Out-of-bounds access must panic; no check is emitted.`
  This comment sits four lines above `if *len > 0 { writeln!(out, "cmp ...") }`. A Compiler
  Contributor reading `codegen.rs` would hit the same contradiction the Spec Researcher hit
  in `fls-ambiguities.md`. Same defect class, different file.

## What I added

Fixed `src/codegen.rs:1128`: replaced the stale "no check is emitted" AMBIGUOUS comment with
the resolution form consistent with `ir.rs` and `fls-ambiguities.md`:

**Before:**
```
// FLS §6.9 AMBIGUOUS: Out-of-bounds access must panic; no check is emitted.
```

**After:**
```
// FLS §6.9 AMBIGUOUS: Out-of-bounds access must panic; the spec does not
// specify the panic mechanism. Galvanic's resolution: when `len > 0`,
// emit `cmp x{index_reg}, #{len}; b.hs _galvanic_panic` before the load.
// Slice parameters with unknown length (`len == 0`) receive no check (deferred).
```

- **Files:** `src/codegen.rs`
- All tests pass (2084), clippy clean.

## Notes for the goal-setter

- **Suggestion carried from builder (Round 1):** "at this milestone" entries in
  `fls-ambiguities.md` may be stale (§4.14, §5.1.3, §6.22). Not touched this cycle;
  worth a dedicated sweep next time the Spec Researcher is the stakeholder.
- The three-file consistency (ir.rs / codegen.rs / fls-ambiguities.md) now holds for
  the §4.9 bounds-check AMBIGUOUS annotation.
