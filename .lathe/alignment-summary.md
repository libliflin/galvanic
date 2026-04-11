# Alignment Summary

Quick-read for William before starting lathe cycles. 30 seconds to verify the framing is right.

---

## Who this serves

- **Spec investigator (William)** — researching whether the FLS is independently implementable; needs FLS citations to be real and deviations to be documented
- **Cache-line codegen researcher (William, different hat)** — studying whether first-class cache-aware design is measurably enforceable; needs enforced layout claims, not just comments
- **FLS contributor** — wants to extend galvanic to cover more FLS sections; needs clear patterns, working tests, and traceable citations as a guide
- **CI system** — quality gate that all three trust; must stay green

---

## Key tensions and how I resolved them

**FLS fidelity vs. coverage.** Favor fidelity. One correctly-cited, well-tested FLS section beats three loosely-cited ones. This is a research project — wrong implementations produce misleading data. This does not change.

**Cache-line enforcement vs. pragmatic progress.** Enforce layouts on hot structures now (Token, Span). Defer arena redesign for AST until the FLS mapping is stable. `ast.rs` itself documents this tradeoff — I followed it.

**Adversarial robustness vs. FLS progress.** CI already covers the minimum bar (no panics, no signal death on adversarial inputs). Maintain that bar; don't invest beyond it without a specific new stakeholder need.

---

## Load-bearing claims (what falsify.sh defends)

| Claim | What it checks | Stakeholder |
|-------|---------------|-------------|
| C1: Token == 8 bytes | `size_of::<Token>() == 8` via test | Cache-line researcher |
| C2: Span == 8 bytes | `size_of::<Span>() == 8` via test or grep | Cache-line researcher |
| C3: Build succeeds | `cargo build` exits 0 | All |
| C4: Tests pass | `cargo test --lib` exits 0 | Contributors, spec investigator |
| C5: No unsafe in library | grep for `unsafe` in `src/` excluding `main.rs` | Spec investigator |
| C6: Milestone-1 pipeline | `galvanic milestone_1.rs` emits `.s` | Spec investigator, researcher |
| C7: FLS citations present | each `src/` module has ≥1 `FLS §` | Spec investigator |

---

## Current focus

Galvanic is in **stage 2**: the core pipeline works for a real Rust subset, and CI is comprehensive. The highest-value work is:

1. **E2E coverage gaps** — many fixture programs have a `.s` file (emitted) but no e2e test that runs the binary via qemu. Each such fixture is a verification gap.
2. **Adversarial inputs** — does the full pipeline survive programs with function parameters (not literals) for every Instr variant? The const-evaluation constraint is stated in `fls-constraints.md` but may not be tested adversarially everywhere.
3. **FLS section coverage** — which sections are cited but not exercised by any fixture? Those are research gaps.

---

## What could be wrong

**Stakeholder I may have undersold:** The project has a GitHub Actions e2e job that requires Ubuntu + qemu + ARM64 cross toolchain. If lathe is running on macOS, the e2e tests won't pass locally — the agent needs to know this or it will confuse local test failures with actual bugs. I've documented this in `skills/testing.md`.

**C2 (Span == 8 bytes):** I found the `size_of::<Span>()` claim in `ast.rs` prose and CI references `lexer::tests::span_is_eight_bytes` — but I couldn't verify whether that test exists. `falsify.sh` tries the named test, falls back to a grep, and produces a "consider adding one" note. If the test doesn't exist yet, C2 will pass weakly — the agent will know to add the test.

**C6 (milestone_1 pipeline):** I couldn't run the binary to verify. If the `target/debug/galvanic` binary isn't present when falsify.sh runs (e.g., on a fresh checkout before C3 runs), C6 will fail. The script checks for the binary after the build step, so a failed build naturally cascades to a C6 failure with the right error message.

**The e2e test file is 1.1MB:** I couldn't read it. I don't know exactly what it covers. The agent will encounter it when the snapshot runs `cargo test` — if e2e tests fail, they'll surface there. The skills/testing.md documents the pattern so the agent can write new e2e tests correctly.

**Repo security for autonomous operation:** The repo is public (`libliflin/galvanic` on GitHub based on the README). Public repos have higher prompt-injection risk from issue/PR spam. The CI workflow uses `pull_request` trigger (not `pull_request_target`), which is safe — untrusted code doesn't get write permissions. The `audit` job uses `permissions: contents: read` at the top level. The default branch protection status is unknown — if you haven't enabled required PR reviews on `main`, consider it.
