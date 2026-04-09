# Claims Registry

Claims are the load-bearing promises galvanic makes to its stakeholders. The falsification suite (`falsify.sh`) checks these every cycle. A failing claim is top priority — fix it before any new work.

---

## Active Claims

### Claim 1: Build integrity

**Stakeholders:** All  
**Promise:** `cargo build` succeeds with no errors and no clippy warnings (`cargo clippy -- -D warnings`).  
**Why it's load-bearing:** A compiler that doesn't compile is not a compiler. Every stakeholder's trust starts here.  
**How it's checked:** `falsify.sh` runs `cargo build` and `cargo clippy -- -D warnings`.

---

### Claim 2: Test suite passes

**Stakeholders:** FLS conformance researcher, cache researcher, Sunday contributor  
**Promise:** `cargo test` exits 0 — all unit and integration tests pass.  
**Why it's load-bearing:** The 1700+ tests in `e2e.rs` represent 197 milestones of confirmed FLS compliance. A silent regression invalidates previous work.  
**How it's checked:** `falsify.sh` runs `cargo test`.

---

### Claim 3: Token stays 8 bytes

**Stakeholders:** Cache-aware codegen researcher  
**Promise:** `size_of::<Token>() == 8` — the lexer's hot-path type fits 8 tokens per 64-byte cache line.  
**Why it's load-bearing:** This is the primary structural claim of the cache-line-first design thesis. If `Token` grows, the thesis is no longer demonstrated at the lexer level.  
**How it's checked:** `falsify.sh` runs `cargo test --lib -- --exact lexer::tests::token_is_eight_bytes`.  
**Structural, not documentary:** This claim fails when the struct grows, regardless of what the doc comment says. A doc comment update does not satisfy this claim.

---

### Claim 4: No unsafe code in library source

**Stakeholders:** Sunday contributor, FLS conformance researcher  
**Promise:** No `unsafe` blocks, `unsafe fn`, or `unsafe impl` in `src/` (excluding `src/main.rs`).  
**Why it's load-bearing:** Galvanic is a research compiler implemented in safe Rust. If unsafe code creeps into the library, it undermines both the safety argument and the clean-room discipline.  
**How it's checked:** `falsify.sh` greps `src/` for `unsafe` keywords, excluding `src/main.rs` and comment lines.

---

### Claim 5: Runtime instruction emission (no const-fold in non-const functions)

**Stakeholders:** FLS conformance researcher  
**Promise:** A non-const function that evaluates `1 + 2` emits a runtime `add` instruction, not a folded `mov x0, #3`.  
**Why it's load-bearing:** FLS §6.1.2:37–45 is the heart of the conformance research question. A compiler that constant-folds non-const code looks correct on exit-code tests but is semantically wrong. The assembly inspection tests in `e2e.rs` are the only way to catch this.  
**How it's checked:** `falsify.sh` runs `cargo test --test e2e -- --exact runtime_add_emits_add_instruction`.  
**Note:** This single check is a proxy for the broader claim. The full defense is the set of assembly inspection tests throughout `tests/e2e.rs`. When adding new features that involve new arithmetic or comparison operations, extend the test suite with new inspection tests.

---

### Claim 6: CLI handles adversarial inputs without panicking

**Stakeholders:** CI/validation infrastructure, Sunday contributor  
**Promise:** The galvanic binary does not panic or crash (exit > 128) when given: empty files, binary garbage, NUL bytes, deeply nested braces (500 levels), or large inputs (10k let bindings).  
**Why it's load-bearing:** A compiler that panics on bad input is not a compiler. Contributors who encounter unexpected panics will leave. The fuzz-smoke CI job encodes this; this claim tracks whether that contract holds locally.  
**How it's checked:** `falsify.sh` constructs adversarial inputs and verifies clean exit (exit <= 128 or recognizable error exit).

---

### Claim 7: Block/paren disambiguation (FLS §6.21)

**Stakeholders:** FLS conformance researcher, CI/validation infrastructure  
**Promise:** A block-like expression (`for`, `while`, `loop`, `if`, `match`) followed by `(` is parsed as two separate expressions, not as a call. `for x in arr {} (s + 1)` emits an `add`, not a `blr` (call).  
**Why it's load-bearing:** This parser invariant was silently regressed in the re-init commits (9866afd). The compile_and_run tests self-skip on macOS, so the regression was invisible until a `compile_to_asm` inspection test caught it. Without this claim in `falsify.sh`, the same regression could recur silently in any future re-init or merge.  
**How it's checked:** `falsify.sh` runs `cargo test --test e2e -- --exact runtime_for_block_then_paren_emits_add_not_blr`. This test uses `compile_to_asm` (no ARM64 tools required) and asserts: `add` appears in output, `blr` does not, result is not constant-folded to `#4`.  
**Structural, not documentary:** This claim fails if the parser regresses to treating `(` after a block-like expression as a call postfix — regardless of what comments say.

---

## Retired Claims

*(None yet. Claims are retired here when they no longer fit the project, with the date and reasoning.)*

---

## Adding New Claims

When a new milestone introduces a new structural promise:

1. Add an entry here with: stakeholder, promise, why it's load-bearing, how it's checked, and whether it's structural (fails if code changes) or documentary (fails if docs change).
2. Add a corresponding check to `falsify.sh`.
3. Choose structural over documentary whenever possible. A claim that can be satisfied by editing comments is not a structural claim.
4. Keep the total number of claims in the 3–10 range. Too many claims that run slowly defeat the purpose. New claims should replace weaker ones where possible.
