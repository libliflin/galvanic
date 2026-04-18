# Verification — Cycle 025, Round 1 (Verifier)

## What I compared

- **Goal:** Fix the §6.5.7 entry in `refs/fls-ambiguities.md` so the formal "Galvanic's
  choice" field and the assembly signature agree, remove the stale Note, add the
  narrow-type false negative as a known gap in §6.5.7 and §6.5.9, and update the
  `lower.rs` annotation at line 11044 to match. Documentation only.

**What I ran:**
- `cargo test` — 2110 pass, 0 fail ✓
- `cargo clippy -- -D warnings` — clean ✓
- Read `refs/fls-ambiguities.md` §6.5.7 entry end-to-end: gap / Galvanic's choice /
  Known gap / Source / Minimal reproducer all consistent and citable ✓
- Grepped for "stale\|Note: the" in `refs/fls-ambiguities.md` — the contradicting Note
  is gone; no other §6.5.7 stale mention remains ✓
- Read `refs/fls-ambiguities.md` §6.5.9 entry: now shows "Three distinct gaps", gap 3
  cross-references §6.5.7 ✓
- Read `src/lower.rs:11044` — AMBIGUOUS annotation describes the guard approach and
  names the false negative, with a pointer to `refs/fls-ambiguities.md` ✓
- Checked source line citations: `codegen.rs:1015` points to `// FLS §6.5.7: Shift
  operator expressions.` — the start of the shift handling block. `lower.rs:11044`
  points to the AMBIGUOUS comment itself. Both citations resolve accurately ✓

## What's here, what was asked

Matches. The work holds up against the goal from my comparative lens.

- The §6.5.7 entry is now citable: "Galvanic's choice" states the explicit range guard
  behavior, the assembly signature confirms it, and no contradicting Note remains.
- The Known gap paragraph names the i32/[32,63] false negative explicitly with the
  correct mechanics (mod-32 vs mod-64).
- §6.5.9 gap 3 is added and correctly cross-references §6.5.7.
- `lower.rs:11044` annotation matches `codegen.rs:1023` in framing: both describe the
  guard approach and name the same false negative.
- Three places that previously described the same decision with three different framings
  now agree.

## What I added

Nothing this round — the work holds up against the goal from my lens.

## Notes for the goal-setter

- The §6.5.7 "Known gap" and §6.5.9 "gap 3" both document the false negative but
  neither names a fix path. A future goal could address this: emit `cmp x{rhs}, #32`
  when the operand type is i32/u32 (and similar for i8/u8/i16/u16). That would close
  the gap structurally. Not a blocker — the current entry is correct and citable as-is.
- None other.
