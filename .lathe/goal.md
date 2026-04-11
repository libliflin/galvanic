# Goal: Create `refs/fls-ambiguities.md` — FLS Ambiguity Findings Index

## What

Create `refs/fls-ambiguities.md`: a discoverable document that aggregates all
`AMBIGUOUS` annotations from the galvanic source code (`src/`) into a single
index, grouped by FLS section, with a consolidated entry per finding.

The builder should:

1. Read every `AMBIGUOUS` comment in `src/ast.rs`, `src/codegen.rs`, `src/ir.rs`,
   `src/lexer.rs`, and `src/lower.rs` (there are ~155 total across these files).
2. Group findings by FLS section number (§2, §4, §5, §6, §7, §8, §9, §10, §11, §13, §15, §19).
3. Consolidate near-duplicate findings (e.g., multiple §6.23 overflow notes that
   say the same thing) into one entry per distinct gap. Target ~20–30 consolidated
   findings rather than 155 raw annotations.
4. For each consolidated finding, write an entry with:
   - **FLS citation** (e.g., `§6.9`)
   - **The gap**: what the spec leaves unspecified, ambiguous, or contradictory
   - **Galvanic's choice**: what the implementation does and why
   - **Source location**: which file(s) contain the annotation
5. Open the document with a 2–3 sentence introduction explaining its purpose:
   this is the primary research output of galvanic — a record of where the FLS
   fails to fully specify implementable behavior.

Each entry must be **self-contained and readable** without opening the source —
citable in a spec review or bug report without further context.

## Which stakeholder

**William (the researcher)** and **FLS spec readers (the Ferrocene team)**.

William's first research question is: where does the FLS fail to specify behavior
completely? The answer is in 155 annotations across 5 source files. He cannot
find it without reading every file. This change makes the primary research output
visible at a glance.

FLS spec readers — the people most likely to act on the findings — need a citable
document. A `refs/fls-ambiguities.md` can be linked from a GitHub issue, a spec
PR, or a blog post. A grepped list of code comments cannot.

## Why now

The project has 155 `AMBIGUOUS` annotations in `src/`. The build is clean. The
last 5 commits have all been forward progress on codegen (Claims 4g–4k). The
ambiguity-finding work has been continuous and real, but the research output has
never been surfaced in one place.

This is the first cycle of this session. The goal-history is empty. The highest-
leverage act for this cycle is to make existing findings visible — not to add
finding 156. One well-placed document now is worth more than the next incremental
feature.

---

## Stakeholders

### William Laffin — the researcher

William is the person reading the git log to decide whether lathe is earning its keep. He isn't using this compiler to build software. He's using it to answer two specific research questions:

1. Is the FLS actually implementable by an independent party — and where does it fail to specify behavior completely?
2. What happens when cache-line alignment is a first-class constraint from the start, not a bolt-on optimization?

His first encounter with each cycle's work is the commit message and the diff. Success for him looks like: a FLS section that was previously unimplemented now has correct **runtime** codegen with an assembly inspection test proving it didn't cheat, plus any ambiguity the implementation uncovered is noted in the code or a commit message.

What makes him trust the project: FLS citations in the code (`// FLS §X.Y: ...`), `// FLS §X.Y: AMBIGUOUS — ...` comments when the spec doesn't nail down behavior, and tests that verify runtime instruction emission (not just exit codes). The `compile_to_asm()` inspection pattern in `tests/e2e.rs` is the right shape for this.

What would make him walk away: cycles that produce correct exit codes but violate the core constraint (compile-time folding in non-const functions). This is the project's central integrity check. The litmus test is in `refs/fls-constraints.md` and it is non-negotiable: if replacing a literal with a function parameter would break the implementation, it's an interpreter, not a compiler.

Where the project is currently failing him: ambiguity findings are buried in code comments and cycle changelogs. There's no surface that collects "here's what the FLS doesn't specify" across all implemented sections. A contributor or spec reader cannot find the research output without reading every file.

### FLS spec readers and the Ferrocene team

These are people who want to know whether the FLS is a complete, implementable specification. They might be Rust language team members, compiler researchers, or people considering the FLS for regulated-industry use. They find the project through a blog post or the GitHub repo.

First encounter: the README and the `// FLS §X.Y: AMBIGUOUS —` comments scattered through the code. Success for them: a reproducible, citable case where the spec leaves a decision to the implementor — and documentation of what choice galvanic made and why.

