# Alignment Summary — Galvanic Customer Champion

This file is for the human. It summarizes the alignment decisions made during init and flags what could be wrong.

---

## Who this serves

**Lead researcher (the author):** Extending the compiler feature by feature, tracking FLS fidelity, documenting cache-line tradeoffs. Uses galvanic daily as a research instrument.

**Spec researcher:** Studying the Ferrocene Language Specification — looking for concrete ambiguities and gaps, using galvanic's findings as evidence. Arrives at `refs/fls-ambiguities.md` and traces findings from source annotations to documentation.

**Compiler contributor:** A Rust programmer adding a new language feature to galvanic as a learning exercise or contribution. Needs the architecture to be discoverable and the patterns to be clear.

---

## Emotional signal per stakeholder

| Stakeholder          | Signal    | Tracks                                                    |
|----------------------|-----------|-----------------------------------------------------------|
| Lead researcher      | Momentum  | Is the compiler getting smarter? Are findings being captured? |
| Spec researcher      | Discovery | Are FLS gaps specific, grounded, and documented?           |
| Compiler contributor | Clarity   | Is the pipeline discoverable? Can a new contributor follow the pattern? |

---

## Key tensions

**Spec fidelity vs. feature breadth.** Tight FLS compliance (precise citations, constraint tracking, ambiguity documentation) slows feature development. Signal: check whether `refs/fls-ambiguities.md` and `refs/fls-constraints.md` are growing alongside the feature set.

**Cache-line rigor vs. implementation speed.** Every IR node needs a cache-line note. This is the research question, not optional polish. Signal: scan recent IR additions — do they have `Cache-line note` comments?

**Contributor accessibility vs. research depth.** FLS traceability and cache-line analysis add cognitive load for contributors. Signal: at which step in the contributor journey does a new contributor stall?

---

## Repository security assessment (for autonomous operation)

**Prompt injection risk:** Lathe reads CI status and PR metadata from GitHub and feeds it into agent prompts. This is a potential prompt injection vector.

- **Workflow triggers:** `ci.yml` is triggered by `push` (to `main`) and `pull_request` (to `main`). It does **not** use `pull_request_target` or `issue_comment`, which are the high-risk triggers that run with elevated permissions on untrusted code. Risk: low.
- **Workflow permissions:** Global `permissions: contents: read` is set. No write permissions granted. Risk: low.
- **Repo visibility:** Could not verify automatically (GitHub CLI not authorized during init). The README and Cargo.toml mention `libliflin/galvanic`, which appears to be a public repo. **Action needed:** Confirm whether the repo is public or private, and whether the default branch (`main`) has branch protection rules enabled. Without branch protection, any lathe PR could be merged without review.

**Recommendation:** Enable branch protection on `main` requiring CI to pass before merge. This is the primary safeguard for autonomous operation.

---

## What could be wrong

**Stakeholder coverage:** The "spec researcher" stakeholder is partially inferred. The README doesn't explicitly name external spec researchers as a target audience — it says "nobody needs to use this." If the actual audience is narrower (just the author), the spec researcher journey may produce misaligned goals. Watch for goals that optimize for spec researcher clarity when the author already knows where the findings live.

**The contributor stakeholder may be premature.** The README says this is not a production compiler and nobody needs to use it. If there are no external contributors and no intention to have any, optimizing for contributor clarity could distract from the research goals. If the author is the sole contributor, the lead researcher and contributor journeys are the same person — the champion should weight researcher momentum over contributor onboarding in that case.

**Cache-line rigor as a first-class invariant:** The goal.md treats cache-line notes as a research artifact the champion should watch for. If the project phase has shifted (e.g., the cache-line research question has been answered), this invariant may no longer be the right thing to guard. The champion should check: are cache-line notes still being written with genuine analysis, or has it become boilerplate?

**FLS version drift:** `refs/fls-pointer.md` notes the table of contents was verified as of 2026-04-05. The FLS is versioned and may have been updated since. Section numbers in source citations could be stale. The champion should periodically spot-check citations against the current spec.

**No `pull_request_target` risk confirmed but not verified:** The security assessment above is based on reading `ci.yml`. If additional workflow files are added in future (e.g., `release.yml` or `labeler.yml`), they should be reviewed for `pull_request_target` or `issue_comment` triggers before lathe is run autonomously.
