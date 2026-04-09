# Claims Registry

This file records the load-bearing promises galvanic makes to its stakeholders. The falsification suite in `falsify.sh` defends these every cycle. A failing claim is top priority — fix it before any new work.

Claims have lifecycles. When a claim no longer fits the project, retire it here with reasoning rather than deleting it or softening the check.

---

## Active Claims

### CLAIM-1: Token is exactly 8 bytes
**Stakeholder**: Compiler/systems researchers; William  
**Promise**: The `Token` struct in `lexer.rs` is exactly 8 bytes. At 64 bytes per cache line, 8 tokens fit per line. This is the foundational cache-line invariant: the parser's hot iteration over `Vec<Token>` costs N/8 cache-line loads for N tokens, not N. If `Token` grows, this claim breaks and the stated 4× locality improvement disappears.  
**Check**: `cargo test --lib -- lexer::tests::token_is_eight_bytes`  
**Adversarial input**: Adding a new `TokenKind` variant or a field to `Token` that pushes it past 8 bytes. The check fails immediately.

---

### CLAIM-2: No unsafe code in library source
**Stakeholder**: Future contributors; William  
**Promise**: `src/` (excluding `src/main.rs`) contains no `unsafe` blocks, `unsafe fn`, or `unsafe impl`. This is an explicit design constraint: galvanic should be compilable and understandable without knowledge of unsafe Rust. The library implements a compiler in safe Rust. If unsafe appears, it signals either a design mistake or a missed abstraction.  
**Check**: `grep -rn 'unsafe' src/ | grep -v '^src/main\.rs:' | grep -Ev ':[0-9]+:[[:space:]]*//'` — must return empty  
**Adversarial input**: Any PR that adds unsafe to lib code. CI catches this; `falsify.sh` catches it locally.

---

### CLAIM-3: IR cache-line discipline is present and growing
**Stakeholder**: Compiler/systems researchers  
**Promise**: The research claim of galvanic is that cache-line awareness is woven throughout the design. The evidence of this is that `ir.rs` accumulates `Cache-line note:` comments as the IR grows, and the reference types that established the discipline still carry their notes. The check is two-tier: (A) ir.rs must have at least 40 cache-line note occurrences total; (B) the specific types that have always had notes (StaticValue, StaticData, VtableShim, VtableSpec, IrBinOp) must still have them.  
**Known gap at init**: Several top-level type declarations currently lack type-level cache-line notes: `Module`, `IrFn`, `ClosureTrampoline`, `Instr`, `IrValue`, `IrTy`, `FCmpOp`, `F64BinOp`, `F32BinOp`. The runtime agent should add these. When all top-level types have notes, the check in `falsify.sh` should be made stricter (check each declaration individually).  
**Adversarial input**: Adding many new IR types without any cache-line documentation, eroding the research artifact. Also: removing existing cache-line notes from the reference types.

---

### CLAIM-4: FLS citations present in core implementation modules
**Stakeholder**: FLS/Ferrocene spec authors; future contributors  
**Promise**: The implementing source files for galvanic's core pipeline — `lexer.rs`, `parser.rs`, `ir.rs`, `lower.rs`, `codegen.rs` — each contain at least one `FLS §` citation. This is the baseline citation discipline: the spec-testing instrument must cite the spec it tests.  
**Check**: Each of the five source files contains at least one occurrence of `FLS §`  
**Adversarial input**: Replacing or deleting FLS citations during refactoring. Also catches a new module that implements FLS behavior but has no citations.

---

### CLAIM-5: E2e fixture assembly files are not orphaned
**Stakeholder**: William  
**Promise**: Every `.s` assembly fixture in `tests/fixtures/` has a corresponding `.rs` source file. An orphaned `.s` file (no `.rs` counterpart) means the assembly was generated from a source file that no longer exists — possibly from a renamed or deleted test. This would silently let a fixture become stale or untestable.  
**Check**: For every `tests/fixtures/*.s`, there exists a `tests/fixtures/*.rs` with the same stem  
**Adversarial input**: Renaming a `.rs` fixture without renaming the `.s`, or deleting a `.rs` while leaving the `.s`.

---

### CLAIM-6: Binary exits cleanly on adversarial input (no signal death)
**Stakeholder**: William; future contributors  
**Promise**: The galvanic binary exits with code ≤ 128 on malformed, empty, or adversarial input. It should never die on a signal (SIGSEGV, stack overflow, OOM). A panic in production signals a missing input-validation path that could corrupt the research workflow.  
**Check**: Feed several adversarial inputs (empty file, syntax garbage, deeply-nested braces, very long line) to the debug binary and verify exit code ≤ 128  
**Adversarial input**: Programs that break assumptions in the parser, lexer, or lowering phase — not just valid-but-unsupported programs.

---

## Retired Claims

*(None yet. When a claim is retired, move it here with a date and reason.)*

---

## Adding New Claims

When a new feature creates a new promise to a stakeholder, add it here and add a case to `falsify.sh`. Keep claims to the most load-bearing ones — 3–8 at any given time. A claim that duplicates what CI already checks is not wrong, but a claim that CI misses is more valuable.
