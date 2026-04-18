# Goal — Cycle 029

**Stakeholder:** Spec Researcher

**What to change:** Remove the opaque "Claim 4m / 4n / 4o / 4p / 4q" labels from the
public-facing research artifacts in `refs/fls-ambiguities.md` and `refs/fls-constraints.md`.
These labels appear in three entries and the constraints summary but reference an internal
verification tracking system that has no public definition. While the actual content (each
description of galvanic's choice) is inline and correct, the claim labels interrupt reading
and make entries look like they reference a companion document that doesn't exist.

Specifically:

1. In `refs/fls-ambiguities.md`, **§4.9 — Bounds Checking Mechanism** (lines ~242, ~247):
   - "Claims 4m/4p" in the body → remove or replace with a concrete description
   - "Prior to Claims 4m/4p, no bounds check was emitted." → rewrite as
     "Prior to implementing bounds checking, no check was emitted."

2. In `refs/fls-ambiguities.md`, **§6.9 / §6.23 — Panic Mechanism** (lines ~629–641):
   - "**Galvanic's choice (updated — Claims 4m, 4o, 4p, 4q):**" → "**Galvanic's choice:**"
   - Remove "(Claim 4m)", "(Claim 4o)", "(Claim 4q)", "(Claim 4p)" from each bullet
   - The descriptions themselves are accurate and complete; only the labels go

3. In `refs/fls-ambiguities.md`, **§6.21 — Comparison Non-Associativity** (line ~949):
   - "**Galvanic's choice (Claim 4n):**" → "**Galvanic's choice:**"

4. In `refs/fls-constraints.md`, **Constraint 3** (line ~88):
   - "Claims 4m, 4o, 4q" in the body → remove the parenthetical or replace with
     a description that names the behavior (literal-zero check, cbz guard, MIN/-1 guard)

5. Update the footer of `refs/fls-ambiguities.md` (last line):
   - "46 entries" → "48 entries" (the count is stale; there are 48 body entries)

**Why this stakeholder:** Cycles 028 = Compiler Contributor, 027 = Cache-Line, 026 = Lead,
025 = Spec Researcher. All four have been served. The rotation returns to Spec Researcher.
The journey immediately surfaced a concrete, citable friction.

**Why now:** At step 3 of the Spec Researcher's journey — "Read one finding end-to-end.
Does it clearly state what the FLS says, what it leaves unspecified, and what galvanic
chose?" — I navigated to §6.9/§6.23 (Panic Mechanism). The entry header read:

```
**Galvanic's choice (updated — Claims 4m, 4o, 4p, 4q):**
```

I paused. "Updated from what?" "What are Claims 4m, 4o, 4p, 4q?" I searched the document
for a claims index: none. I searched the repo: no claims registry, no companion document.
The only definition is scattered across the lathe session history (cycles 007–021) — internal
CI tracking that was never translated into a public explanation.

Each bullet then ends with "(Claim 4m)", "(Claim 4o)", etc. The actual descriptions are
inline and correct — I can read galvanic's choice without knowing what the Claims are. But
the labels signal: "there is a reference document you haven't found." I can't cite this
entry in a spec meeting without explaining that "Claim 4m" is internal notation. That
reduces citeability.

The same labels appear in §4.9 ("Claims 4m/4p"), §6.21 ("Claim 4n"), and in
`fls-constraints.md`. Three entries and one constraints summary affected.

When I finished reading and looked at the footer to confirm how many entries to expect:
"46 entries." I had counted 48 in the TOC. The stale count raised a second doubt: is the
file being maintained? Are there 2 entries somewhere that aren't in the TOC?

**The class of fix:** Internal tracking identifiers from the verification system should not
appear in public research artifacts without definition. A research document is self-contained
when every reference it makes is either explained in that document or points to an accessible
external source. The Claims labels are neither — they reference an internal state machine
that no Spec Researcher can access.

Removing them makes wrong states unrepresentable: no future entry can accidentally carry an
internal tracking label because the convention is visibly absent from the document.

The stale footer is a related signal: the document's metadata (count, date) should be
accurate so that a reader can trust that the document reflects the current state of the
compiler.

**What success looks like:**

A Spec Researcher reads §6.9/§6.23 from top to bottom and can cite galvanic's panic
mechanism choices — literal zero rejection, runtime cbz guard, MIN/-1 guard, bounds check —
without encountering any unexplained reference. The §6.21 and §4.9 entries read the same
way. The footer says "48 entries" which matches the actual count.

**Constraint:** No content changes — every description of galvanic's choice is accurate and
should be preserved. Only the claim labels and stale count are removed or updated. Do not
add any new content to these entries; the fix is subtraction and correction, not addition.

**Lived experience note:** I became the Spec Researcher. I opened `refs/fls-ambiguities.md`
without having written it. I read the README — one sentence, two research questions — the
right voice. I found the TOC immediately, 48 entries, in section order. I set a two-minute
clock and navigated to §6.9/§6.23 as a topic I cared about. I read the Gap — clear. Then
the header of Galvanic's choice: "updated — Claims 4m, 4o, 4p, 4q." I stopped.

I spent 45 seconds looking for what "Claims" meant. I searched the document. I searched
the repo. Nothing. I read the bullets — each ends with a label like "(Claim 4m)" that
means nothing to me. The actual descriptions are there, complete and correct. But I cannot
write "galvanic satisfies this per Claims 4m, 4o, 4p, 4q" in a spec discussion — I have
to strip the labels and describe the behavior in my own words, which is what the document
should have done in the first place.

The hollowest moment: finding that the document I came to as a trusted research artifact
contains a reference system — "Claims" — that is only interpretable by someone who read
the lathe CI session logs from a year ago. The information is there; the labels are noise
that signals "you're missing something" when you're not.
