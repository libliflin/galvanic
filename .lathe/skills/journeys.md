# Stakeholder Journeys

Concrete step-by-step journeys the customer champion walks each cycle. One per stakeholder. These are the paths to walk — not describe abstractly, but actually execute — when inhabiting each person.

---

## Lead Researcher

**Emotional signal:** Momentum — each run tells them something new, and the output is always trustworthy.

**The journey:**

1. `git pull && cargo test` — watch for any red. If red, the whole session is about fixing it.
2. Pick an FLS fixture from `tests/fixtures/`. Either use an existing one or write a new program from a spec section not yet covered.
3. `cargo run -- tests/fixtures/<fixture>.rs` and read the output.
   - Green path: `galvanic: emitted <fixture>.s`. Open the `.s` file. Check that the assembly has runtime instructions (not folded immediates), that register usage matches the AAPCS64 ABI, and that cache-line-critical structs are laid out as documented.
   - Partial path: `lowered N of M functions (K failed)`. Check whether each failing function is named in stderr. Check whether the failing construct is named specifically (e.g., "not yet supported: TupleExpr in match scrutinee"). Ask: given this error, do I know where to look next?
   - Full failure: No assembly emitted. Is the error message specific enough to navigate to the relevant FLS section and source location?
4. If a new ambiguity surfaced during the run, check `refs/fls-ambiguities.md`. Is there already an entry? Is the entry accurate? Does it name galvanic's resolution?
5. Try the exact same program with one value replaced by a parameter (e.g., `fn main() -> i32 { 42 }` → `fn foo(x: i32) -> i32 { x }`). The output should use a register, not a constant. If it doesn't, const-folding has leaked into non-const context.

**What to watch for:**
- Any "not yet supported" error that doesn't name the FLS section and the specific construct.
- A partial failure where the failing function count in the summary doesn't match the number of error lines in stderr.
- An emitted `.s` file that folds a constant that should be a runtime computation.
- An ambiguity entry in `refs/fls-ambiguities.md` that says "see the code" without stating the resolution.

---

## Spec Researcher

**Emotional signal:** Curiosity satisfied — "I found a citable finding in under two minutes."

**The journey:**

1. Land on the repo. Read the README. Note the two research questions the project is answering.
2. Open `refs/fls-ambiguities.md`. Find the table of contents. Try to navigate to a specific section — say, §6.22 (Closures) or §6.18 (Match expressions).
3. Read one finding end-to-end. Does it clearly state: (a) what the FLS says, (b) what the FLS leaves unspecified, (c) what galvanic chose to do, and (d) where in the source to find the annotation?
4. Try to find *all* findings related to a topic. Example: everything about floating-point (§6.5.3, §6.5.5). Can you find them all without reading the whole file?
5. Open `refs/fls-constraints.md`. Try to understand one constraint end-to-end — the FLS citation, the status, and the litmus test.

**What to watch for:**
- Missing TOC, or TOC entries that are out of sync with the body.
- Entries in body order that doesn't match section number order.
- A finding that describes the spec gap but doesn't state galvanic's resolution.
- A constraint entry where the "status" field says something like "partially" without explaining what's missing.
- Inconsistency between `fls-ambiguities.md` entries and the actual code annotations.

---

## Compiler Contributor

**Emotional signal:** Confidence — "I know exactly where to make this change."

**The journey:**

1. Clone the repo. `cargo build`. `cargo test`. Confirm green.
2. Find a feature that produces "not yet supported." Example: run `galvanic` on a program that uses a construct not yet implemented. Read the error.
3. Open `lib.rs` to understand the module structure.
4. Trace the error back through the source: which module produced the "not yet supported" message? Which FLS section does the comment cite?
5. Open `lower.rs` or `codegen.rs`. Find the match arm where a new case would go. Look for an existing similar case to pattern-match from.
6. Add the new case. Write a fixture in `tests/fixtures/`. Add a test in `tests/fls_fixtures.rs`. Run the tests.
7. Try to write the PR description. Ask: can you describe what you changed, which FLS section it covers, and why the approach is correct — without looking anything up?

**What to watch for:**
- A place in the pipeline where the module seam is invisible — you can't tell from the code which module "owns" a transformation.
- An IR node added without an FLS traceability comment.
- A fixture in `tests/fixtures/` that doesn't have a corresponding test in `fls_fixtures.rs`.
- A "not yet supported" error in the CLI that doesn't identify the FLS section or specific construct.
- An architecture decision that's implied by the code but never documented — especially around the IR's role as the bridge between FLS semantics and machine instructions.

---

## Cache-Line Performance Researcher

**Emotional signal:** Verifiable — "the claim is documented, tested, and visible in the output."

**The journey:**

1. Read the README. Find the cache-line claim ("cache-line alignment as a first-class concern").
2. `cargo bench` — look at the throughput numbers.
3. Find the size assertion tests: `cargo test --lib -- --exact lexer::tests::token_is_eight_bytes`. Do they pass? Are there similar tests for other cache-critical types?
4. Compile a test program: `galvanic tests/fixtures/fls_2_4_literals.rs`. Open the emitted `.s` file. Find where a token-stream loop would be. Is the data structure laid out as claimed?
5. Find the cache-line comment in `src/lexer.rs` (Token is 8 bytes, 8 per cache line). Find the corresponding test. Can you trace the full argument: claim → code → test → benchmark?
6. Look for a recently added IR node or data structure. Does it have a cache-line note consistent with the rest of the codebase?

**What to watch for:**
- A cache-line claim in a comment that has no corresponding size test.
- A new struct added in a recent commit without a cache-line note, in a file where all other structs have them.
- Benchmark output that doesn't report throughput (bytes/sec or tokens/sec) — only raw time is hard to interpret for cache analysis.
- An emitted `.s` file where the layout of a cache-critical struct differs from the documented layout.
