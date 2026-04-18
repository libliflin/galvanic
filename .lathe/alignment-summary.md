# Alignment Summary — Galvanic Customer Champion Init

For the project maintainer. Plain-English summary of the decisions made during init.

---

## Who This Serves

**Spec Researcher** — A person studying or contributing to the Ferrocene Language Specification. They use galvanic to find documented evidence of where the spec is silent or ambiguous. Their primary artifact is `refs/fls-ambiguities.md`. They may be a spec author, language committee member, or academic researcher.

**Lead Researcher (William)** — The project's author and runner. Asking two research questions: (1) Is the FLS implementable by an independent party? (2) What does "cache-line-aware from the start" actually produce? Checks in periodically, watches the compiler boundary advance, reviews cycle changelogs for substance.

**Compiler Contributor** — Someone who wants to extend galvanic to handle more Rust. Needs to understand the pipeline, find the right place to add a feature, follow FLS citation conventions, and write a test in the right tier.

---

## Emotional Signal Per Stakeholder

**Spec Researcher → Confidence.** "I found what I was looking for and I trust it's complete." The anti-signal is doubt about completeness — "did I miss an entry for this section?"

**Lead Researcher → Momentum.** "Each cycle the compiler handles one more Rust construct and the research output grows." The anti-signal is stagnation — cycles that polish without advancing the boundary or the register.

**Compiler Contributor → Clarity.** "I know exactly where to add this feature, how to test it, and what the FLS says about it." The anti-signal is confusion — unclear pipeline, no obvious test pattern to follow.

---

## Key Tensions

**Research completeness vs. implementation momentum.** Organizing `fls-ambiguities.md` serves the Spec Researcher; implementing new language features serves the Lead Researcher. Both are real value. The signal for prioritization: check which has gone 4+ cycles without attention, and whether the register's size has outgrown its navigability.

**Coverage vs. correctness.** Adding language feature support is valuable, but the FLS constraint on const-folding (§6.1.2:37–45) is absolute. Exit-code-only tests can mask const-folding. Assembly inspection tests are the enforcement mechanism. Coverage without assembly inspection tests is suspect.

**Navigability vs. raw completeness in the ambiguity register.** A long, unsorted file with more entries is harder to use than a shorter, organized one. The tipping point is around 20+ entries — after that, structure matters as much as content.

---

## Repository Security (for autonomous operation)

Checked during init:

- **CI triggers:** `.github/workflows/ci.yml` uses `push: branches: [main]` and `pull_request: branches: [main]`. Neither `pull_request_target` nor `issue_comment` is used. **Low prompt-injection risk** — lathe cannot be triggered by external actors through issue comments or fork PRs.
- **CI permissions:** `permissions: contents: read` — minimal. No write permissions granted to workflows.
- **Repo visibility:** The README references `libliflin/galvanic` — this appears to be a public repo. Lathe will feed CI metadata and PR content into agent prompts. PR descriptions from external contributors (if any) are an injection surface, though low-risk given the project's research scope.
- **Default branch protection:** Not verified during init (requires GitHub API access). **Recommended action:** Confirm main is protected (require PR + CI pass before merge) to prevent direct pushes from the lathe builder.

---

## What Could Be Wrong

**Missing stakeholder — the Spec Author.** I identified "Spec Researcher" as a consumer of galvanic's findings, but the actual Ferrocene spec team (the people who *write* the FLS) might be a distinct stakeholder with different needs. A spec author wants to know "which section produced the most ambiguities" or "what wording change would close gap X" — a more actionable frame than a researcher reading findings. If the project gets attention from Ferrocene contributors, this stakeholder should be added.

**Lead Researcher and Compiler Contributor may collapse into one.** In this project, William is likely both. If they're the same person, the tension between "advance the compiler" (Contributor) and "run the experiment" (Researcher) may be lower than documented. The champion should notice when cycling between these two produces no real rotation and consolidate them if needed.

**Ambiguity register may already be the dominant priority.** The most recent goal (the one in the project root at init time) was entirely about the Spec Researcher — adding a TOC and sorting `fls-ambiguities.md`. The 15 prior cycles heavily favored the Lead Researcher. This suggests the register-navigability tension is already live and the champion should be aware of where in that cycle they are.

**No brand.md.** Galvanic is a research project with a distinctive voice (the README's "sacrificial anode" framing, the two-question mission). That voice should inform which fixes feel right. Brand.md doesn't exist yet, so the champion falls back to stakeholder emotional signal. When the project has more cycles of output to read, a brand.md derived from the actual commit history would strengthen the champion's direction-choosing.

**E2E test dependencies on macOS.** The e2e tests require `aarch64-linux-gnu-as` and `qemu-aarch64`. These skip gracefully locally but always run on CI. A Compiler Contributor on macOS may not see e2e failures until after pushing. This is a Contributor journey friction point worth watching.
