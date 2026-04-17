# Goal: Add a navigable table of contents to `refs/fls-ambiguities.md`

## What to change

Add a sorted, linked table of contents at the top of `refs/fls-ambiguities.md`, immediately after the introductory paragraph. The TOC should:

1. List every documented FLS section in numeric section order (§2 before §4, §6.5.3 before §6.5.9, etc.)
2. Include a one-line summary of the gap for each entry (what the spec doesn't say, in ~10 words)
3. Use Markdown anchor links so each entry links directly to its section heading in the file

Additionally, reorder the body entries into the same FLS section order. Currently, entries added in later cycles were appended at the bottom out of order (§4.14 appears after §12.1; §6.10, §6.12.2, §6.13, §6.14, §6.15.6, §9.2, §13, §14, §19 all appear after §15), so the file would have a TOC pointing to one order while the content appears in another. A single consistent sort fixes both problems.

## Which stakeholder this helps and why

This serves the **Spec Researcher** — a person studying the FLS who arrived at galvanic because it documents where the spec is silent or ambiguous and they want concrete, citable findings to take back to spec authors.

Their journey step 2 is: open `refs/fls-ambiguities.md` and scan for sections they care about. That step is currently broken. There is no table of contents. The file is 807 lines across 47 entries, and the entries are not in section order. A spec researcher wanting all float-related findings (§6.5.3, §6.5.5) must scroll 800 lines or use editor search. A researcher preparing for a spec meeting on §6.15 (loops) has no way to scan which loop-related findings exist without reading the whole file.

This is a class-level fix: a single well-structured, sorted TOC eliminates the entire "I know a finding exists but I can't find it" category for all 47 current entries and every future one.

## Why now

The ambiguity registry has grown to 47 entries covering 30+ FLS sections — it is now large enough that scanning without a TOC is genuinely painful, but small enough that building the TOC and sorting the entries is a single, clean, bounded task.

The last ~15 cycles have exclusively served the Lead Researcher (Claims 4m–4s, Constraint 8, e2e test additions). The Spec Researcher's primary artifact — `refs/fls-ambiguities.md` — has grown in volume without growing in navigability. Adding entries without adding structure erodes the value of the registry: 47 findings scattered across 807 unsorted lines is harder to use than 20 well-indexed ones.

## Lived-experience note

**Stakeholder:** The Spec Researcher.

**What I tried:** I walked step 2 of the Spec Researcher journey. I opened `refs/fls-ambiguities.md` wanting to find everything galvanic says about floating-point semantics. The file opens with "§2.4.4.1 — Integer Literals" — a lexer-level entry — and proceeds roughly in section order through the §4 and §5 entries. By line 200 I'm in §6. I find §6.5.3 at line 213 and §6.5.5 at line 228 — good. Then I try to verify I haven't missed anything in §6.5: I have to manually scan every subsequent entry because later additions (§6.10 at line 604, §6.13 at line 620, §6.15.6 at line 665) were appended out of order.

**The worst moment:** Trying to answer "does galvanic have a finding for §6.15 (loop expressions)?" There is no §6.15 entry at the position I'd expect it. There IS a §6.15.1 (For Loop) entry at line 330 and a §6.15.6 (Break-with-Value) entry at line 665 — 335 lines apart, out of order relative to each other. A spec researcher preparing a talk on loop semantics would miss the second entry entirely unless they read the whole file.

**The hollowest moment:** The introductory paragraph says this file aggregates AMBIGUOUS annotations "by FLS section." The entries are not by section. The promise of the document contradicts its structure. That gap between promise and reality is the thing to fix.
