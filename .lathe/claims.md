# Load-Bearing Claims

These are the promises galvanic makes to its stakeholders. Each one, if broken,
would undermine the project's research value. The falsification suite in
`falsify.sh` tests these every cycle.

The runtime agent extends this list when new features create new promises.
Do not remove claims — if a claim is no longer testable, document why.

---

## Claim 1: Compiled programs generate runtime code, not compile-time constants

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: Galvanic is a compiler, not an interpreter. When it compiles
`fn main() -> i32 { 1 + 2 }`, the emitted assembly must contain an `add`
instruction — not `mov x0, #3`. The fact that inputs are statically known
does not justify constant folding for non-const code (FLS §6.1.2:37–45).

**Violated if**: `compile_to_asm("fn main() -> i32 { 1 + 2 }\n")` returns
assembly that contains `mov     x0, #3` instead of an `add` instruction.

**Test**: `cargo test --test e2e -- runtime_add_emits_add_instruction`

---

## Claim 2: Token is exactly 8 bytes

**Stakeholder**: Compiler Researchers, William

**Promise**: The `Token` struct is 8 bytes, enabling 8 tokens per 64-byte cache
line. Every cache-line comment in `lexer.rs` (and the design rationale in the
README) depends on this. A silent growth to 16 bytes invalidates the research
premise.

**Violated if**: `std::mem::size_of::<Token>() != 8`

**Test**: `cargo test --lib -- lexer::tests::token_is_eight_bytes`

---

## Claim 3: No unsafe code in library source

**Stakeholder**: Compiler Researchers, CI / Validation

**Promise**: Galvanic is written in safe Rust. `unsafe` blocks, `unsafe fn`,
and `unsafe impl` are forbidden in `src/` except `src/main.rs` (which shuts
out to the assembler/linker). This is verified by both CI `audit` job and the
falsification suite.

**Violated if**: `grep -rn 'unsafe\s*{' src/` finds matches outside `main.rs`,
or `grep -rn 'unsafe\s*fn\b' src/` finds matches outside `main.rs`.

**Test**: direct grep in falsify.sh

---

## Claim 4: The pipeline accepts valid milestone programs without panicking

**Stakeholder**: William, CI / Validation

**Promise**: Galvanic either succeeds or exits cleanly with a diagnostic. It
never panics (exits with a signal) on valid Rust programs. `fn main() {}` must
exit 0. `fn main() -> i32 { 42 }` must exit 0 (the galvanic process, not the
compiled program).

**Violated if**: Running `galvanic` on `fn main() {}` exits with code > 128
(indicating death by signal, i.e., a panic).

**Test**: runs galvanic binary on minimal inputs in falsify.sh

---

## Claim 5: FLS citations in source are structurally valid

**Stakeholder**: FLS / Ferrocene Ecosystem, Compiler Researchers

**Promise**: `FLS §X.Y` references in source code use real section numbers
from the specification table of contents. A citation that refers to a
nonexistent section is worse than no citation — it creates misleading
documentation.

**Current status**: This claim is difficult to falsify automatically without
fetching the spec (network dependency). It is enforced by code review and the
documented FLS TOC in `refs/fls-pointer.md`. The runtime agent is responsible
for checking citations against the TOC when adding or modifying code.

**Test**: Not currently automated. The runtime agent must verify manually.
When a mechanism for automated checking is possible (e.g., a local copy of
the spec's section list), add it here.

---

## Claim 6: Function calls with literal arguments emit branch instructions, not folded constants

**Stakeholder**: William (researcher), Compiler Researchers

**Promise**: Galvanic does not inline functions and constant-fold their results. When
a function is called with statically-known literal arguments, the call must emit `bl`
at the call site and compute the result at runtime. `square(6)` must emit `bl square`
and NOT `mov x0, #36`. This is a specific attack on Claim 1: a compiler could pass
the `1 + 2 → add` check while still folding function calls with known inputs.

**Violated if**: `compile_to_asm("fn square(x: i32) -> i32 { x * x }\nfn main() -> i32 { square(6) }\n")`
returns assembly containing `mov x0, #36` instead of `bl square`.

**Test**: `cargo test --test e2e -- runtime_fn_call_result_not_folded`

---

## Not Yet Claims (honest gaps)

These are promises the project will eventually make but cannot yet be falsified:

- **Correct runtime behavior for function parameters**: `fn f(x: i32) -> i32 { x + 1 }` called with x=5 must exit 6. This requires the cross toolchain and QEMU to run, which falsify.sh cannot use. It is covered by `cargo test --test e2e` on CI.

- **Arithmetic overflow behavior**: In non-const code at runtime, integer overflow must panic in debug mode and wrap in release mode (FLS §6.1.2:49–50). Galvanic does not yet enforce this — no bounds checking is emitted. When it does, add a claim here.

- **Unicode identifier handling**: FLS §2.3 requires NFC normalization for Unicode identifiers. Galvanic accepts non-ASCII identifiers but does not normalize them. This is a known gap, documented in `lexer.rs`.
