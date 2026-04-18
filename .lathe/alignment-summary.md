# Alignment Summary — Galvanic Champion Init

For the human maintainer. Not read by the runtime agents.

---

## Who this serves

- **Lead Researcher** — the project's author; uses galvanic to explore FLS completeness and cache-line-aware codegen. Needs momentum: the frontier should move each cycle.
- **Spec Researcher** — an FLS student or contributor who uses `refs/fls-ambiguities.md` as a citable reference for where the spec is silent. Needs confidence: the registry should be navigable, sorted, and citable.
- **Compiler Contributor** — a developer who wants to extend galvanic's language coverage. Needs clarity: error messages should name the FLS section, CI should enforce invariants, the architecture should be self-documenting.

---

## Emotional signal per stakeholder

- **Lead Researcher:** momentum — "the frontier moved; I can compile something I couldn't yesterday"
- **Spec Researcher:** confidence and authority — "I can trust this registry; I can cite it"
- **Compiler Contributor:** clarity — "I know exactly where to make this change and what invariants I need to uphold"

---

## Key tensions

**Research progress vs. stability.** The Lead Researcher wants new language coverage; the Compiler Contributor needs CI green and no regressions. Signal: if recent commits include `fix: §X.Y regression`, stability is losing.

**Registry growth vs. navigability.** New ambiguity entries are valuable but erode usability if appended out of order. Signal: if entries in `refs/fls-ambiguities.md` are out of FLS section order, or the TOC doesn't match the body, the Spec Researcher is being deprioritized.

**FLS fidelity vs. ARM64 pragmatism.** The spec is silent on many codegen decisions. Signal: new IR variants or codegen cases without `// FLS §X.Y` citations or corresponding `refs/fls-ambiguities.md` entries are silent assumptions.

**Cache-line discipline vs. implementation reach.** New IR types should carry size assertions, but adding them requires knowing the final size. Signal: new IR variants in `ir.rs` without `assert_eq!(size_of::<T>(), N)` tests.

---

## What could be wrong

**Missing `ambition.md`.** The champion's "What Matters Now" section references `.lathe/ambition.md` to measure project maturation against the stated destination. This file does not exist. The champion will fall back to journey-only maturation (polish is legitimate earlier), which is the right fallback — but if you want the champion to target the research horizon rather than the nearest friction, write `ambition.md`. A suggested starting point: "Compile realistic `no_std` Rust programs end-to-end on ARM64: functions with loops, conditionals, structs, and non-trivial exit codes — with every FLS gap documented."

**Missing `brand.md`.** No brand file was found. The champion will skip the brand-as-tint step and use stakeholder signal alone to break ties between friction moments and fix directions. If you want brand to influence the champion's voice and the fix directions it proposes, write `.lathe/brand.md`.

**Default branch protection not verified.** The CI workflow has `permissions: contents: read` and no `pull_request_target` or `issue_comment` triggers — the workflow configuration is secure. However, whether the `main` branch has push protection enabled on GitHub was not verified during init (requires API access). Recommend confirming branch protection is enabled to prevent direct pushes bypassing CI.

**Repository visibility not verified.** The repo appears public based on the README (`libliflin/galvanic`). Lathe reads CI status and PR metadata from GitHub and feeds it into the agent prompt — this is a potential prompt injection surface. Any PR from an external contributor could contain content that influences the agent's prompt. Mitigation: the CI workflow's minimal permissions (`contents: read`) limit the blast radius. If the repo becomes a target for adversarial input, consider adding a step that sanitizes PR titles/descriptions before feeding them to the agent context.

**No `fls-ambiguities.md` domain coverage for the contrib-side.** The `refs/fls-ambiguities.md` is well-organized for the Spec Researcher, but there's no skill file documenting *how to add a new entry* (format, required fields, where to find the source annotation). This gap affects the Compiler Contributor when they discover a new ambiguity mid-implementation. Consider adding a brief format guide to `refs/fls-ambiguities.md`'s intro section or to `skills/architecture.md`.

**Only three stakeholders identified.** The stakeholder map may be incomplete. Possible missing stakeholders: (1) a downstream `no_std` Rust developer who wants to check whether a specific Rust construct is FLS-conforming; (2) a compiler course student using galvanic as a worked example; (3) an operator running galvanic in a CI pipeline to continuously test FLS compliance. If any of these exist, they should be added to the stakeholder map.

---

## Ambition

**Destination:** Galvanic answers both research questions with evidence across the full `no_std` Rust surface — a citable FLS ambiguity map covering every section, and a worked demonstration of cache-line-first codegen on programs with real register pressure.

**Current gap(s):** (1) No real register allocator — virtual registers ≥31 silently reuse x9 scratch, so any program with >30 live variables compiles incorrectly. (2) Large-immediate encoding (MOVZ+MOVK) not yet implemented — FLS §2.4.4.1 gap. (3) Pattern matching surface incomplete — tuple scrutinee in match, guards on non-last arms, nested tuple/@ binding patterns all error with "not yet supported." (4) Ambiguity registry does not yet cover FLS §15+.

**What could be wrong:** The README frames the destination as pure research ("value comes from what we learn"), not as a specific compilation target. I read "real programs with real register pressure" as the implicit bar for research question 2, but the project never names a specific program or language subset as the finish line. If the Lead Researcher has a concrete target program in mind (e.g., a specific `no_std` crate), that should override the inferred destination here. The register allocator gap is structural and obvious; the FLS §15+ coverage gap is inferred from the registry TOC — the researcher may have already explored those sections without writing entries.
