# Claims Registry

Load-bearing promises galvanic makes to its stakeholders. Each claim is a specific promise that, if violated, would break the research or erode trust. The falsification suite in `falsify.sh` enforces these every cycle.

Claims have lifecycles — retire them here with reasoning when they no longer fit, rather than softening the check.

---

## C1: Token fits a cache-line slot

**Stakeholder:** Cache-line codegen researcher  
**Claim:** `size_of::<Token>() == 8`  
**Why load-bearing:** Token is the hot type in the lexer's iteration loop. The entire cache-line argument for the lexer ("8 tokens per cache line, ~4× better than a naive 32-byte token") depends on Token being exactly 8 bytes. If Token grows, the claim becomes aspirational prose.  
**Check:** `cargo test --lib -- --exact lexer::tests::token_is_eight_bytes`

---

## C2: Span fits a cache-line slot

**Stakeholder:** Cache-line codegen researcher  
**Claim:** `size_of::<Span>() == 8`  
**Why load-bearing:** Span is carried alongside Token in the parser's hot path. The 8-byte budget is documented in `ast.rs` and is the one AST layout property currently enforced.  
**Check:** Structural assertion in the test suite (`lexer::tests::span_is_eight_bytes` if it exists; otherwise via a direct `assert_eq!` in falsify.sh using a test binary).

---

## C3: The build succeeds

**Stakeholder:** All — contributors, spec investigator, CI  
**Claim:** `cargo build` exits 0 with no errors  
**Why load-bearing:** A project that doesn't compile is not usable by any stakeholder.  
**Check:** `cargo build`

---

## C4: The test suite passes

**Stakeholder:** FLS contributor, spec investigator  
**Claim:** `cargo test` exits 0  
**Why load-bearing:** The test suite is the contributor's safety net and the spec investigator's regression guard. A failing test is a broken promise to a specific stakeholder.  
**Check:** `cargo test`

---

## C5: No unsafe code in library

**Stakeholder:** Spec investigator  
**Claim:** No `unsafe` blocks, `unsafe fn`, or `unsafe impl` appear in `src/` excluding `src/main.rs`  
**Why load-bearing:** Galvanic implements the FLS in safe Rust. Adding unsafe to library code would mean relying on invariants the FLS doesn't guarantee — contaminating the research.  
**Check:** `grep -rn 'unsafe\s*{\|unsafe\s*fn\b\|unsafe\s*impl\b' src/ | grep -v '^src/main\.rs:'` returns empty

---

## C6: Full pipeline works on the milestone_1 program

**Stakeholder:** Spec investigator, cache-line codegen researcher  
**Claim:** `galvanic tests/fixtures/milestone_1.rs` (the minimal `fn main() -> i32 { 0 }`) exits 0 and emits a `.s` file without error  
**Why load-bearing:** This is the minimal end-to-end proof that lex → parse → lower → codegen works at all. If this breaks, nothing above it is trustworthy.  
**Check:** Build the binary, run it against `tests/fixtures/milestone_1.rs`, verify exit 0 and `.s` file creation.

---

## C7: FLS citations are present in source modules

**Stakeholder:** Spec investigator  
**Claim:** Every `src/` module that implements FLS behavior contains at least one `FLS §` citation in its source.  
**Why load-bearing:** The research depends on traceability. A module that implements parser rules but has no FLS citations is untraceable — the spec investigator can't verify correctness, can't find ambiguities, can't cite the code in research notes.  
**Check:** For each of `src/lexer.rs`, `src/parser.rs`, `src/ir.rs`, `src/lower.rs`, `src/codegen.rs`: `grep -c 'FLS §'` returns > 0.
