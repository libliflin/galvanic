# Stakeholder Journeys

Concrete first-encounter journeys the champion walks each cycle. One per stakeholder. Each journey names the entry point, the steps, and the emotional signal to track.

---

## Lead Researcher

**Emotional signal:** momentum — *the frontier moved.* "I can compile something I couldn't yesterday."

**Journey: Compiling a new FLS-section program**

This is the Lead Researcher's day-to-day work. They pick a Rust construct the project doesn't yet handle and try to compile a realistic example of it.

1. Browse `refs/fls-ambiguities.md` to find a gap that's been documented but not yet resolved with working codegen.
2. Write a minimal but non-trivial Rust program that exercises that construct. Not `fn main() { let x = 5; }` — something that actually uses the feature. For example, for §6.15 loop expressions: a `while` loop that accumulates a value.
3. Run `cargo run -- /path/to/example.rs`.
4. Read the output: does it say `galvanic: emitted example.s`? Or `error: lower failed in 'main'`?
5. If it emitted assembly, open the `.s` file. Check: are runtime instructions present (branch instructions for the loop, not a folded constant)? Do the register choices reflect cache-line discipline?
6. If it errored, read the error message. Does it cite the FLS section? Does it name the failing construct? Can you navigate directly to the fix?

**Where the experience turns:**
- Good: `galvanic: emitted example.s` with correct ARM64 runtime instructions and an FLS section citation in the assembly comment. Frontier moved.
- Bad: `not yet supported: while expression` with no FLS citation, no hint about which IR construct is needed. Black box — no momentum.
- Hollow: Assembly emitted, but it's a `mov x0, #result` (compile-time folding) where there should be a `cmp`/`blt`/branch loop. The test passed but the invariant was violated.

**Harder journey (when the first completes cleanly):**
Compile a multi-function program that passes values between functions, uses a loop, and returns a non-trivial exit code. This exercises the full pipeline: call convention, register allocation across functions, branch instructions, stack frames.

---

## Spec Researcher

**Emotional signal:** confidence and authority — *I can trust this, I can cite it.* "This is the registry I'll reference in my talk."

**Journey: Finding all findings for a specific FLS section**

They're preparing a talk or document on a specific FLS section and want everything galvanic has found.

1. Open `refs/fls-ambiguities.md`.
2. Read the introductory paragraph. Does it match the file's actual organization?
3. Use the table of contents to jump to the section they care about (e.g., §6.5 — Operator Expressions).
4. Read the entries. Is each one precise? Does each have a gap description, galvanic's resolution, a source location, and a minimal reproducer?
5. Check: is the reproducer a complete, compilable program with `fn main()`?
6. Verify: can they find all entries for their section without scanning the full file? Or do related entries appear in multiple non-adjacent locations?

**Where the experience turns:**
- Good: TOC present, sorted by FLS section, entries in matching order. Jump directly to §6.5 — find all floating-point findings grouped together. Each has a reproducer that compiles (or fails in the stated way).
- Bad: No TOC, or TOC present but entries are out of order in the body. A §6.15.6 entry appears 335 lines after §6.15.1 because it was added in a later cycle and appended at the end.
- Hollow: Entry says "FLS §6.5.3: NaN comparison behavior" but the gap description is two sentences and the reproducer is `fn main() { }` — no actual demonstration of the gap.

**Harder journey:**
Count how many findings relate to floating-point semantics (§6.5.3, §6.5.5, §6.5.9 float-to-int). This requires knowing which sections cover floats, scanning all of them, and being confident no entries were missed. If the registry isn't sorted and navigable, this is impossible without reading every entry.

---

## Compiler Contributor

**Emotional signal:** clarity — *I know exactly what to do next.* "The error told me which function, which FLS section, and what's missing."

**Journey: Implementing a new FLS section**

They want to add support for a language construct galvanic doesn't yet handle.

1. `git clone`, `cargo build` — does it build cleanly? How long does it take?
2. Run `cargo test` — all green?
3. Open `src/lib.rs`. Read the module-level docs: pipeline overview, module table, "Adding a new language feature" section.
4. Choose a feature to implement: pick something from the unsupported constructs. (A good starting point: run `cargo run -- tests/fixtures/fls_6_expressions.rs` and see what errors.)
5. Read the FLS section for the chosen feature.
6. Add AST types to `src/ast.rs`, a parser case to `src/parser.rs`.
7. Add IR variant(s) to `src/ir.rs` with FLS traceability comment and cache-line note.
8. Add lowering case to `src/lower.rs`.
9. Add codegen case to `src/codegen.rs`.
10. Write a fixture in `tests/fixtures/fls_<section>_<topic>.rs`.
11. Add parse-acceptance test in `tests/fls_fixtures.rs`.
12. Add assembly inspection test in `tests/e2e.rs`.
13. Run `cargo test`. Does the new test pass? Did anything regress?
14. Run `cargo clippy -- -D warnings`. Any warnings?

**Where the experience turns:**
- Good: Step 3 gives a clear map. Step 11 produces a clean error message naming the function, FLS section, and construct. Steps 6–12 each have a clear home.
- Bad: Step 11 says `not yet supported: some expression` — no function name, no FLS section, no hint about where to add the fix. Contributor is stuck.
- Hollow: CI passes, but the e2e test checks the exit code rather than the assembly. A compile-time fold would also produce the right exit code — the test doesn't actually catch the invariant violation.

**Harder journey:**
Implement a feature that crosses multiple pipeline stages: a new expression form that requires new AST nodes, a new IR instruction, a new lowering rule, and a new codegen pattern — all in one PR, with full test coverage.
