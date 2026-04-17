# Domain Map — Galvanic

Galvanic spans four distinct domains of authority. A bug that looks like one domain's problem often lives in another. This map answers "who to ask about what" and marks where domain boundaries create confusion.

---

## Domain 1: The Ferrocene Language Specification (FLS)

**Covers:** What Rust programs mean. What expressions are valid, what type rules apply, what evaluation order is specified, what behavior is undefined, what the compiler must and must not do. Const contexts, ownership rules, trait semantics, pattern matching, generics.

**Authoritative source:** `https://rust-lang.github.io/fls/` (versioned; was previously at `spec.ferrocene.dev`). See `.lathe/refs/fls-pointer.md` for the current table of contents.

**Project artifacts:**
- `refs/fls-constraints.md` — what galvanic must not do (constraints from the spec)
- `refs/fls-ambiguities.md` — where the spec is silent or ambiguous, and what galvanic chose
- `AMBIGUOUS: §N.M — ...` annotations throughout `src/`
- `FLS §N.M: ...` citations throughout `src/`

**Boundary confusion with IR design:** The FLS defines what code means; the IR defines how galvanic represents it internally. When a lowering decision feels wrong, ask: "Is the FLS actually prescribing this, or am I choosing an IR representation that could have been different?" The FLS doesn't dictate IR shape — that's galvanic's design choice. FLS citations in `src/lower.rs` mark where the spec drives the decision; unmarked choices are galvanic's own.

**Boundary confusion with ABI:** The FLS defines the Rust abstract machine but does not specify calling conventions, register layout, or binary format — those are the ABI's domain. When a function call produces the wrong result, check the ABI first (are arguments in the right registers?), not the FLS.

---

## Domain 2: ARM64 ISA (Instruction Set Architecture)

**Covers:** Which instructions exist, what they do, their encoding, their latency. `add`, `sub`, `mul`, `sdiv`, `mov`, `movz`, `movk`, `ldr`, `str`, `bl`, `ret`, `svc`, branch conditions, vector instructions.

**Authoritative source:** ARM Architecture Reference Manual (ARM DDI 0487). For galvanic's purposes, the GAS (GNU Assembler) mnemonics and syntax are sufficient — see any `aarch64-linux-gnu-as` documentation.

**Boundary confusion with ABI:** The ISA says what `bl` does (branch with link, sets `x30`). The ABI says what argument goes in `x0` before the call. When a function call is wrong, distinguish: is the wrong instruction being used (ISA problem), or are the arguments in the wrong registers (ABI problem)?

**Boundary confusion with FLS:** The ISA doesn't know about Rust types or const contexts. When galvanic emits `mov x0, #5` for `fn main() -> i32 { 2 + 3 }`, the ISA is fine with it — but the FLS constraint says `2 + 3` is not a const context, so the compiler must emit `mov x0, #2; mov x1, #3; add x0, x0, x1` instead. ISA correctness and FLS compliance are different questions.

---

## Domain 3: Platform ABI (Application Binary Interface)

**Covers:** Calling conventions (which registers hold arguments, return values, are callee-saved vs caller-saved), stack layout, binary format, syscall conventions. Platform-specific differences between macOS, Linux, and BSDs.

**Authoritative sources:**
- `refs/arm64-abi.md` — AAPCS64 (Procedure Call Standard for AArch64), shared by all platforms
- `refs/arm64-platform-abi.md` — platform-specific differences (syscall ABI, binary format, startup convention)

**Key facts:**
- Register conventions (x0–x7 for integer args, d0–d7 for float args, x0/d0 for return, x29/x30 for frame/link) are identical on all platforms (AAPCS64).
- Binary format and syscall convention differ:
  - Linux: ELF, `svc #0` with syscall number in `x8`
  - macOS: Mach-O, different syscall numbers (galvanic doesn't currently target macOS natively)
  - BSDs: ELF like Linux, different syscall numbers
- Galvanic emits Linux ELF. The output binary will not run on macOS even on Apple Silicon.

**Boundary confusion with FLS:** The ABI is not specified by the FLS — the FLS defines the Rust abstract machine, not the binary representation. Calling conventions are an ABI concern, not a spec concern. When galvanic's codegen puts arguments in the wrong registers, that's an ABI bug, not an FLS compliance bug.

**Boundary confusion with ISA:** The ISA says `x0`–`x30` are general-purpose registers. The ABI says `x0` holds the first integer argument and return value, `x29` is the frame pointer, `x30` is the link register. When a function returns the wrong value, check whether the ABI convention is being followed before checking the instruction encoding.

---

## Domain 4: Safe Rust (the implementation language)

**Covers:** The Rust language galvanic is written in. Ownership, borrowing, lifetimes, error handling (`Result`, `?`), iterators, string formatting, standard library types.

**Authoritative source:** The Rust Reference (`https://doc.rust-lang.org/reference/`) and `rustc` documentation.

**Important distinction:** Galvanic is written in Rust, but the Rust being *compiled* by galvanic is constrained by the FLS, not by the full Rust reference. The FLS covers a subset of Rust (including `no_std`). When galvanic needs to *implement* something (e.g., a `HashMap` for the symbol table), it uses whatever Rust feature works — including `std`. When galvanic needs to *compile* something, it's bound by the FLS subset.

**Boundary confusion:** A contributor might try to add support for a Rust feature that galvanic uses in its own implementation but has not yet implemented for compilation. These are separate: "galvanic uses `HashMap`" and "galvanic can compile programs that use `HashMap`" are independent.

---

## Where boundaries create confusion

| Symptom | Likely domain | Common mistake |
|---|---|---|
| Wrong exit code from compiled binary | ABI (wrong return register) | Blamed on FLS |
| Constant result emitted instead of runtime code | FLS §6.1.2 violation | Blamed on codegen (it's actually correct ISA, wrong semantics) |
| Function call passes wrong value | ABI (wrong argument register) | Blamed on ISA |
| Section number in citation doesn't exist | FLS version drift | Treated as annotation error |
| Binary runs on Linux but not on macOS | Platform ABI (ELF vs Mach-O) | Treated as ISA bug |
| `AMBIGUOUS` annotation with no matching ref entry | Process gap | Treated as code quality issue |
| Clippy warning about an implementation detail | Safe Rust domain | Confused with FLS compliance |
