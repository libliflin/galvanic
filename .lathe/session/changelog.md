# Changelog — Cycle 025, Round 1 (Builder)

## Goal
Fix the §6.5.7 entry in `refs/fls-ambiguities.md` so it accurately describes
galvanic's current behavior (explicit panic guard, not hardware mod-64 delegation).
Remove the stale Note. Add the narrow-type false negative as a known gap in both
§6.5.7 and §6.5.9. Update the `lower.rs` source annotation to match.

## Who This Helps
- **Stakeholder:** Spec Researcher
- **Impact:** The §6.5.7 entry is now citable. The formal "Galvanic's choice" field
  and the assembly signature agree. The stale Note is gone. The known gap (i32 shifts
  in [32, 63]) is documented so researchers don't have to infer it from the assembly.

## Applied

**`refs/fls-ambiguities.md` — §6.5.7 entry rewritten:**
- "Galvanic's choice" now states the current behavior: explicit `cmp x{rhs}, #64` +
  `b.hs _galvanic_panic` guard, with negative amounts handled by unsigned comparison.
- "No masking instruction" accurately describes the absence of `AND`.
- Added a "Known gap" paragraph: shift amounts in [32, 63] on `i32` pass the guard
  (false negative — should panic for mod-32 semantics).
- Removed the stale Note that contradicted the formal field.
- Updated source citations from stale line numbers (`codegen.rs:594`, `lower.rs:10639`)
  to current ones (`codegen.rs:1015`, `lower.rs:11044`).

**`refs/fls-ambiguities.md` — §6.5.9 entry extended:**
- Added gap 3 (was "two distinct gaps", now "three distinct gaps"): the shift-amount
  false negative for narrow types, with a cross-reference to §6.5.7.

**`src/lower.rs:11044` — AMBIGUOUS annotation updated:**
- Old framing: "does not specify the exact register width used for the modulo (ARM64
  uses 6 bits)." This described the old hardware-delegation behavior.
- New framing: describes the guard approach and names the false negative for narrower
  types. Points to `refs/fls-ambiguities.md` for the full record.

Files: `refs/fls-ambiguities.md`, `src/lower.rs`

## Validated
- `cargo test` — 2110 pass, 0 fail ✓
- `cargo clippy -D warnings` — clean (documentation-only change, no new code) ✓
- Verifier: navigate to `refs/fls-ambiguities.md` §6.5.7. Read "Galvanic's choice"
  and the assembly signature — they now agree. No Note contradicting the formal field.
  The "Known gap" paragraph names the i32/[32,63] false negative explicitly.
- Verifier: check `lower.rs:11044` — the AMBIGUOUS comment now matches the codegen.rs
  description at line 1023.
