# Brand

**Identity.** A research instrument that speaks like the researcher using it. Precise about scope, honest about limits, obsessive about traceability. Every claim is anchored — to an FLS section, a count, a concrete resolution. When something partially works, galvanic says so and shows you what it got. When the spec is silent, galvanic names the silence and documents the choice it made.

The project describes itself as "a sacrificial anode — it exists to find ambiguities in the spec and to explore what 'dumb but cache-aware' codegen can do. Nobody needs to use this. Value comes from what we learn." (README.md:14–15) That sentence — blunt, research-framed, dismissive of production pretension — is the voice.

---

## How we speak

**When we say no:** We name what the spec doesn't say, document what we chose anyway, and move on. Not "unsupported" — "not yet supported," with the implicit acknowledgment that the scope is deliberate and bounded. The `AMBIGUOUS` annotation format (`FLS §X.Y AMBIGUOUS: ...`) appears dozens of times across the codebase, always with the gap described and galvanic's resolution recorded. We say no to whole categories of production-compiler behavior in one line: "Do not use this to compile anything you care about." (README.md:17)

**When we fail:** We report everything in a single run, not just the first error. (main.rs:88–89: "Print every per-function error so the researcher sees the full error landscape in a single run (not just the first failure).") A partial success is visible and named: `galvanic: emitted fixture.s (partial — some functions failed)`. (main.rs:148) The exit code is non-zero even when partial assembly is emitted — we never let a partial success look like a clean one.

**When we explain:** We cite. Every module opens with a traceability block mapping its own functions to FLS sections. Comments in the code explain the *why* of a design decision — "The litmus test: if replacing a literal with a function parameter would break the implementation, it's an interpreter, not a compiler." (lower.rs:14) Cache-line reasoning is documented inline wherever a data structure's layout is load-bearing.

**When we onboard a new user:** Minimal. `usage: galvanic <source.rs> [-o <output>]`. No cheerful intro, no feature list. The README opens with a one-sentence project description, follows with two numbered research questions, and ends with `cargo build` / `cargo test`. The assumption is that you arrived because the questions interest you, not because you need a compiler.

**When we succeed:** We name the artifact and stop. `galvanic: emitted fls_10_2_assoc_types.s`. (main.rs:150) No congratulations, no banner. If the assembly is there, the fact of its existence is the celebration.

---

## The thing we'd never do

We'd never let a partial success be silent. The project goes out of its way — structurally, not just stylistically — to ensure that partial lowering produces both a non-zero exit code *and* a visible artifact with an explicit `(partial — some functions failed)` annotation. The implementation of `had_lower_errors` threading through main.rs (lines 87–155) exists entirely to make this guarantee structural. We'd never paper over a partial compile with a clean exit code; that's the silent wrong answer that makes the researcher leave (goal.md:30).

---

## Signals to preserve

- **The count.** When something partially fails, galvanic reports `lowered N of M functions (K failed)`. This pattern — exact counts, not vague success/failure — should carry forward to any new pipeline stage that can partially succeed.
- **The `FLS §X.Y AMBIGUOUS:` format.** The annotation is a research artifact. It names the spec section, describes the gap, and states galvanic's resolution. These three parts are load-bearing; collapsing any of them ("see the code" instead of a resolution) breaks the Spec Researcher's journey.
- **"Not yet supported" over "unsupported."** The phrasing is honest about the project's developmental state without being dismissive. Keep `not yet` — it's the right hedge for a research compiler in motion.
