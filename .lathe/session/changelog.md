# Changelog — Cycle 017, Round 1 (Builder)

## Goal
Fix the stale and contradictory §4.9 (Bounds Checking Mechanism) entry in
`refs/fls-ambiguities.md` so the Spec Researcher can read it and form a
clean, citable finding about galvanic's current behavior.

## Who This Helps
- **Stakeholder:** Spec Researcher — a compiler author reading galvanic's
  research artifacts to find citable FLS findings.
- **Impact:** Before this fix, the §4.9 entry said "No bounds check is
  emitted" in the "Galvanic's choice" section, then immediately contradicted
  itself in an "Assembly signature" note that said bounds checks ARE emitted.
  A researcher reading the entry couldn't trust either statement. After this
  fix, the entry leads with current behavior and isolates the historical
  decision in a clearly labeled "Historical note" subsection.

## Applied
- Updated `refs/fls-ambiguities.md` §4.9 entry:
  - Replaced the stale "Galvanic's choice: No bounds check is emitted" with
    "Galvanic's choice (current):" describing the `cmp`/`b.hs` + `brk #1`
    mechanism now in place (Claims 4m/4p).
  - Moved the original no-bounds-check decision to a "Historical note:"
    subsection, clearly marked as resolved.
  - Removed the self-contradictory "Assembly signature" prose that buried
    the correction inside the same entry.
  - Updated the minimal reproducer assembly signature note to a clean
    one-liner matching actual emitted output.
- **File:** `refs/fls-ambiguities.md`

## Validated
- `cargo test` — all 2084 tests pass (no test touches fls-ambiguities.md
  directly; smoke/fls_fixtures/e2e remain green).
- `cargo build` — clean.
- `cargo clippy -D warnings` — clean.
- Verifier: navigate to `refs/fls-ambiguities.md` §4.9. Confirm:
  1. "Galvanic's choice (current):" names `cmp`/`b.hs` + `brk #1`.
  2. "Historical note:" explains the pre-Claims-4m/4p state.
  3. No contradictory statement remains.
  4. Assembly signature note: `cmp x1, #3` / `b.hs <trap>` before the `ldr`.
