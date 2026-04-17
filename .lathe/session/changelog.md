# Changelog — Cycle 007

## Stakeholder: The Spec Researcher

Walked steps 4–7 of the Spec Researcher journey: searched source for AMBIGUOUS annotations,
cross-checked against `refs/fls-ambiguities.md`, and tried to navigate from annotation to
documented finding.

## What I experienced

Found 50 unique FLS section references with AMBIGUOUS annotations in source. Cross-checked
against the ref file TOC. Four sections had annotations with no matching ref entry:
`§4.7`, `§6.3.2`, `§6.6.1`, `§6.7`.

Investigation revealed 3 stale FLS section numbers (annotations using old FLS organization):
- `§6.3.2` in `src/ast.rs:1127` → finding is documented under current §6.12.2 (Method Auto-Deref)
- `§6.7` in `src/parser.rs:2270` → finding is documented under current §6.21 (Comparison Non-Associativity)
- `§6.6.1` in `src/parser.rs:2346` → no ref entry; current FLS §6.6 = Underscore Expressions, not bitwise AND

The `§6.6.1` annotation documents a genuine finding: the FLS defines `&` as both a borrow
operator and bitwise AND without specifying how a recursive-descent parser should disambiguate
them. Galvanic resolves this positionally. No ref entry exists for this finding.

## Goal set

Fix the 3 stale FLS section numbers in source AMBIGUOUS annotations:
- `src/ast.rs:1127`: update `§6.3.2` → `§6.12.2`
- `src/parser.rs:2270`: update `§6.7` → `§6.21`
- `src/parser.rs:2346`: update `§6.6.1` → correct current §6.5.x subsection for bitwise AND

Add a `refs/fls-ambiguities.md` entry for the `&` disambiguation finding (currently undocumented).

## Why

The cycle-004 goal added reproducers to all existing ref entries. The next barrier is
source-to-ref navigability: searching `grep -r 'AMBIGUOUS' src/ | grep '§6.3'` and arriving
at the documented finding. Three stale section numbers break this navigation silently — the
finding exists but is filed under a different number. Correct citations = unbroken search-and-navigate for the Spec Researcher.

Specific moment: Step 4, finding `§6.3.2` in source, looking up §6.3 in `refs/fls-pointer.md`
("Path Expressions"), searching the ref file — no §6.3.2 entry. Dead end. Same pattern for
§6.7 and §6.6.1.

---

## Applied

Fixed 3 stale AMBIGUOUS annotation section numbers and added 1 missing ref entry.

**`src/ast.rs`**
- Line 1120–1130: Updated all `§6.3.2` references → `§6.12.2` (Method Call Expressions).
  The fls-ambiguities.md entry was already filed under `§6.12.2 — Method Auto-Deref Step Limit`.

**`src/parser.rs`**
- Line 2268–2275 (`parse_cmp`): Updated all `§6.7` references → `§6.21` (Expression Precedence).
  The fls-ambiguities.md entry was already filed under `§6.21 — Comparison Non-Associativity`.
- Line 2344–2352 (`parse_bitand`): Updated `§6.6.1` → `§6.5.7` (Bit Expressions, confirmed
  via live FLS fetch). Clarified the annotation to name both §6.5.1 (Borrow) and §6.5.7 (Bit).

**`refs/fls-ambiguities.md`**
- Added TOC entry: `§6.5.7 — Bitwise AND Disambiguation: & as Borrow vs Bit AND`
- Added body entry with gap description, galvanic's resolution, source pointer, and minimal
  reproducer. Inserted before the existing §6.5.7 shift entry (both are valid §6.5.7 findings).

**Files modified:**
- `src/ast.rs`
- `src/parser.rs`
- `refs/fls-ambiguities.md`
- `.lathe/session/changelog.md`

## Validated

- `cargo test`: 2055 passed, 0 failed.
- The three AMBIGUOUS annotations now match their corresponding ref entries:
  - `grep -r 'AMBIGUOUS' src/ | grep '§6.12.2'` → finds `src/ast.rs`, ref entry exists at `§6.12.2`
  - `grep -r 'AMBIGUOUS' src/ | grep '§6.21'` → finds `src/parser.rs`, ref entry exists at `§6.21`
  - `grep -r 'AMBIGUOUS' src/ | grep '§6.5.7'` → finds `src/parser.rs`, ref entry now exists at `§6.5.7 — Bitwise AND Disambiguation`
- The §6.6.1 stale annotation is eliminated; no AMBIGUOUS annotation remains for a non-existent section.
