# Alignment Summary

Plain-English summary of the customer champion setup for human review. This file is for you, not the runtime agent.

---

## Who this serves

- **Lead Researcher** — the primary driver of the project. Runs galvanic against FLS fixture programs, studies compiler output, discovers ambiguities, advances FLS coverage. This is likely the project owner.
- **Spec Researcher** — external. Opens `refs/fls-ambiguities.md` and `refs/fls-constraints.md` looking for citable findings to take to spec discussions. Does not run the compiler.
- **Compiler Contributor** — wants to implement a new FLS section inside galvanic. Needs to understand the pipeline architecture and find the right place to add code.
- **Cache-Line Performance Researcher** — studies whether cache-aware codegen produces measurable differences. Uses benchmarks and inspects emitted assembly.

---

## Emotional signal per stakeholder

- **Lead Researcher** — Momentum. Each run tells them something new and the output is always trustworthy. The worst feeling is ambiguity: an error that doesn't name what failed, or an `.s` file that might be wrong.
- **Spec Researcher** — Curiosity satisfied. "I found a concrete, citable finding in under two minutes." The worst feeling is a finding that raises more questions than it answers.
- **Compiler Contributor** — Confidence. "I know exactly where to make this change." The worst feeling is architectural opacity — not knowing whether a new feature belongs in the IR, the lowering pass, or the codegen.
- **Cache-Line Researcher** — Verifiable. "The claim is documented, tested, and visible in the output." The worst feeling is a cache-line argument with no corresponding test.

---

## Key tensions

**Breadth vs. depth.** Covering more FLS sections (breadth) serves the Lead Researcher's research goals. Making each covered section's findings thorough and citable (depth) serves the Spec Researcher. Signal: if `refs/fls-ambiguities.md` entries say "see the code" without stating galvanic's resolution, depth wins. If whole FLS chapters have no entries, breadth wins.

**Research artifact quality vs. implementation velocity.** Making the research artifacts (fls-ambiguities.md, fls-constraints.md) more navigable is time not spent extending the compiler. Signal: if multiple consecutive cycles have been implementation-only without touching research artifacts, the Spec Researcher is being under-served.

**Contributor clarity vs. feature momentum.** Adding features without updating architecture docs erodes the Compiler Contributor experience. Signal: walk a new contributor through adding a feature using only existing docs and comments. If you hit an ambiguous step, that's the friction to fix.

**Cache-line discipline vs. implementation speed.** New types added without cache-line notes slip the discipline the project is built on. Signal: check whether types added in recent commits have cache-line commentary consistent with the rest of the codebase.

---

## What could be wrong

**Missing stakeholder: downstream academic/industry readers.** The project's research questions are interesting beyond the immediate development team. There may be an audience of compiler researchers, language designers, or Rust users who would benefit from the findings but aren't captured in the current four stakeholders. If the project ever publishes a paper or a blog post, that audience appears.

**"Cache-Line Performance Researcher" may be the same person as Lead Researcher.** In practice, these might not be different people — the Lead Researcher may be the only one who ever uses the benchmarks. If so, the champion shouldn't rotate to this stakeholder separately; treat cache-line concerns as part of the Lead Researcher's journey instead. Verify by looking at who actually uses `cargo bench`.

**Spec Researcher journey assumes `refs/fls-ambiguities.md` is navigable.** The existing goal.md in the repo root (from cycle 013 or so) was specifically written to fix a navigability problem in that file — out-of-order entries, no TOC. If that work has been merged, the Spec Researcher's journey now starts from a working TOC. The champion should check the actual state of the file each cycle rather than assuming it's broken or fixed.

**No branch protection visible.** Whether the default branch is protected (required reviews, required CI) is not visible from the local repo. This affects how much the autonomous agent loop can modify the repository without human review. The CI workflow has `permissions: contents: read` which is restrictive — but that only applies to the workflow itself, not to force pushes or direct commits.

**Repository security.** The CI workflow triggers on `push` to main and `pull_request` to main — not on `pull_request_target` or `issue_comment`, which are the high-risk trigger types for prompt injection attacks. The repo appears to be public. Prompt injection risk exists if external PR titles, issue bodies, or commit messages are fed verbatim into agent prompts — which lathe may do via its snapshot. The agent should treat CI metadata from external PRs as potentially adversarial.

**brand.md is absent.** The `.lathe/brand.md` file was deleted (per git status). The champion should treat brand as in emergent mode and skip the brand tint, falling back to stakeholder emotional signal only.

**Unverified assumption: FLS coverage is partial.** The architecture and test structure suggest galvanic supports a meaningful subset of Rust (functions, closures, structs, enums, generics, traits, some pattern forms, static items) but not the full language. The champion's maturation judgment (not yet working / core works / battle-tested) needs to be made fresh each cycle from the snapshot and from walking the journey — this document can't tell the agent where the current frontier is.
