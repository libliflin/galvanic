# Changelog — Customer Champion Cycle 007

## Stakeholder: The Spec Researcher

**Became:** A Spec Researcher trying to trace AMBIGUOUS annotations in source to documented
findings in `refs/fls-ambiguities.md`. Walked steps 4–7 of the Spec Researcher journey.

**Rotation rationale:** Cycle 005 served the Compiler Contributor. Cycle 006 served the
Lead Researcher. The Spec Researcher was last served in cycle 004 (two cycles ago) — their
turn.

---

## Floor check

Build: OK. Tests: 2055 pass, 0 fail. Clippy: OK. Unsafe audit: OK. Floor intact.

---

## What I experienced

Walked step 4 of the Spec Researcher journey: searched source for AMBIGUOUS annotations and
tried to navigate to matching entries in `refs/fls-ambiguities.md`.

```
grep -r 'AMBIGUOUS' src/ | grep -oE '§[0-9]+(\.[0-9]+)*' | sort -u
```

Found 50 unique section references in source. Cross-checked against the ref file TOC.
Four sections with AMBIGUOUS annotations in source have no matching ref entry:
`§4.7`, `§6.3.2`, `§6.6.1`, `§6.7`.

Investigated each:

- **§6.3.2** (`src/ast.rs:1127`): "spec does not fully specify how many auto-deref steps are
  legal." In the current FLS (from `refs/fls-pointer.md`), §6.3 is "Path Expressions" — not
  method calls. The method-call finding IS documented in refs as `§6.12.2 — Method Auto-Deref
  Step Limit`. Stale section number.

- **§6.7** (`src/parser.rs:2270`): "spec says comparison operators are non-associative but this
  is a type-level constraint, not syntactic." In the current FLS, §6.7 is "Parenthesized
  Expressions". The comparison non-associativity finding IS documented in refs as `§6.21 —
  Comparison Non-Associativity: Chained Comparisons`. Stale section number.

- **§6.6.1** (`src/parser.rs:2346`): "`&` is overloaded — in unary position it is a borrow
  operator; in binary position it is bitwise AND. The disambiguation is positional: `parse_bitand`
  is only entered after a LHS has been fully parsed." In the current FLS, §6.6 is "Underscore
  Expressions" — not bitwise operators. **No matching ref entry exists for this finding.**

- **§4.7** (`src/lower.rs:3754`): partially absorbed into the §4.8 ref entry (fat pointer ABI).

**The worst moment:** Picking §6.3.2 from the grep output, looking it up in `refs/fls-pointer.md`
(§6.3 = Path Expressions), searching refs/fls-ambiguities.md for a §6.3.2 entry — and finding
nothing. The annotation is not abandoned: it's documented under §6.12.2. But the wrong section
number means the search-and-navigate workflow silently breaks. A Spec Researcher would not
know whether this is an unfiled finding or an annotation error.

**The hollowest moment:** §6.7. The annotation is real: galvanic's parser currently parses
`a < b < c` as `(a < b) < c` (left-associative), deferring rejection to the type checker,
which matches the §6.21 ref entry. But the annotation uses §6.7 — which in the current FLS is
"Parenthesized Expressions". A Spec Researcher who searches source for `§6.21` finds the
lowering-stage annotation; one who searches for `§6.7` finds the parser-level annotation; these
are the same finding viewed from different pipeline stages, but only one section number navigates
to the ref.

---

## Goal

**Fix the 3 stale FLS section numbers in source AMBIGUOUS annotations and add a ref entry for
the genuinely undocumented `&`-disambiguation finding.**

### What to change

**1. `src/ast.rs:1127`:** The `MethodCall` variant's AMBIGUOUS annotation currently cites
`§6.3.2`. The current FLS §6.3 is "Path Expressions"; method calls are §6.12.2. Update both
the non-ambiguous citation and the AMBIGUOUS annotation to `§6.12.2`.

**2. `src/parser.rs:2270`:** The `parse_cmp` function doc currently cites `§6.7` for comparison
operators. The current FLS §6.7 is "Parenthesized Expressions"; expression precedence and
non-associativity are §6.21. Update both the non-ambiguous citation and the AMBIGUOUS annotation
to `§6.21`.

**3. `src/parser.rs:2346`:** The `parse_bitand` function doc currently cites `§6.6.1` for
bitwise AND. The current FLS §6.6 is "Underscore Expressions"; bitwise operators fall under
§6.5. Determine the correct current FLS subsection for bitwise AND (within §6.5.x) and update
the annotation. Then add a matching entry to `refs/fls-ambiguities.md` (after the §6.5.5
entry) that documents the `&` disambiguation finding: the FLS defines `&` as both a borrow
operator (§6.5.1) and bitwise AND (§6.5.x), but does not specify how a recursive-descent parser
must disambiguate them; galvanic resolves this positionally (binary `&` only reachable after a
full LHS parse). No minimal reproducer is needed for a purely parsing distinction not observable
in assembly output (pattern: "Not demonstrable via assembly — distinction is at parse level only").

### Why this is the most valuable change right now

Cycle-004 added reproducers to all 45 existing ref entries, making each finding demonstrable.
The next barrier for the Spec Researcher is navigability at the source-to-ref link: searching
source by section number and arriving at the documented finding. Three annotations break this
navigation because they use section numbers from an older FLS version. This is a class-level
fix: correct source citations = unbroken search-and-navigate for every finding, now and for
every new annotation added in future cycles.

The `&` disambiguation (§6.6.1) is also the one genuinely undocumented finding of the four:
the `parse_bitand` annotation describes a real design choice with FLS implications (two uses of
the same token, resolved positionally) but has no ref entry at all. Adding it makes the finding
citable.

### The specific moment

Step 4 of the Spec Researcher journey. Ran `grep -r 'AMBIGUOUS' src/ | grep '§6.3'`. Found
annotation at `src/ast.rs:1127` citing `§6.3.2`. Navigated to `refs/fls-ambiguities.md` — no
§6.3.2 entry. Looked up §6.3 in `refs/fls-pointer.md` — "Path Expressions", not method calls.
Dead end. The finding exists (under §6.12.2), but the stale citation breaks the workflow
completely. The same dead-end plays out for §6.7 and §6.6.1.
