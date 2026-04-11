# Alignment Summary

This file is for William (the user), not the runtime agent. It records the decisions made during goal-setter setup and the uncertainties worth checking.

---

## Who this serves

**William Laffin** — the primary researcher. Reads the git log. Evaluates whether cycles advance the two research questions: (1) is the FLS implementable? (2) what does cache-line-aware codegen look like from the start? His relationship with the project is through commit history and the research findings that surface.

**FLS spec readers / Ferrocene team** — people who want to know where the spec fails to pin down behavior. They see the project through `// FLS §X.Y: AMBIGUOUS —` annotations. The research output they care about is currently scattered — no single document collects the findings.

**Future contributors** — the person who finds this on GitHub on a Sunday. They can read the commits and infer the "claims" methodology, but nothing explicitly documents the contribution pattern. The README is 4 short paragraphs.

**Lathe itself** — reads goal.md + snapshot each cycle. The goal-setter is now set up to rank work by stakeholder impact rather than a fixed feature ladder.

---

## Key tensions

**Parse acceptance vs. full pipeline.** ~40 parse fixtures exist for closures, generics, traits, etc. Adding more parse fixtures is not research progress — it's scaffolding. Real progress is taking a parse fixture all the way to e2e codegen with assembly inspection.

**FLS breadth vs. correctness depth.** Several implemented features may have exit-code e2e tests but no assembly inspection tests. A test that checks only the exit code cannot distinguish "compiled correctly" from "constant-folded at compile time and emitted the result." Assembly inspection closes the gap. Until that gap is closed for all existing features, new claims may be premature.

**Ambiguity documentation vs. forward progress.** The project's primary research output — where the FLS is silent, ambiguous, or contradictory — lives as scattered inline comments with no surface document collecting them. One cycle spent aggregating those findings would make the research more visible.

---

## What could be wrong

**The assembly inspection coverage may be incomplete.** I looked at the e2e test structure but didn't read every test. It's possible that some milestone features (e.g., while-let, match, struct-expressions, which appear in recent commits) have only exit-code tests and no `compile_to_asm()` inspection. If so, the core constraint could be silently violated. Recommend checking: `grep -n "compile_to_asm" tests/e2e.rs` to see which features have assembly inspection.

**Closures (§6.14) are the hard case.** They require a capture environment — a fundamentally different memory model than anything currently implemented. The goal-setter ranks next claims by proximity to existing infrastructure. If a cycle picks closures and the IR doesn't support capture slots, the agent will hit a wall. The goal.md notes that closures are unimplemented at codegen level, but I haven't verified whether the IR has any scaffolding for them.

**No branch protection check performed.** I couldn't directly query GitHub's branch protection settings from here. The CI uses `pull_request` (not `pull_request_target`) — that's the safe pattern. The `permissions: contents: read` on the main CI job is correct. Recommend verifying via GitHub settings that the default branch (`main`) is protected and requires CI to pass before merge.

**The repo is public** (github.com/libliflin/galvanic). The snapshot.sh reads `git log` and `git status` — not PR titles or issue comments — so prompt injection from external PR metadata is not a current risk. If the engine is ever configured to feed PR descriptions into the agent prompt, that changes and should be audited.

**No `skills/fls-constraints.md`** exists — the constraints doc is in `refs/fls-constraints.md`. That's fine, but the testing.md I wrote refers to `refs/fls-constraints.md` correctly. The goal.md also references it correctly.

**The "claims" numbering** (4a, 4b, ..., 4k) appears to be within a milestone 4 series. I don't know what claims 1–3 covered or what determines a milestone boundary. The goal-setter doesn't need this, but a future contributor would benefit from the methodology being documented somewhere explicit.
