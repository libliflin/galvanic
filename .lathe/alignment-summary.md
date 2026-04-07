# Alignment Summary

Read this before starting cycles. It summarizes the alignment decisions made
during init and what you should verify before trusting the agent.

---

## Who This Serves

- **William (you)** — researching FLS implementability and cache-line codegen. Success = finding spec ambiguities and advancing FLS coverage with correct runtime codegen.
- **FLS/Ferrocene ecosystem** — benefits when galvanic finds real spec ambiguities, documented as `// FLS §X.Y: AMBIGUOUS — ...` in source.
- **Compiler researchers** — reading galvanic as a reference for spec-driven compiler design. Success = coherent FLS traceability and documented cache-line rationale.

---

## Key Tensions (and how I resolved them)

1. **New milestones vs. stress-testing existing ones** — I favored stress-testing. The agent will prioritize adding assembly inspection tests to milestones that only have exit-code tests before adding new FLS sections. Change this if you want aggressive forward progress over depth.

2. **Cache-line purity vs. pragmatic implementation** — I favored documentation over enforcement. New structs must have a cache-line comment explaining the layout decision, but there is no automated size test beyond Token. If you want to add size tests for other structs, add them to claims.md.

3. **FLS faithfulness vs. making tests pass** — I favored faithfulness. The agent is instructed to document every ambiguity rather than silently resolving it.

---

## Load-Bearing Claims (what falsify.sh defends every cycle)

1. **Runtime codegen, not interpretation** — `1 + 2` must emit an `add` instruction, NOT `mov x0, #3`. This is the fundamental correctness claim. `runtime_add_emits_add_instruction` in e2e.rs.

2. **Token is 8 bytes** — the cache-line layout rationale in lexer.rs is only valid if Token stays at 8 bytes. `token_is_eight_bytes` test.

3. **No unsafe in library source** — safe Rust only, outside main.rs. Enforced by CI audit job and falsify.sh grep.

4. **Pipeline doesn't panic on valid programs** — graceful errors, not signals/panics.

---

## Current Focus

The agent will prioritize (in order):

1. Fix any falsification failures
2. Fix any CI failures
3. Add assembly inspection tests to milestones that only have exit-code tests (especially recent milestones: closures §6.14/§6.22, default trait methods §10.1.1/§13, associated constants §10.3/§11)
4. Advance to the next FLS section

---

## What Could Be Wrong

- **Branch protection unknown.** I could not determine whether the main branch requires PR reviews or restricts direct push. Before running cycles, check GitHub → Settings → Branches and confirm protection is enabled. Without it, an agent that directly pushes to main bypasses CI.

- **FLS citation accuracy is not automatically tested.** Claim 5 in claims.md is manual-only. The agent is instructed to check citations against `refs/fls-pointer.md`, but there is no automated guard. If citation accuracy matters to you, consider building a local spec TOC checker.

- **Assembly inspection coverage is unknown.** I can see that assembly inspection tests exist for arithmetic operations (add, sub, mul) and some control flow (if/cbz, while). I do not know if closures, default trait methods, and associated constants — the three most recent milestones — have both positive and negative assembly inspection assertions. The agent will audit this in early cycles.

- **The "not an interpreter" check covers addition only.** falsify.sh tests `runtime_add_emits_add_instruction`. If closures are implemented as an interpreter but arithmetic is compiled correctly, falsify.sh will pass. The agent should extend falsify.sh when new features are added.

- **falsify.sh needs `chmod +x` before the engine can run it.** This could not be executed during init (sandbox restriction). Run: `chmod +x .lathe/falsify.sh` before starting cycles.

- **e2e tests skip on macOS without cross tools.** `compile_and_run` tests return early if aarch64-linux-gnu-as/ld/qemu are unavailable. The `compile_to_asm` tests (assembly inspection) always run. The agent's local validation on macOS only catches codegen correctness, not runtime correctness.

---

## Before Starting Cycles

```bash
# Make falsify.sh executable
chmod +x .lathe/falsify.sh

# Verify it runs and exits 0
./.lathe/falsify.sh
```

If it exits non-zero, the first cycle will fix the failing claim.
