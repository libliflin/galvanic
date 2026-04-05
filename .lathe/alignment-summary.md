# Alignment Summary

For William to read before starting lathe cycles. Takes about a minute.

---

## Who this serves

**William (you)** — The only stakeholder. Galvanic is a research compiler. Nobody else is running it, depending on its API, or waiting on features. Every cycle should serve your ability to answer the two research questions: (1) is the FLS independently implementable, and (2) what does cache-aware codegen actually look like?

There are no external users, no library consumers, no downstream teams. This keeps the alignment simple.

---

## Key tensions I found

**Breadth vs. depth.** You've implemented lexing and parsing of `fn` items with full expression support (Phase 1 and 2 are committed). The FLS has many more sections: structs, enums, traits, impls. The temptation is to keep extending the parser. But the existing code has almost no behavioral tests — the only test is a smoke test that checks the binary runs. I've aligned the agent to prioritize testing what exists before implementing more.

**Cache-line research vs. getting something working.** The Token/Span layout is carefully designed. The AST docs explicitly say "get the FLS mapping right first, not premature optimization" and flag the arena redesign as future work. I've encoded this: the agent preserves the cache-line constraints but doesn't refactor the AST toward arenas prematurely.

---

## Current focus

**Stage 2: Core works, untested.** The lexer and parser exist and compile. The binary runs. But there are zero tests of actual parsing behavior. The agent will prioritize:

1. Parser unit tests — does `fn add(a: i32, b: i32) -> i32 { a + b }` produce the right AST?
2. Lexer unit tests — does `0xFF` produce `LitInteger`? Does `'a` produce `Lifetime`?
3. Error case tests — what happens when the parser sees malformed input?

Only after those foundations are in place should the agent move to extending the grammar.

---

## What could be wrong

**Branch protection**: I couldn't verify whether `main` is protected on GitHub. If it's not, lathe's pushes could land directly on main without CI. Please check: Settings → Branches → Branch protection rules for `main`. Require PRs and status checks before allowing merges.

**The research questions**: I inferred William's goals from the README. If the research focus has shifted (e.g., you're now more interested in codegen than spec coverage), the agent's priorities should be updated.

**FLS coverage tracking**: There's no explicit tracking of which FLS sections are "done" vs. "stub." The architecture.md skill has a rough table, but it was written from reading the code — it may be incomplete. If you have a more authoritative list of what's implemented, I didn't find it.

**Unicode**: The lexer comment notes that non-ASCII identifiers are accepted but NFC normalization isn't applied. If this is a known gap you're tracking separately (e.g., in an issue), the agent won't know about it.
