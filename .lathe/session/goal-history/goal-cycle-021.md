# Goal — Cycle 021

**Stakeholder:** Spec Researcher

**What to change:** Update `refs/fls-constraints.md` Constraint 3 to reflect the current
divide-by-zero implementation, remove the stale "no guard instruction is emitted" claim,
and update the closing paragraph to accurately name what is still missing.

**Why this stakeholder:** Cycles 020 = Compiler Contributor, 019 = Cache-Line Researcher,
018 = Lead Researcher, 017 = Spec Researcher. Spec Researcher was served four cycles ago —
the most under-served stakeholder.

**Why now:** There is a direct factual contradiction between two primary research artifacts:

- `refs/fls-constraints.md`, Constraint 3 body (line 84): "Division-by-zero: **no guard
  instruction is emitted**. ARM64 `udiv` produces zero; `sdiv` behavior is undefined."
- `src/codegen.rs` lines 626, 652, 675, 690, 813, 818: `cbz x{rhs}, _galvanic_panic`
  guards ARE emitted before every `sdiv`, `srem`, and `udiv` instruction.
- `refs/fls-ambiguities.md`, §6.9/§6.23: "a `cbz xRHS, _galvanic_panic` guard is emitted
  before every `sdiv`, `srem`, and `udiv` instruction. (Claim 4o)"

Claims 4m, 4o, 4p, 4q implemented divide-by-zero guards, MIN/-1 guards, and bounds checks.
The constraints file was written when none of this existed and was never updated. The
ambiguities file was updated. The two documents now disagree.

**The specific moment:** Step 5 of the Spec Researcher's journey — "Open
`refs/fls-constraints.md`. Try to understand one constraint end-to-end." I read Constraint
3, which covers the panic-triggering cases: divide-by-zero, overflow. The body says
"Division-by-zero: no guard instruction is emitted." I made a note. Then I cross-referenced
with `refs/fls-ambiguities.md` §6.9/§6.23 for more context. The entry there says guards ARE
emitted. I went to the source: guards ARE there. The constraints file is factually wrong on
a concrete, citable claim. The hollowest moment: I had written "divide-by-zero: unguarded"
in my notes and almost took it to a spec discussion. The two research documents are actively
misleading a reader who reads both.

**What the constraints file needs to say instead:**

- Divide-by-zero guard (non-literal divisors): **Satisfied.** `cbz x{rhs}, _galvanic_panic`
  emitted before every `sdiv`, `srem`, and `udiv`. (Claim 4o)
- Literal zero divisors: **Satisfied.** Rejected at compile time in `lower.rs`. (Claim 4m)
- `i32::MIN / -1` overflow guard: **Satisfied.** `movz`/`sxtw`/`cmp`/`cmn` guard emitted
  before `sdiv`. (Claim 4q)
- Debug-mode arithmetic overflow (+, -, *): **Not yet implemented.** This is the remaining
  genuine gap — galvanic uses 64-bit arithmetic throughout and does not insert overflow
  checks for these operators. Known deviation from debug-mode Rust semantics.
- "No panic infrastructure exists at this milestone" must be removed — it is wrong.
  `_galvanic_panic` exists and handles three categories of runtime errors.

The closing paragraph must be updated: "The remaining gap is arithmetic overflow detection
for `+`, `-`, `*` in debug mode. Divide-by-zero, MIN/-1 overflow, and bounds checking are
all implemented."

**Secondary: Constraint 8 is not an FLS constraint.** Its source is explicitly "Project
design principle (not FLS)." It appears in the summary table alongside FLS constraints
labeled "Not satisfied," misleading a Spec Researcher into thinking galvanic fails an FLS
requirement. It should be removed from this document or moved to a clearly separated section
labeled "Internal design goals (not FLS)." The builder should determine the right
disposition, but it must not appear in the FLS compliance summary table without clear
separation.

**The class of fix:** Research documents that describe implementation status must be updated
when the implementation advances. A constraints file that says "not implemented" after the
implementation has been done is worse than no constraints file — it is actively wrong.
The fix makes the constraint status a reliable single source of truth, not a document the
reader must cross-check against source code to verify.

**Lived experience note:** I became the Spec Researcher. I opened `refs/fls-constraints.md`
to understand galvanic's FLS compliance posture — specifically what the spec requires and
whether galvanic delivers. I read Constraint 3 carefully: overflow semantics, divide-by-zero,
MIN/-1. The body said "no guard instruction is emitted." I took a note. Then I went to
fls-ambiguities.md for the §6.9/§6.23 entry (the pointer in Constraint 3 led me there).
The ambiguities entry said guards ARE emitted, with specific instruction sequences. I stared
at both documents. I checked the source. The source confirms guards. The constraints file is
wrong. I cannot cite either document without qualifying it; they say opposite things. The
constraints file — the one explicitly designed to be a compliance reference — is less
accurate than the ambiguities file. That's the emotional breaking point: the authoritative
compliance document gives me wrong information that I almost used.
