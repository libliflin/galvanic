# Changelog — Cycle 029, Round 1 (Builder)

## Goal
Remove the "Claim 4m / 4n / 4o / 4p / 4q" labels from `refs/fls-ambiguities.md`
and `refs/fls-constraints.md`. Update the stale footer count (46→48). Content stays;
only the opaque labels go.

## Who This Helps
- **Stakeholder:** Spec Researcher
- **Impact:** The Claim labels were internal CI-verification tracking artifacts from
  lathe cycles 007–021 that were incorporated into the live document but never explained.
  They made three well-described entries look like they were missing a companion document.
  Removing them lets the researcher read each entry as a complete, self-contained finding
  without encountering unexplained references.

## Applied
Three files changed — documentation only, no code touched:

**`refs/fls-ambiguities.md`:**
- §6.9/§6.23 entry: removed `(updated — Claims 4m, 4o, 4p, 4q)` from the header and
  individual `(Claim 4m)`, `(Claim 4o)`, `(Claim 4q)`, `(Claim 4p)` labels from the
  four bullet points.
- §6.21 entry: removed `(Claim 4n)` from the "Galvanic's choice" field.
- Footer: updated `46 entries` → `48 entries` and date to 2026-04-18.

**`refs/fls-constraints.md`:**
- §6.23 paragraph: removed `Claims 4m, 4o, 4q` from the cross-reference to
  `refs/fls-ambiguities.md §6.9/§6.23`.

## Validated
- `cargo test` — **2116 pass, 0 fail** (documentation-only change; no test changes expected)
- `grep "Claim 4" refs/fls-ambiguities.md refs/fls-constraints.md` — empty (no labels remain)

**Where the verifier should look:**
- `refs/fls-ambiguities.md` lines ~629–641 (§6.9/§6.23 entry) — "Galvanic's choice:" header
  and four bullets should have no Claim labels
- `refs/fls-ambiguities.md` line ~949 (§6.21 entry) — "Galvanic's choice:" should have no
  Claim label
- `refs/fls-ambiguities.md` footer — should read "48 entries"
- `refs/fls-constraints.md` line ~88 — cross-reference should end with "(§6.9/§6.23 — Panic
  Mechanism)." without Claim labels
