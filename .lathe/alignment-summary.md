# Alignment Summary

A 30-second brief on the decisions I made during init. Read this, gut-check it, then start cycles.

---

## Who This Serves

- **William** — the researcher. Primary stakeholder. Reads the git log to judge whether the tool is earning its keep. Cares about two things: (1) FLS ambiguities surfaced by the implementation, and (2) whether treating cache-line alignment as a first-class constraint from the start produces measurably different outcomes. Both questions require the compiler to be *correct*, not just working.

- **Future contributors** — anyone who finds the repo and spends 90 seconds deciding whether to contribute. The code needs to be intelligible: consistent FLS citations, clear design decisions, tests that aren't cheating.

- **The FLS / Ferrocene spec** — galvanic tests the spec. Every section it can implement cleanly is evidence. Every ambiguity it encounters is the research output. The `// FLS §X.Y: AMBIGUOUS — <description>` pattern in the code is the primary deliverable.

---

## Key Tensions

**FLS parse coverage vs. lowering depth**: There are ~40 parse-acceptance fixtures but many FLS sections have no lowering or codegen tests. I favored **depth over breadth** at the current stage — promoting a parse-only fixture to a runtime test is more valuable than adding another parse-only fixture. This flips if there are obvious parser gaps.

**Cache-line design vs. FLS compliance**: I favored **FLS compliance** as the harder constraint. The cache-line design is documented and argued; if they conflict, document the tradeoff as an FLS note.

**Compiler correctness vs. milestone pace**: No contest. The FLS constraints file (`refs/fls-constraints.md`) is explicit: a compiler that constant-folds non-const code is wrong, even if it produces the right exit codes. I encoded this as a falsification claim (Claim 4).

---

## Load-Bearing Claims

The falsification suite checks these every cycle:

1. **Build succeeds** — `cargo build` exits 0. Baseline for everything.
2. **Token is 8 bytes** — `size_of::<Token>() == 8`. Structural invariant for the cache-line design hypothesis.
3. **FLS parse-acceptance suite passes** — all 40+ tests in `tests/fls_fixtures.rs` pass. Regression guard on the parser.
4. **Non-const code emits runtime instructions** — `fn main() -> i32 { 1 + 2 }` produces an `add` instruction, not `mov x0, #3`. The most important correctness property: galvanic must be a compiler, not an interpreter.
5. **Adversarial inputs exit cleanly** — empty file, binary garbage, and 300-deep nested braces don't panic or hang.

---

## Current Focus

The agent will prioritize: **FLS sections that parse but don't lower or codegen**. Many sections have parse fixtures but no runtime tests. The highest-value cycles promote a parse-only section to a runtime-verified section, which produces FLS evidence.

Second priority: assembly-inspection tests for features already in the lowering pass but not yet verified at the instruction level.

---

## What Could Be Wrong

**Stakeholder I might have missed**: The project is currently single-author. If William has specific collaborators or a downstream audience for his research (a paper, a talk, a course), they're stakeholders I haven't named. The agent.md treats the FLS team as an indirect stakeholder, but if there's a specific human reading William's findings, they deserve their own entry.

**Claim 4 assumption**: The falsification check for "runtime codegen" compiles `fn main() -> i32 { 1 + 2 }` and checks for `add` in the assembly. This assumes galvanic actually handles this program without error. If the lowering pass currently fails on this exact program (regression), the claim check would fail for the wrong reason. I believe this works based on the milestone history in the code, but I couldn't verify by running `cargo test` during init.

**Claim 4's `add` heuristic**: ARM64 uses `add` for integer addition. But the codegen might emit `adds` (add + set flags) or `add` with immediate form. The grep is for the literal string `add` as a whole word, which should match ARM64's `add x1, x0, x2` form. If the codegen emits `madd` or similar, the check might produce a false pass. This is intentionally simple — refine it if the heuristic ever gives false results.

**falsify.sh not executable**: I couldn't run `chmod +x` in this environment. **Before starting cycles, run:**
```bash
chmod +x .lathe/falsify.sh
```

**Branch protection**: I could not verify whether the GitHub repo's default branch has push protection enabled. For an autonomous agent writing to feature branches and creating PRs, this is the main security control. Confirm that direct pushes to `main` are restricted in GitHub Settings → Branches → Branch protection rules.

**No `pull_request_target` risk**: The CI workflow (`ci.yml`) triggers on `push` to `main` and `pull_request` to `main`. No `pull_request_target` or `issue_comment` triggers — those would allow untrusted input to run with elevated permissions. The repo's CI is low-risk for prompt injection.

**Public repo + autonomous agent**: The repo is public. Anyone can submit a PR. The lathe engine reads structured CI data (statuses, numbers) — not free-text fields like PR comments or titles. But if your engine setup ever adds PR comment reading, revisit this.

**`snapshot.sh` already runs `cargo test`**: The snapshot collects full test output. The falsify.sh also runs `cargo test --test fls_fixtures`. This means the FLS parse tests run twice per cycle (once in snapshot, once in falsify). This is intentional — the falsification suite is the authoritative pass/fail signal; the snapshot provides context. If the cycle time becomes too slow, retire the snapshot's `cargo test` and rely on falsify.sh instead.
