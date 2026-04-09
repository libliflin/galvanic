# Alignment Summary

**Read this in 30 seconds before starting cycles. It documents the choices made during init — gut-check them before trusting the agent.**

---

## Who This Serves

- **William (you)** — Building galvanic milestone by milestone through FLS sections. Each cycle should either advance FLS coverage, harden existing coverage with adversarial tests, or fix a broken claim/CI failure.
- **FLS/Ferrocene spec authors** — Galvanic surfaces spec ambiguities. They benefit when galvanic notes `FLS §X.Y AMBIGUOUS:` in source comments. They're not active users but they're a real stakeholder of the research output.
- **Compiler/systems researchers** — Reading galvanic to understand what cache-line-aware codegen looks like from first principles. They trust the cache-line notes; they leave if those notes become aspirational rather than accurate.
- **Future contributors** — Reading the codebase to understand it or extend it. They need FLS citations and doc comments to understand the "why."

---

## Key Tensions

**Milestone velocity vs. hardening**: At milestone 87+, the temptation is to keep adding FLS sections. But many existing sections have only happy-path tests. The agent is configured to occasionally favor hardening — building adversarial fixtures for existing features. **You can override this** by noting in the snapshot "priority: milestone velocity" or "priority: harden §X.Y."

**Cache-line discipline vs. implementation speed**: Every new IR type needs a cache-line note. The agent is instructed to always add the note. If cycles feel slow because of this, it's intentional — the notes are the research output.

---

## Load-Bearing Claims

These are what `falsify.sh` defends every cycle. If any fails, the agent stops new work and fixes it.

1. **CLAIM-1**: `Token` is exactly 8 bytes — verified by `cargo test --lib -- lexer::tests::token_is_eight_bytes`. If Token grows, the stated 8-tokens-per-cache-line property breaks.

2. **CLAIM-2**: No `unsafe` in library source (`src/` minus `main.rs`). Structural constraint. Currently passing.

3. **CLAIM-3**: IR cache-line discipline — at least 40 `Cache-line note:` occurrences in `ir.rs`, and the reference types (StaticValue, StaticData, VtableShim, VtableSpec, IrBinOp) still have their notes. **Known gap**: Several top-level types (`Module`, `IrFn`, `Instr`, `IrValue`, `IrTy`, `FCmpOp`, `F64BinOp`, `F32BinOp`, `ClosureTrampoline`) currently lack type-level cache-line notes. The agent should add these.

4. **CLAIM-4**: FLS citations (`FLS §`) present in all five core source files. Currently passing.

5. **CLAIM-5**: No orphaned `.s` fixture files (every `.s` has a matching `.rs`). Currently passing.

6. **CLAIM-6**: Binary exits ≤ 128 on adversarial inputs (no signal death). Verified by running the debug binary against 6 constructed adversarial inputs. Requires `cargo build` to be run first.

---

## Current Focus

The project is at milestone 87+ with significant FLS coverage. CI is comprehensive (build, test, clippy, fuzz-smoke, audit, e2e, bench). The most valuable near-term work is probably:

1. Adding type-level cache-line notes to `Module`, `IrFn`, `Instr`, `IrValue`, `IrTy`, `FCmpOp`, `F64BinOp`, `F32BinOp`, `ClosureTrampoline` (closes the CLAIM-3 known gap)
2. Adversarial testing of features that have only happy-path e2e fixtures
3. Continuing FLS milestone coverage when the above is in good shape

---

## What Could Be Wrong

- **Stakeholder I may have missed**: The README says "nobody needs to use this" — but I named FLS authors as a stakeholder based on the research framing. If William doesn't think of the FLS team as a real audience, CLAIM-4 (citation discipline) and the `FLS §X.Y AMBIGUOUS` annotations might feel over-engineered. Check whether the FLS-research framing matches your actual intent.

- **CLAIM-3 threshold**: The minimum of 40 cache-line notes is set conservatively (ir.rs has ~82). If the IR grows rapidly with many new types added without notes, the count could stagnate while `pub` types increase. The agent should periodically check the ratio of pub types to cache-line notes, not just the absolute count.

- **E2e test coverage**: I couldn't read `tests/e2e.rs` fully (file too large). The claims assume e2e coverage is in good shape. If there are e2e tests that always skip or are marked `#[ignore]`, that's a gap I didn't catch.

- **`Token` size test**: CLAIM-1 assumes `lexer::tests::token_is_eight_bytes` exists. If this test was renamed or moved, `falsify.sh` will report it as missing. Check the test name in `src/lexer.rs` if CLAIM-1 fails unexpectedly.

- **`falsify.sh` executability**: The file was written but `chmod +x` requires shell approval in this environment. Run `chmod +x .lathe/falsify.sh` manually before starting cycles.

- **CI timing**: The e2e job has a 20-minute timeout and requires QEMU + cross toolchain. If milestones generate very large assembly files, e2e may approach the limit.

---

## Repository Security

- **Branch protection**: Unknown — not checked during init. Recommend requiring PR reviews on `main` and restricting direct push.
- **Actions triggers**: CI uses `pull_request` (not `pull_request_target`), with `permissions: contents: read`. Low injection risk.
- **Repo visibility**: Public. The engine only fetches structured data (statuses, numbers) — not free-text comments. Risk is low but non-zero.
- **Recommendation**: Enable branch protection on `main` before running many autonomous cycles.