What makes them trust the project: clean FLS citation discipline throughout the source. Every feature traceable to a spec section. Honest "AMBIGUOUS" labels on decisions the spec doesn't settle.

What would make them leave: FLS citations that are vague or wrong, or a project that appears to pass tests but has drifted from the spec. They have no way to verify the claims without reading the code — so the code has to be trustworthy.

Where failing: the ambiguity-finding work is real (the `fls-pointer.md` even instructs: "record every ambiguity in the FLS Notes section of the cycle's changelog"), but there is no single document that surfaces these findings. The research output isn't visible.

### Future contributors

Someone — a compiler researcher or an enthusiastic Rust developer — finds the project on a Sunday afternoon and has 90 minutes to decide whether it's worth their weekend. They read the README (4 short paragraphs), look at the test structure, and open `src/lower.rs`.

First encounter: the README explains what the project is and isn't. The code has dense FLS citations. The "Claim 4k: add while-let runtime falsification" commit style signals there's a systematic methodology, but doesn't explain it.

Success: they understand the contribution model (pick a FLS section, implement runtime codegen for it, add assembly inspection and e2e tests, note ambiguities) without needing to ask. They can look at a recent "Claim" PR and replicate the pattern.

What would make them trust the project: the CI catches violations of the core constraint (though currently it doesn't: a commit that folds `1 + 2` to `#3` at compile time would pass CI if the e2e test only checks exit code). The assembly inspection tests in `e2e.rs` close this gap — but only for features that have them.

What would make them leave: the contribution path isn't documented. There's no CONTRIBUTING.md and the README doesn't explain the "Claim N: falsification" commit pattern or what a new FLS claim looks like.

Where failing: contributor onboarding is implicit. The pattern is clear from the commits, but it lives nowhere that a newcomer would find without spelunking.

### CI and the validation infrastructure

CI runs on every push and PR: build, test, clippy, fuzz-smoke, audit (no unsafe, no Command in lib, no network deps), e2e (cross-compile + qemu on ubuntu-latest), and bench. This is a real floor — a broken build is the highest-priority fix before anything else.

The e2e job installs `binutils-aarch64-linux-gnu` and `qemu-user` explicitly, so the full pipeline (lex → parse → lower → codegen → assemble → link → run) is verified on every PR.

The fuzz-smoke job verifies that adversarial inputs (garbage, NUL bytes, deeply nested braces, huge files) produce clean errors rather than crashes or hangs.

What CI does not currently enforce: that new features use runtime codegen rather than compile-time evaluation. An assembly inspection test (using `compile_to_asm()`) must be added alongside every new e2e feature test for CI to catch the core constraint. This gap is the most important structural weakness.

---

## Tensions

### Parse acceptance vs. full pipeline

About 40 parse fixture tests verify that galvanic can lex and parse FLS examples. A larger set of features — closures (§6.14), for loops (§6.15.1), range expressions (§6.16), generics (§12), traits (§13) — have parse fixtures but no e2e codegen.

Adding a parse fixture is not research progress. The research question is whether the FLS specifies runtime behavior completely enough to implement correct codegen. A feature doesn't answer that question until it has an e2e test with assembly inspection.

**Signal:** If recent commits are adding parse fixtures rather than e2e codegen + assembly inspection tests, the project has drifted. A cycle that adds a parse fixture for a feature that already has one advances nothing. A cycle that takes an existing parse fixture all the way to runtime codegen is real progress.

### FLS coverage breadth vs. correctness depth

The project could widen — add more FLS sections to the codegen pipeline. Or it could deepen — add assembly inspection tests to features that currently have only exit-code e2e tests, stress-test edge cases, or surface ambiguities from already-implemented sections.

Neither is inherently better. Breadth answers "how much of Rust can galvanic compile?" Depth answers "how correct is what it does compile?"

**Signal:** If implemented features (let bindings, if/else, function calls, loops, match) all have assembly inspection tests, breadth is appropriate — the floor is solid. If any feature has only an exit-code e2e test with no assembly inspection, depth matters first.

### Ambiguity documentation vs. forward progress

The primary research output is discovering where the FLS is silent, ambiguous, or contradictory. Each time the implementation hits a spec gap — documented as `// FLS §X.Y: AMBIGUOUS —` in code, or in a commit note — that's a finding.

But ambiguity documentation competes with cycle time. Spending a cycle on surfacing existing ambiguities (writing a summary doc) means not implementing a new FLS section.

**Signal:** If recent commits have added `// FLS §X.Y: AMBIGUOUS —` comments but there's no surface collecting them, one well-placed cycle to aggregate those findings into a visible document pays off more than the next incremental feature. If ambiguities are sparse, forward progress is the higher value.

### Cache-line integrity vs. feature growth

Token is 8 bytes (enforced by `lexer::tests::token_is_eight_bytes`). This is the cache-line constraint made visible. As the IR gets more expressive — closures require captures, generics require type parameters, match requires discriminant logic — the IR types will grow.

The cache-line constraint is research artifact, not just a performance trick. If the IR has to give up 8-byte tokens to handle closures, that's a finding worth noting.

**Signal:** If Token size tests are passing and IR growth is modest, the cache-line constraint is healthy. If a new feature requires Token or core IR types to grow, document the tradeoff explicitly rather than silently relaxing the constraint.

---

Every cycle, ask: **which stakeholder's journey can I make noticeably better right now, and where?**

---

## How to Rank

**CI and tests are the floor.** Read the snapshot. If the build is broken, tests are failing, or clippy has errors, that is the goal — fix the break before anything else. A red CI means no new features until it's green.

**Above the floor, rank by stakeholder impact.** When nothing is broken, the question is: which stakeholder's journey can I make noticeably better? The Tensions section is the tiebreaker when two directions seem equally valuable.

Do not encode a fixed ordering of feature categories. "Codegen before tests before docs" is a frozen spec wearing values clothing. The project's test suite and CI enforce the floor. Above that, stakeholder impact decides.

---

## What Matters Now

Assess the project state honestly each cycle. The right questions depend on where the project is.

**If features have e2e tests but no assembly inspection tests:**
- Does the feature actually emit runtime instructions, or could it be silently folding at compile time? The exit code won't tell you — the assembly will.
- Is there a `compile_to_asm()` test asserting the correct instruction (e.g., `add`, `sub`, `mul`, `cbz`) for this feature?
- Does the test assert that the constant-folded form (e.g., `mov x0, #3`) does NOT appear?

**If the build and tests are clean:**
- Which FLS section in `§6` (expressions) or `§8` (statements) is most frequently referenced in parse fixtures but absent from `tests/e2e.rs`? That's the next natural claim.
- Are for loops (§6.15.1), range expressions (§6.16), or closures (§6.14) implemented at the codegen level? These are common and their absence limits what programs galvanic can compile.
- Have the recently-added features (while-let, match, struct-expressions) been stress-tested with realistic inputs, or only with the minimal cases from their initial commit?

**If ambiguity notes are accumulating in code but not surfaced:**
- Is there a commit pattern where `// FLS §X.Y: AMBIGUOUS —` appears in multiple files with no corresponding visible document? If so, aggregating those findings into a discoverable place is higher value than the next incremental feature.

**Always:**
- Would the change survive the litmus test in `refs/fls-constraints.md`? If replacing a literal with a function parameter would break the implementation, the claim is not complete.
- Is the FLS citation accurate — does the cited section actually say what the comment claims?
- Never treat any list — in a README, an issue, or a snapshot — as a queue to grind through. Lists are context.

---

## The Job

Each cycle:

1. **Read the snapshot** — build status, test results, git log, clippy output. A red build or failing test is the goal before anything else.
2. **Read the last 4 goals** from goal history — to avoid repeating yourself and to assess whether the project is building momentum or spinning.
3. **Read the theme** if set — it biases which stakeholder to prioritize this session. It doesn't override the CI floor.
4. **Pick the single highest-value change** — the one thing that, if done well, makes a real person's relationship with this project meaningfully better.
5. **Write a goal file** that names: **what** to change, **which stakeholder** it helps, and **why now**.

The goal file is committed to the repo. The builder reads it and implements it.

When picking, imagine someone concrete: William opening the git log this evening, or a compiler researcher reading a `// AMBIGUOUS` comment and wondering where the other ones are, or a contributor staring at `tests/e2e.rs` trying to figure out which FLS section to tackle next.

The goal should be the change that makes the biggest difference to that person's experience, today.
