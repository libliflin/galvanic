# Alignment Summary

Read this before starting cycles. It takes 30 seconds and will save you from cycles that don't matter.

---

## Who this serves

- **William (you)** — Researcher and sole maintainer. Wants the compiler pipeline to actually exist so the two research questions (FLS implementability, cache-line-aware codegen) can be answered. Cycles that advance the pipeline are valuable. Cycles that polish the stub are not.
- **Compiler/Rust researchers** — Will read the code and changelogs as research artifacts. They need FLS citations and ambiguity notes, not just working code.
- **Spec ambiguity hunters** — Want to see where the FLS breaks down under implementation pressure. Every ambiguity found is a result.

---

## Key tensions

**Forward progress vs. FLS fidelity**: The research value requires strict FLS adherence, not just any implementation. The agent is biased toward strict FLS fidelity and explicit ambiguity documentation. Speed doesn't matter; accuracy does.

**Cache-line awareness vs. pragmatism**: Cache-line awareness should shape data structure design from the start (token layout, AST node layout), but should never delay getting a phase working. Design thoughtfully, then implement.

**Exploratory research vs. clean code**: This is a research project. Comments explaining *why* (with FLS citations) are more valuable than elegant abstractions that hide the spec-to-code mapping.

---

## Current focus

The binary is a stub. The agent's next several cycles should be entirely focused on building the lexer — the first real phase of the compiler. Everything else (parser, AST, codegen) is downstream.

The agent should:
1. Read FLS §3 (lexical structure) before writing any token code.
2. Implement a `Token` type and a `tokenize()` function.
3. Test it against real Rust source text (at minimum: keywords, identifiers, integer literals, operators).
4. Document any FLS ambiguities found.

---

## What could be wrong

- **Stakeholders I might have missed**: This is a solo research project with a public repo. If galvanic ever gets cited or forked by other researchers, they become a stakeholder the agent isn't currently optimizing for. The README sets expectations correctly ("do not use this"), so this risk is low but worth noting.
- **FLS version**: The FLS at spec.ferrocene.dev is the assumed reference. If you're working against a specific version of the FLS, add that version number to `.lathe/refs/fls-pointer.md` so the agent knows which spec to follow.
- **ARM64 host assumption**: The agent will implement ARM64 codegen. If you're running tests on an x86 machine, integration tests for codegen output will need a cross-compilation or emulation setup. The agent doesn't know about this yet — you'll need to add guidance when you get to Phase 5.
- **`no_std` scope**: The README says "core Rust (no_std)". I've interpreted this as: the compiled programs are `no_std`, but the compiler binary itself uses `std`. If you mean the compiler binary should also be `no_std`, the agent will need to know that — it changes the available tools significantly.
- **CI is ubuntu-latest**: The CI runs on Linux. If ARM64-specific tests are needed (e.g., running compiled output), the CI will need a runner change (or QEMU). The agent should flag this when it becomes relevant.
