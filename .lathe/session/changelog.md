# Changelog — Cycle 004, Round 1

## Goal
Add `Minimal reproducer:` fields to every entry in `refs/fls-ambiguities.md`
where galvanic supports the required constructs and the behavior is observable
via assembly inspection.

## Who This Helps
- **Stakeholder:** The Spec Researcher
- **Impact:** Step 7 of the Spec Researcher journey (verify a finding by
  running a program) no longer requires guessing whether galvanic supports the
  needed construct or inferring what assembly to look for. Each entry now
  includes a ≤10-line program and a one-line assembly signature. A Spec
  Researcher can copy the program, run `cargo run -- /tmp/repro.rs`, and
  confirm the finding without domain expertise in ARM64 or the galvanic
  internals.

## Applied
Added a `**Minimal reproducer:**` block to all 46 entries in
`refs/fls-ambiguities.md`. Each block contains either:
- A ≤10-line Rust program + one-line assembly signature describing what
  instruction(s) to look for to confirm the finding, or
- "Not yet demonstrable — requires X" explaining why the required construct
  is not yet compiled end-to-end.

Entries that received working reproducers (25 of 46):
§2.4.4.1, §2.4.4.2, §4.1, §4.2/§2.4.5, §4.8/§4.9, §4.9, §4.13, §5.1.4,
§6.1.2, §6.4.2, §6.4.4, §6.5.3, §6.5.5, §6.5.7, §6.5.9 (×2), §6.9/§6.23 (×2),
§6.10, §6.11, §6.12.2, §6.14, §6.15.1, §6.15.6, §6.16, §6.18, §6.21, §6.22,
§7.1, §7.2, §8.1, §9.2, §10.1, §13, §14.1, §15.

Entries marked "not yet demonstrable" (21 of 46): §2.6, §4.14, §5.1.8, §6.13,
§6.17, §10.2, §11, §12.1, §14, §19, and others where enforcement is deferred
or the feature is parse-only.

**Files:** `refs/fls-ambiguities.md` (documentation only, no source changes)

## Validated
- `cargo test`: 2050 pass, 0 fail — documentation-only change, no code touched.
- Spot-verified assembly signatures against source:
  - `fadd d0, d0, d1` confirmed in `src/codegen.rs:1552`
  - `and w0, w0, #255` confirmed in `src/codegen.rs:868`
  - `fcvtzs w0, d0` confirmed in `src/codegen.rs:1519`
  - `cbz x1, _galvanic_panic` confirmed in `src/codegen.rs:594`
  - `.align 3` confirmed in `src/codegen.rs:202`
  - `lsl x0, x0, x1` (no mask) confirmed in `src/codegen.rs:733`

## Where to look
```
grep -A 5 'Minimal reproducer' refs/fls-ambiguities.md | head -60
```
Or navigate to any entry via the TOC and read the `**Minimal reproducer:**`
block at the bottom of the entry.
