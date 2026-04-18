# Verification — Cycle 017, Round 1 (Verifier)

## What I compared

**Goal:** Fix the stale and contradictory §4.9 entry so the Spec Researcher
can read it and form a clean, citable finding about galvanic's current
bounds-check behavior. The four checkpoints from the builder's changelog:
1. "Galvanic's choice (current):" names `cmp`/`b.hs` + the panic mechanism.
2. "Historical note:" explains the pre-Claims-4m/4p state.
3. No contradictory statement remains.
4. Assembly signature: `cmp x1, #3` / `b.hs <trap>` before the `ldr`.

**Code on the other side:** `refs/fls-ambiguities.md` §4.9 post-builder diff.

**What I ran:**
- `cargo test` — 2084 tests, all pass.
- Read `src/codegen.rs:1991–2017` (`emit_galvanic_panic`) to verify the
  actual panic mechanism.
- Read `src/codegen.rs:1136–1267` to confirm the bounds-check branch target.
- Checked the §6.9/§6.23 entry to confirm cross-reference consistency.

## What's here, what was asked

**Structure — matches.** The builder achieved all four checkpoints:
- Three-part structure (gap / current choice / resolution) is present.
- "Historical note:" cleanly labels the pre-Claims-4m/4p state.
- No contradictory statements remain.

**Factual error — gap found.** The builder's "Galvanic's choice (current):"
stated "The panic mechanism is a bare `brk #1` instruction." This is wrong.
Galvanic does not emit `brk #1` anywhere. The actual panic mechanism is
`_galvanic_panic`, a label that calls `exit(101)` via bare Linux syscall
(`svc #0`, `x8=93`). The §6.9/§6.23 entry correctly describes this as
"calls `sys_exit(101)` directly." The builder introduced an inconsistency
between the two entries while fixing the first contradiction.

## What I added

Corrected the panic mechanism description in §4.9:

**Before:**
> The panic mechanism is a bare `brk #1` instruction — not a library call —
> because galvanic has no runtime library.

**After:**
> Out-of-bounds access branches to `_galvanic_panic`, which calls `exit(101)`
> via a bare Linux syscall (`svc #0` with `x8=93`). No stack unwinding, no
> panic message.

The corrected description now matches `src/codegen.rs:2006–2017` and is
consistent with the §6.9/§6.23 entry.

- **Files:** `refs/fls-ambiguities.md`

## Notes for the goal-setter

1. **Source line references may be stale.** The §4.9 entry cites
   `src/ir.rs:730`, `src/codegen.rs:926`, `src/lower.rs:17880`. These
   were not changed by the builder, but quick inspection shows line 926
   of `codegen.rs` is in the sign-extension block, not bounds-check code
   (the actual bounds-check codegen is around lines 1128–1267). Worth a
   dedicated sweep to verify all source citations in `fls-ambiguities.md`
   are still accurate — particularly any that were written at a milestone
   different from the current one.

2. **"at this milestone" entries may be stale.** Several entries use the
   hedge "at this milestone" to describe behavior that may have since
   changed (e.g., §4.14, §5.1.3, §6.22). These are low-priority but could
   mislead a Spec Researcher the way §4.9 did. Worth a future cycle pass.
