# Claims Registry — galvanic

Load-bearing promises this project makes to its stakeholders. Each claim is checked every cycle by `falsify.sh`. A failing claim is top priority — fix it before any new work.

Claims have lifecycles. When a claim no longer fits the project (e.g., a struct is intentionally redesigned to use a different layout), retire it here with reasoning rather than silently softening the check.

---

## Claim 1: Token is 8 bytes

**Stakeholder:** Cache-line codegen researcher  
**Promise:** `size_of::<galvanic::lexer::Token>() == 8`

The entire cache-line rationale depends on this. 8 tokens fill one 64-byte cache line. If Token grows, the "N/8 cache-line loads for N tokens" claim in the module doc becomes false without any visible signal.

**Check:** `cargo test --lib -- --exact lexer::tests::token_is_eight_bytes`  
**Adversarial:** Any field added to `Token` (e.g., a `flags: u8` or a wider span) triggers failure.  
**Status:** Active

---

## Claim 2: Non-const functions emit runtime instructions

**Stakeholder:** FLS researcher, maintainer  
**Promise:** Compiling `fn f(a: i32, b: i32) -> i32 { a + b }` produces ARM64 assembly containing a runtime `add` instruction in `f`'s body.

This is the FLS §6.1.2:37–45 constraint: non-const functions execute at runtime. Galvanic is a compiler, not an interpreter. If the lowering pass constant-folds a parameter-based arithmetic expression, it violates the spec regardless of whether the output is numerically correct.

**Check:** Compile the function via `galvanic` binary, inspect `.s` output for `\tadd\t` or ` add `.  
**Adversarial:** Using function parameters (not literals) as operands — these cannot be constant-folded without violating the spec. The operands are not statically known to the compiler.  
**Status:** Active

---

## Claim 3: No unsafe code in library modules

**Stakeholder:** Safety researcher, anyone trusting galvanic is safe Rust  
**Promise:** No `unsafe { }`, `unsafe fn`, or `unsafe impl` appears in `src/` outside of `src/main.rs`.

The project's research value includes demonstrating that a non-trivial compiler can be written in safe Rust. The CI `audit` job enforces this; `falsify.sh` independently checks it each cycle.

**Check:** `grep -rn 'unsafe' src/` filtered to exclude `src/main.rs` and comment lines.  
**Adversarial:** Any unsafe block added for convenience (e.g., to avoid a bounds check) is a violation.  
**Status:** Active

---

## Claim 4: The library never shells out

**Stakeholder:** Library user (anyone using galvanic as a library crate)  
**Promise:** No `std::process::Command` or `process::Command` appears in `src/` outside of `src/main.rs`.

The compiler library is pure computation. Only the CLI driver (`main.rs`) invokes the assembler and linker. Library consumers must not observe side effects from importing galvanic.

**Check:** `grep -rn 'process::Command' src/` filtered to exclude `src/main.rs`.  
**Adversarial:** A lowering or codegen helper that calls an external tool for any reason.  
**Status:** Active

---

## Claim 5: galvanic exits cleanly on empty input

**Stakeholder:** CLI user  
**Promise:** `galvanic empty.rs` (where `empty.rs` is zero bytes) exits with code 0 and does not panic, hang, or die on a signal.

Empty input is a degenerate but valid case. A compiler that crashes on empty input is unreliable for any automation.

**Check:** Run the galvanic binary on a zero-byte `.rs` file; assert exit code is 0 and ≤ 128.  
**Adversarial:** The file exists but contains nothing — no tokens, no items.  
**Status:** Active

---

## Claim 6: galvanic exits non-zero (cleanly) on a missing file

**Stakeholder:** CLI user  
**Promise:** `galvanic /does/not/exist.rs` exits with a non-zero exit code ≤ 128 (not a signal/panic) and writes an error message to stderr.

A missing file should produce a clean error, not a panic or hang. Exit codes > 128 indicate death by signal.

**Check:** Run galvanic on a nonexistent path; assert exit code is > 0 and ≤ 128.  
**Adversarial:** A path that does not exist and has never existed.  
**Status:** Active

---

## Retired Claims

_(none yet — add here with date and reason when retiring)_

---

## Notes for the Runtime Agent

- **Extend** this file when a new milestone creates a new promise. If you implement `fn main() -> i32` returning a computed value, the promise "the returned value reaches the process exit code" is load-bearing for the e2e stakeholder — add it.
- **The adversarial input matters.** A claim about runtime codegen that only tests literal arithmetic is not adversarial. Parameters are adversarial. Nested calls are adversarial. 
- **The suite should stay fast.** Each claim check takes seconds. If a new claim requires a multi-second cargo compile, consider whether there's a more targeted check (a named unit test, a grep) that defends the same promise.
- **`Span` is 8 bytes** by design (`start: u32` + `len: u32`) but has no `size_of` assertion test yet. Adding `ast::tests::span_is_eight_bytes` (analogous to the token test) is a good early cycle.
