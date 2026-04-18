# Stakeholder Journeys

Concrete first-encounter journeys for each stakeholder. These are what the customer champion walks each cycle. The journeys describe the *steps*, the *moments to watch for*, and the *emotional signal* to track. Current-state observations (what's broken right now, what's been fixed) live in the snapshot — these describe what to try.

---

## Journey 1: The Spec Researcher

**Emotional signal:** Confidence — "I found what I was looking for, and I trust it's complete."

The Spec Researcher arrives from a reference to galvanic as an FLS implementability experiment. They have a specific FLS section in mind — they're checking whether the spec is implementable, or they're looking for evidence of a gap to bring back to spec authors.

### Step 1 — Land and orient
Open the README. Read the two research questions. See that `refs/fls-ambiguities.md` is the primary research output.

*Watch for:* Does the README get them to `refs/fls-ambiguities.md` in under 60 seconds? Or does it require reading the whole file before they know where the findings are?

### Step 2 — Navigate to findings for their section
Open `refs/fls-ambiguities.md`. Try to find everything galvanic says about a specific FLS section (pick one: §6.5 floating-point, §6.15 loops, §4.13 traits).

*Watch for:* Is there a table of contents? Is the file sorted by section number? Can they answer "does galvanic have a finding for §X?" without reading the whole file? The hollowest moment is when the file *claims* to be organized by section but isn't.

### Step 3 — Read an entry
Find an entry for a section they care about. Read: (a) the gap description — what the spec doesn't say, (b) galvanic's chosen resolution, (c) the source annotation location.

*Watch for:* Is the resolution specific ("galvanic uses modulo wrapping, same as LLVM") or vague ("behavior is implementation-defined")? Vague resolutions break confidence — the researcher can't cite what galvanic decided.

### Step 4 — Verify in source (optional)
Open the referenced source file and find the `// FLS §X.Y: AMBIGUOUS — ...` comment. Read it in context.

*Watch for:* Does the source comment add context beyond the registry entry? Or does it just duplicate it? The ideal: the source comment captures the *reasoning* and the registry entry captures the *finding*.

### Step 5 — Take findings
Leave with citable evidence for spec work. The measure: did they find what they needed? Did they trust they hadn't missed anything?

*The worst case:* They found 3 entries for §6.5 but aren't sure if there's a 4th they missed because the file is unsorted and long. Uncertainty about completeness destroys the value of the register.

---

## Journey 2: The Lead Researcher

**Emotional signal:** Momentum — "Each cycle, the compiler handles one more Rust construct and the research output grows."

The Lead Researcher is William, checking in on the project. This is not a first-encounter — it's an ongoing check-in. The journey reflects what a returning maintainer experiences each time they look.

### Step 1 — Check CI
Look at CI status (from snapshot or GitHub Actions). Are all jobs green? jobs: build, fuzz-smoke, audit, e2e, bench.

*Watch for:* Any red job immediately shifts the mood from "checking progress" to "something is broken." A red build on main means cycles have been producing code that doesn't hold up under CI.

### Step 2 — Read the latest cycle
Read the most recent cycle's goal and changelog. What did the builder implement? Did the verifier PASS? Was the goal specific to a real stakeholder moment?

*Watch for:* Goals that are vague ("improve test coverage") versus specific ("at step 2 of the Spec Researcher journey, entry discovery takes too long because entries are unsorted"). Vague goals produce low-value cycles.

### Step 3 — Try the boundary
Write a Rust snippet that exercises the most recently added feature. Compile it through galvanic. Confirm it works. Then write a snippet one step beyond — add a variable, a branch, a function call — and see where the compiler stops.

```
galvanic source.rs
```

*Watch for:* The error message when galvanic can't compile something. Is it useful ("not yet implemented: while loops") or confusing ("lower failed: unknown expression kind")? The Lead Researcher reads these errors as research signals.

### Step 4 — Check the research output
Open `refs/fls-ambiguities.md`. Did the last few cycles add new entries? Are existing entries well-organized enough to scan?

*Watch for:* Cycles that advance the compiler without adding ambiguity findings (missed research opportunity). Cycles that add findings without organizing them (register grows but becomes harder to use).

### Step 5 — Check test quality
Scan `tests/e2e.rs` for recent additions. Are new features tested with assembly inspection (`assert!(asm.contains("add"))`) or only exit-code tests (`assert_eq!(exit_code, 0)`)?

*Watch for:* Exit-code-only tests for arithmetic or control flow are a red flag — they can pass even if galvanic is const-folding at compile time, which violates FLS §6.1.2:37–45. The constraint is what makes the research valid.

---

## Journey 3: The Compiler Contributor

**Emotional signal:** Clarity — "I know exactly where to add this feature, how to test it, and what the FLS says about it."

The Compiler Contributor is someone new to this codebase who wants to implement a Rust language feature. They understand Rust. They may be a student, compiler enthusiast, or researcher.

### Step 1 — Clone and build
```
git clone <repo>
cargo build
cargo test
```

*Watch for:* Any build failure or confusing warning. The first 30 seconds are the trust calibration. A clean build is the minimum bar for "I can contribute here."

### Step 2 — Read the README
Read the two research questions. Understand: galvanic is clean-room (no rustc internals), targets ARM64, implements `no_std` core Rust. Read the "what this is not" section.

*Watch for:* Is the mission clear enough that the contributor knows what kind of PRs are welcome? Does the README point them toward the pipeline?

### Step 3 — Understand the pipeline
Open `src/lib.rs`. See the six modules. Open `src/main.rs`. Read the pipeline: lex → parse → lower → codegen → assemble/link.

*Watch for:* Is the pipeline linear? Is there hidden coupling between stages? The pipeline should be readable top-to-bottom with no surprises.

### Step 4 — Pick a feature and find the FLS section
Decide what to add (e.g., `while` loop). Find the FLS section number (`refs/.lathe/fls-pointer.md` has the TOC). Read that section.

*Watch for:* Does the FLS section help them understand what to implement? Or is it one of the ambiguous sections where the spec doesn't say enough? If ambiguous, do they know to add an entry to `refs/fls-ambiguities.md`?

### Step 5 — Find where to add it
Read the source file for the relevant pipeline stage. Find a similar, already-implemented construct to follow as a pattern.

*Watch for:* Is the FLS annotation convention (`// FLS §X.Y: ...`) consistently applied? Can the contributor find a clear prior example to follow? The hollowest moment is when every source file looks different and there's no pattern to copy.

### Step 6 — Write a test
Decide: parse acceptance (use `tests/fls_fixtures.rs`) or full pipeline (use `tests/e2e.rs`). Write the test first.

*Watch for:* Is the distinction between test tiers clear? A contributor who doesn't know when to use assembly inspection vs. exit-code testing will write a weaker test — or write it in the wrong file.

### Step 7 — Implement and iterate
Write the feature. Follow the FLS citation convention. Run `cargo test`. See what passes.

*Watch for:* Does `cargo clippy -- -D warnings` pass cleanly? Are there confusing error messages from Clippy that weren't documented anywhere?

### Step 8 — Submit
Push and open a PR. CI runs. Does it pass?

*Watch for:* Does CI give actionable feedback when it fails? A failure in the e2e job with no context is harder to debug than a failure with the assembly output printed.
