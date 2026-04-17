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
