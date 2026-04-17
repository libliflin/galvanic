# Verification — Cycle 1, Round 1

## What was checked

- Read the builder's diff in full: TOC addition, introductory text update, 12 out-of-order entries relocated to their correct FLS-section positions, footer datestamp updated.
- Verified that all 45 TOC entries have a corresponding `## §` body heading.
- Verified that body headings are in FLS section numeric order.
- Verified no duplicate headings remain.
- Ran the full test suite: 2047 tests, 0 failures.

## Findings

**Duplicate entry introduced (fixed):** The builder moved the old `§6.21 / §6.7 — Comparison Operator Non-Associativity` entry from the unsorted tail into sorted position — but the file already contained a newer, more detailed `§6.21 — Comparison Non-Associativity: Chained Comparisons` entry at that position (from Claim 4n). The result was two §6.21 body sections with contradictory content:

- The newer entry (Claim 4n): enforcement at the lowering stage — the correct, current behavior.
- The older entry (§6.21 / §6.7): describes enforcement as deferred, left-associative at grammar — a superseded approach.

Both entries appeared in the TOC. A spec researcher following the TOC would read both and encounter contradictory descriptions of galvanic's behavior.

No other issues found. The sort order, TOC anchor links, and entry completeness are correct for all remaining 45 entries.

## Fixes applied

Removed the stale `§6.21 / §6.7 — Comparison Operator Non-Associativity` duplicate from both the TOC and the body. The authoritative `§6.21 — Comparison Non-Associativity: Chained Comparisons` entry (Claim 4n) is the only §6.21 entry remaining.

**File modified:** `refs/fls-ambiguities.md` — entry count: 46 → 45.

## Witnessed

- `cargo test`: 2047 passed, 0 failed (lib + e2e + fls_fixtures + smoke).
- `grep "^## §" refs/fls-ambiguities.md | wc -l` → 45; `grep "^- \[§" refs/fls-ambiguities.md | wc -l` → 45. TOC and body counts match.
- `grep "^## §" refs/fls-ambiguities.md | sort | uniq -d` → empty. No duplicates.
- Confirmed §6.15.1 and §6.15.6 are now adjacent, resolving the "335 lines apart" navigation problem named in the goal.

## Confidence

High. The goal is met: the file has a sorted, linked TOC and a body sorted by FLS section number. The one genuine gap introduced by the builder (duplicate §6.21) is fixed. No regressions, no missing entries, no anchor link mismatches.

VERDICT: PASS

---

# Changelog — Customer Champion Cycle

## Stakeholder: The Spec Researcher

**Became:** The Spec Researcher — an FLS contributor or compiler educator who arrived at galvanic to find concrete, citable findings about where the spec is silent or ambiguous.

**Rotation rationale:** The last ~15 cycles have served the Lead Researcher exclusively (Claims 4m–4s, Constraint 8, e2e test additions). The Spec Researcher is the most under-served stakeholder. Their primary artifact — `refs/fls-ambiguities.md` — has grown without becoming more navigable.

---

## What I experienced

Walked step 2 of the Spec Researcher journey: opened `refs/fls-ambiguities.md` to scan for sections of interest.

The file is 807 lines with 47 entries and no table of contents. The entries are not in FLS section order — entries added in later cycles were appended at the bottom (§4.14 after §12.1; §6.10, §6.12.2, §6.13, §6.14, §6.15.6, §9.2, §13, §14, §19 all after §15). The introductory paragraph says the file organizes findings "by FLS section" — but the body does not reflect that.

**The worst moment:** Trying to find all loop-related findings (§6.15). There are two entries: §6.15.1 at line 330 and §6.15.6 at line 665 — 335 lines apart, with no connection between them. A spec researcher would find the first and miss the second.

**The hollowest moment:** The promise of the document ("aggregates AMBIGUOUS annotations by FLS section") is not matched by its structure. 47 documented findings with real research value, presented as an unsorted 807-line wall of text.

---

## Goal set

**Add a sorted, linked table of contents to `refs/fls-ambiguities.md` and reorder the body entries into FLS section order.**

The TOC: every section listed in numeric order, one-line gap summary per entry, Markdown anchor links.

The reorder: move the out-of-order entries (§4.14, §6.10, §6.12.2, §6.13, §6.14, §6.15.6, §6.17, §9.2, §10.2, §13, §14, §19) to their correct positions in the body.

**Why this is the most valuable change right now:** It's a class-level fix. A TOC + sort eliminates the entire "finding exists but can't be found" category for all 47 current entries and all future ones. The registry has reached the size where scanning without a TOC is genuinely costly; the task is bounded and self-contained.

---

## Floor check

Build: OK. Tests: 2047 pass, 0 fail. Clippy: OK. Unsafe audit: OK. Floor intact.

---

## Builder Changelog — Round 1

### Applied
- Added a 46-entry linked TOC immediately after the intro paragraph, with one
  line per finding and Markdown anchor links to each body section.
- Reordered 12 out-of-position entries into correct FLS section number order:
  §4.14, §6.10, §6.12.2, §6.13, §6.14, §6.15.6, §6.17, §9.2, §10.2,
  §13, §14, §19 — all moved from their appended positions to their correct
  numeric slots.
- Updated the footer to 2026-04-17 and noted the file is now sorted with a
  linked TOC.

**Files:** `refs/fls-ambiguities.md`

### Validated
- `cargo test`: 2047 passed, 0 failed — doc-only change, no code touched.
- `grep -n '§6.15' refs/fls-ambiguities.md` now shows §6.15.1 and §6.15.6
  within a few lines of each other (previously 335 lines apart).
- Verifier: open `refs/fls-ambiguities.md`, click any TOC entry to confirm
  anchor navigation works.

---

# Changelog — Customer Champion Cycle 002

## Stakeholder: The Compiler Contributor

**Became:** A Compiler Contributor — a CS student or Rust enthusiast who picked galvanic to learn compiler internals from the FLS, step by step.

**Rotation rationale:** Cycle 001 served the Spec Researcher. The 15 cycles before that served the Lead Researcher. The Compiler Contributor has not been served in recent memory.

---

## What I experienced

Walked step 7 of the Compiler Contributor journey: ran `cargo run --` on each of the 5 parse-only fixtures. Every one failed with a "not yet supported" error that named the symptom but gave no source location.

For `fls_9_functions.rs` (19 items, 200+ lines):
```
error: lower failed (not yet supported: integer literal with non-integer type)
```

To find *which* of the 19 items triggered this, the only options were: comment out functions one by one, or add `eprintln!` calls to `src/lower.rs`. That's archaeology, not contribution.

The AST has `Span` on every node. The lowering pass knows which item it's processing. That information is discarded before the error is returned to `main.rs`.

**The worst moment:** Found the error at `src/lower.rs:10504` via grep, confirmed it's in the `LitInt` matching arm — and still had no idea which function in the 200-line file triggered it.

**The hollowest moment:** The message is correct and technically precise. It just gives the contributor zero foothold.

---

## Goal set

**When lowering fails with "not yet supported", include the name of the item being lowered in the error output.**

Before: `error: lower failed (not yet supported: integer literal with non-integer type)`  
After: `error: lower failed in 'const_add': not yet supported: integer literal with non-integer type`

The item name is already present in the AST at the point where per-item lowering happens. Thread it into the error. This is a class-level fix: every future "not yet supported" error across all 5 parse-only fixtures (and future ones) will carry context automatically.

---

## Floor check

Build: OK. Tests: 2047 pass, 0 fail. Clippy: OK. Unsafe audit: OK. Floor intact.
