# Verification — Cycle 017, Round 4 (Verifier)

## What I compared

- Goal: Four-file consistency for §6.9 AMBIGUOUS annotations (ast.rs / ir.rs / lower.rs / codegen.rs) — the Spec Researcher reads a clean, citable finding about bounds checking.
- Builder's Round 3 fixed `StoreIndexed` in `ir.rs` and `codegen.rs`. Verifier's Round 3 fixed `ast.rs`.
- Ran: `cargo test` (2084 pass), `cargo clippy -- -D warnings` (clean).
- Witnessed: grepped `FLS §4.9 AMBIGUOUS` and `FLS §6.9 AMBIGUOUS` across all four source files.

## What's here, what was asked

The four fixes from Rounds 1–3 are all present and correct. One additional defect found:

`src/lower.rs:18427–18428` read:
> "FLS §4.9 AMBIGUOUS: The FLS does not specify bounds checking. Galvanic omits bounds checking at this milestone."

Every other `§4.9 AMBIGUOUS` annotation in `lower.rs` (six sites) is about the `&[T]` fat-pointer ABI (two-register representation). A Spec Researcher tracking `§4.9` entries would encounter a bounds-checking note mixed in with ABI notes — it doesn't belong there thematically. And a Spec Researcher tracking `§6.9` entries would miss this code path's deferred bounds check entirely (since it cited `§4.9`).

The site is in the `local_slice_slots` branch — the code path that handles slice parameter indexing. `lower.rs:18341–18347` already carries a correct `§6.9 AMBIGUOUS` note describing this deferred case at the top of the match arm; the comment at 18427 was a redundant, mislabeled copy.

## What I added

Fixed `src/lower.rs:18427–18429`: corrected `§4.9` → `§6.9` and updated the comment to match the resolution pattern used by all other `§6.9 AMBIGUOUS` entries, with a cross-reference to the canonical note at the top of the match arm.

**Before:**
```
// FLS §4.9 AMBIGUOUS: The FLS does not specify bounds checking.
// Galvanic omits bounds checking at this milestone.
// FLS §6.1.2:37–45: All four instructions are runtime.
```

**After:**
```
// FLS §6.9 AMBIGUOUS: Slice parameters carry a runtime length, but
// galvanic does not yet use it for bounds checking here (deferred —
// see §6.9 AMBIGUOUS note at the top of this match arm for details).
// FLS §6.1.2:37–45: All four instructions are runtime.
```

- **Files:** `src/lower.rs`
- All tests pass (2084), clippy clean.
- All `§4.9 AMBIGUOUS` entries in `lower.rs` now exclusively address the `&[T]` fat-pointer ABI. Both `§6.9 AMBIGUOUS` entries address bounds checking. Thematic consistency holds.

## Notes for the goal-setter

- The §6.9 AMBIGUOUS sweep is now complete across all five annotation sites in the four source files.
- The "at this milestone" sweep (§4.14, §5.1.3, §6.22) flagged in earlier rounds remains a candidate for the next Spec Researcher cycle.
- None of the `§4.9 AMBIGUOUS` entries (fat-pointer ABI) have been touched; they are a separate ambiguity category and consistent with each other.
