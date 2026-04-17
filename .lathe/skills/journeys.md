# Stakeholder Journeys — Galvanic

These are the concrete journeys the customer champion walks each cycle. Each journey is the first 10–15 minutes of a stakeholder's encounter with galvanic at any point in the project's life. Walk the steps, run the commands, read the output. The emotional signal tells you whether the experience was good, hollow, or broken.

---

## Journey 1: The Lead Researcher extends a feature

**Emotional signal to track:** Momentum — the compiler is getting smarter, findings are being captured.

**Steps:**

1. `git pull` — get latest main. Check: any CI failures in recent commits?
2. `cargo test` — confirm the floor is intact. All tests green?
3. Pick a Rust construct to implement. Find a fixture in `tests/fixtures/` that only has a parse test (`.rs` file exists but no e2e entry for it). That's the next feature to work on.
4. Read the FLS section for the construct. Find it via `refs/` — check `.lathe/refs/fls-pointer.md` for the table of contents.
5. Find an existing, similar feature in `src/lower.rs` as a template. (Example: if adding a new binary operator, find how `Add` or `Sub` is handled.)
6. Write a minimal Rust program using the new construct:
   ```
   echo 'fn main() -> i32 { <new construct here> }' > /tmp/test_feature.rs
   cargo run -- /tmp/test_feature.rs
   ```
7. Read the output — does `galvanic` accept it? Does it emit an `.s` file? Is the assembly correct?
8. Run `cargo test` — any new failures?
9. Check the emitted assembly: does it contain runtime instructions (not a constant result)? Does it match the AAPCS64 ABI?
10. Write the FLS citation in the new code. Check `refs/fls-constraints.md` for relevant constraints.
11. Did you encounter anything the FLS doesn't specify? Add an `AMBIGUOUS: §N.M — ...` annotation and a matching entry in `refs/fls-ambiguities.md`.
12. Does the new IR node have a `Cache-line note`? If not, add one.

**Friction signals:**
- A construct from `tests/fixtures/` can't be lowered even though it parses — the pipeline has a gap.
- The emitted assembly doesn't match the ABI (wrong register, wrong calling convention).
- An `AMBIGUOUS` annotation exists in source but has no matching entry in `refs/fls-ambiguities.md` — a finding was recorded in code but not in the ref file.
- A new IR node was added without a cache-line note — the research artifact is incomplete.
- `cargo test` output doesn't tell you which test failed or why — the test message is unhelpful.

---

## Journey 2: The Spec Researcher audits a finding

**Emotional signal to track:** Discovery — a specific, grounded finding about an FLS gap.

**Steps:**

1. Read the README — understand the research goals and that this is not a production compiler.
2. Find `refs/fls-ambiguities.md`. Does it have a table of contents or index? Can you navigate to a specific FLS section without reading the whole file?
3. Pick an FLS section you care about (say §6.5, arithmetic operators, or §9, functions).
4. Search for annotations in source:
   ```
   grep -r 'AMBIGUOUS' src/ | grep '§6.5'
   ```
5. For each annotation found, navigate to the corresponding entry in `refs/fls-ambiguities.md`. Is it there?
6. Read the entry — is it specific enough to act on? Does it name: the FLS section, the gap, and galvanic's resolution?
7. Try to write a minimal Rust program that demonstrates the ambiguity:
   ```
   echo '<minimal program>' > /tmp/ambig_test.rs
   cargo run -- /tmp/ambig_test.rs
   ```
8. Does the output confirm the finding? Is the behavior exactly what the ambiguity predicts?
9. Check: is the FLS section number still valid? (The FLS is versioned and may have been updated.)

**Friction signals:**
- `refs/fls-ambiguities.md` has no section index — navigating to a specific FLS section requires reading the whole file.
- An `AMBIGUOUS` annotation exists in source but has no matching entry in the ref file — a finding was captured in code but not documented.
- An entry in `refs/fls-ambiguities.md` is vague: "the spec probably doesn't cover this" without a concrete example.
- A section number in source doesn't match the current FLS (spec was updated, annotation is stale).
- There's no way to reproduce the ambiguity from the documented entry — no minimal example, no test.

---

## Journey 3: The Compiler Contributor adds a feature

**Emotional signal to track:** Clarity — I know exactly where this goes and how to do it right.

**Steps:**

1. Clone the repo:
   ```
   git clone https://github.com/libliflin/galvanic
   cd galvanic
   cargo build && cargo test
   ```
   Does it build? Do all tests pass? This is the first trust signal.
2. Read the README. Understand that this is a research compiler, not a production tool, and that every feature needs FLS citations and cache-line notes.
3. Look at `src/` — six files. Read the module doc comment at the top of each file (1–2 minutes each) to understand each file's job.
4. Find a feature to add. Look at `tests/fixtures/` — pick a `.rs` file that has a parse test in `fls_fixtures.rs` but no e2e entry. That's a feature that's parsed but not yet compiled.
5. Find the relevant FLS section (use `refs/` → `.lathe/refs/fls-pointer.md`).
6. Find an existing, similar feature as a template. For example:
   - To add a new binary operator: find `IrBinOp::Add` in `src/ir.rs`, then `BinOp::Add` in `src/lower.rs`, then `IrBinOp::Add` in `src/codegen.rs`.
   - To add a new expression type: find `ExprKind::If` or `ExprKind::Loop` and trace it through all six files.
7. Add the feature through the pipeline in order: AST → parse → lower → IR → codegen → test.
8. After each step, run `cargo test` to check nothing broke.
9. Add a parse acceptance test in `fls_fixtures.rs` and an assembly inspection test in `e2e.rs`.
10. Write FLS citations for each new code path. Write a cache-line note for any new IR node.
11. Run `cargo clippy -- -D warnings` — fix any warnings.
12. Push a PR — watch CI. All five jobs should pass.

**Friction signals:**
- The build fails after cloning — the contributor is blocked before they start.
- The test output on failure doesn't point to the right file or line — the contributor has to read the full backtrace to find the problem.
- The pattern for adding a new binary operator is clear in `lower.rs` but the cache-line note format in `ir.rs` is unclear — the contributor doesn't know what to write.
- `fls_fixtures.rs` doesn't have a clear comment explaining the pattern for adding a new fixture test.
- The FLS section for the feature isn't obvious — the contributor has to read the whole spec to find it.
- A feature can be parsed but the lowering path is completely missing — no error message tells the contributor what happened, just a panic or an opaque `codegen: not yet supported` message.

---

## What to watch across all journeys

**The hollowest moments:**
- Something compiles without error but the output is wrong (constant folding instead of runtime code — the compile looks successful but the research invariant was violated).
- An `AMBIGUOUS` annotation in source that leads nowhere (finding captured but not documented).
- A test that passes because it checks the exit code but not the assembly (coverage that isn't).

**The highest-value moments:**
- A new feature works end-to-end — the binary runs and produces the correct exit code on CI.
- A new ambiguity is discovered, annotated in source, and documented in `refs/fls-ambiguities.md` with a minimal reproducer.
- A contributor adds a feature and the architecture was discoverable enough that they didn't have to ask for guidance.
