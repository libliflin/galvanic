# Alignment Summary

*Read this in 30 seconds and gut-check before starting cycles.*

---

## Who This Serves

- **William as FLS conformance researcher** — testing whether the Ferrocene Language Specification is independently implementable, milestone by milestone, with every decision traceable to a `FLS §X.Y` citation and no rustc-internal knowledge.
- **William as cache-aware codegen researcher** — testing what a compiler looks like when cache-line alignment is a first-class structural constraint from the start, not an optimization pass at the end.
- **The Sunday contributor** — someone who finds this interesting and wants to add a FLS section; needs clear testing patterns and a clear map of what's done.
- **CI/validation infrastructure** — the trust substrate; galvanic has solid CI (build, clippy, fuzz-smoke, audit, e2e with QEMU, bench). The lathe's work is only as trustworthy as the CI that validates it.

---

## Key Tensions

- **Parser coverage vs. codegen coverage:** The parser is far ahead of the codegen. `fls_fixtures.rs` covers many FLS constructs that `e2e.rs` doesn't yet compile. The agent favors codegen progress over expanding parse acceptance — confirmed by the project's milestone-based trajectory.

- **FLS fidelity vs. convenience:** The whole value of the project is the discipline (spec-only, no rustc cheating). The agent should never take a convenient path that papers over a spec ambiguity. Fidelity wins.

- **Cache-line documentation vs. enforcement:** There are more cache-line doc comments than size assertions. The agent should add `size_of` assertions for types with explicit budgets, but should not add claims it can't actually enforce. The `Token` size claim is structural and enforced; the IR instruction notes are currently aspirational and documented as such.

---

## Load-Bearing Claims

These are the promises encoded in `.lathe/claims.md` and checked every cycle by `falsify.sh`:

1. **Build integrity** — `cargo build` and `cargo clippy -- -D warnings` succeed.
2. **Test suite passes** — `cargo test` exits 0.
3. **Token is 8 bytes** — `size_of::<Token>() == 8`, enforced via `lexer::tests::token_is_eight_bytes`.
4. **No unsafe in library source** — grep check on `src/` excluding `main.rs`.
5. **Runtime instruction emission** — `fn main() -> i32 { 1 + 2 }` emits a runtime `add`, not `mov x0, #3` (FLS §6.1.2:37–45 compliance proxy).
6. **CLI handles adversarial inputs** — empty files, binary garbage, NUL bytes, deeply nested braces don't crash the binary (exit > 128).

---

## Current Focus

Galvanic is at milestone 197 (`for x in &mut slice`). The project is advancing through FLS §6.15.1 (for loops with various iterator patterns) and §4.9 (slice/reference types). The agent should continue at this frontier — implementing the next uncovered FLS section that the parser already handles but the codegen doesn't yet fully support.

The assembly inspection tests lag the milestone tests. When adding new milestones, always add both exit-code tests AND assembly inspection tests for any new runtime instruction patterns.

---

## What Could Be Wrong

**Falsify.sh not verified to run to completion.** The init sandbox could not execute `cargo` or `bash` commands, so `falsify.sh` was not run to confirm the summary line appears. Before starting cycles, you should run:

```bash
cd /Users/williamlaffin/code/galvanic
chmod +x .lathe/falsify.sh
bash .lathe/falsify.sh
```

Confirm the output ends with `=== Summary === passed: N  failed: M`. If it dies silently before that line, the most likely cause is a `grep` command returning exit code 1 (no matches) under `pipefail`. The script uses `|| true` guards on all grep invocations, but if bash is bailing earlier check for unbound variables or syntax errors.

**Claim 2 (test suite passes) is not in falsify.sh.** Running the full `cargo test` suite every cycle would take 2-5 minutes and is already done by snapshot.sh. The falsify.sh omits it and relies on snapshot output + CI for full test coverage. If you want it in the falsification loop, add a targeted run like `cargo test --lib` (fast) rather than the full suite.

**Branch protection not verified.** I couldn't check whether the default branch on GitHub has protection rules (require PR reviews, restrict direct push). For autonomous operation, branch protection on `main` is important. Check repo settings and enable "Require a pull request before merging" if not already set.

**Repo visibility not confirmed.** The README doesn't state whether the repo is public or private. If it's public (`libliflin/galvanic`), external contributors can file issues and PRs with injected text. The lathe engine only consumes structured data from GitHub (status codes, PR numbers), not free-text fields, so injection risk is low — but worth knowing.

**The e2e test (Claim 5) requires the build to succeed first.** `cargo test --test e2e -- --exact runtime_add_emits_add_instruction` will fail if `cargo build` failed first. The falsify.sh runs build first and stops if it fails, so this ordering is correct — but note that the $FAIL count could cascade if build fails.

**Milestones 102, 107, 128, 141, 146, 158, 171-173, 175-179, 191 are skipped.** The e2e.rs milestone numbering has gaps. This is normal — milestone numbers were retired or reassigned. It's not a bug to investigate.

**The "Sunday contributor" claim is aspirational.** I couldn't find a `CONTRIBUTING.md` or a clear "what's next" document. The Sunday contributor's journey depends on being able to find the next uncovered FLS section. The agent could improve this by adding a brief "where we are / what's next" section to the README, but that's a future cycle.
