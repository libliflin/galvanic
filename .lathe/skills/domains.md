# Domain Map — Galvanic

Galvanic operates across several distinct knowledge domains. Each has its own authority. A bug that looks like a spec gap might actually be a platform ABI difference. A decision that looks like implementation freedom might be constrained by the FLS. This file maps the domains so the builder and verifier attribute problems to the right authority and propose fixes in the right layer.

---

## Domain 1: Ferrocene Language Specification (FLS)

**What it covers:** Language semantics — expression evaluation, type system, pattern matching, function calling, lifetimes, const contexts, operator behavior. Everything about *what Rust means*.

**Authoritative source:** `https://rust-lang.github.io/fls/` (section-stable URLs). Table of contents in `.lathe/refs/fls-pointer.md`.

**In this project:** Every implementation decision in `src/lexer.rs`, `src/parser.rs`, `src/lower.rs`, `src/codegen.rs` should cite the FLS section it implements. When the spec is ambiguous or silent, the decision is documented in `refs/fls-ambiguities.md`.

**Boundary with Platform ABI domain:** The FLS describes *what* calling conventions must achieve (pass arguments, return values) but does not specify ARM64 register assignments. That's the ABI domain.

**Boundary with Rust compiler conventions domain:** Galvanic does NOT consult rustc internals. When the FLS and rustc behavior diverge, galvanic follows the FLS. The research value depends on this independence.

**Common confusion:** "How does rustc handle X?" is the wrong question. "What does the FLS say about X?" is the right one. If the FLS doesn't say, document the gap — that's the output.

---

## Domain 2: ARM64 Architecture

**What it covers:** Instruction encoding, register names and roles (x0–x30, sp, lr, xzr), addressing modes, branching, calling convention at the instruction level (which registers hold args, which hold return values, which are caller-saved).

**Authoritative source:** ARM Architecture Reference Manual (ARMv8-A). Key practical reference: `refs/arm64-abi.md` (galvanic's own summary of what it needs).

**In this project:** `src/codegen.rs` is the only file that reasons about ARM64 instructions. It emits GNU assembler syntax.

**Boundary with FLS domain:** FLS says "this expression returns a value." ARM64 says "return values go in x0 for integers, x0/x1 for 64-bit pairs." The boundary is the calling convention — the FLS says *what* must happen, ARM64 says *how*.

**Boundary with Platform ABI domain:** ARM64 registers are the same on all platforms; syscall numbers and binary format differ. A bug in which register holds a return value is an ARM64 domain bug. A bug in the `_start` entry point vs. `main` calling convention is a Platform ABI bug.

---

## Domain 3: Platform ABI

**What it covers:** How the OS interacts with the binary. Entry point (`_start` for Linux ELF, different for macOS Mach-O). Syscall instruction (`svc #0` on Linux, same on macOS but different numbers). Binary format (ELF for Linux/BSDs, Mach-O for macOS). Syscall numbers (each OS has its own table).

**Authoritative source:** `refs/arm64-platform-abi.md` — galvanic's own comparison table across macOS, Linux, and BSD. The Linux syscall table (kernel headers or `man 2 syscall`).

**In this project:** Galvanic currently emits Linux ARM64 ELF. The e2e tests run via `qemu-aarch64`. Platform differences are documented in `refs/arm64-platform-abi.md` for future work.

**Common confusion:** A failing e2e test may be a Platform ABI bug (wrong entry point, wrong syscall number) rather than a codegen bug. Check the ELF format and syscall choices before assuming the instruction encoding is wrong.

---

## Domain 4: Rust Compiler Conventions (Excluded by Design)

**What it covers:** What rustc does internally — its IR, its optimization passes, its actual ABI choices for edge cases the FLS doesn't specify.

**Authoritative source:** rustc source code, compiler-internal documentation.

**In this project:** **Explicitly excluded.** Galvanic is a clean-room implementation. Looking at rustc internals to resolve an FLS ambiguity would corrupt the research output. When the FLS is silent, galvanic makes a reasoned choice and documents it in `refs/fls-ambiguities.md`. That documented choice — not rustc's actual behavior — is the artifact.

**Common confusion:** "This is how rustc does it" is not a valid justification for a galvanic implementation choice. "This is what the FLS says" is. If the FLS doesn't say, document the ambiguity.

---

## Domain 5: Cache-Line Optimization Theory

**What it covers:** How cache lines work (64 bytes on ARM64), data structure layout for cache efficiency, instruction stream density (fewer bytes = more instructions per cache line = better throughput for instruction-fetch-bound code).

**Authoritative source:** ARM Architecture Reference Manual (cache behavior), empirical measurement. The design thesis is stated in `README.md`.

**In this project:** Every data structure in `src/ir.rs` and `src/lexer.rs` carries a cache-line note. The `Token` type is enforced to be 8 bytes. Cache-line awareness shapes IR node design, not as a post-hoc optimization but as a constraint considered during initial design.

**Boundary with ARM64 domain:** ARM64 specifies cache line size (64 bytes). Cache-line optimization theory says how to use that fact. The domains are adjacent — a cache-line note in the code cites the ARM64 cache line size and then explains the layout decision.

**Common confusion:** Cache-line awareness is a *design constraint*, not an optimization pass. Adding a cache-line pass that runs after IR construction misses the point. The question to ask when adding an IR node: "Does this fit in a cache line? What are the tradeoffs if it doesn't?"

---

## Where Domains Create Confusion

| Symptom | Likely domain | Wrong attribution |
|---|---|---|
| Emitted assembly has wrong register | ARM64 ABI | FLS ambiguity |
| Binary doesn't run on Linux but codegen looks right | Platform ABI (entry point, syscall) | ARM64 instruction encoding |
| FLS says X but galvanic does Y | FLS implementation gap | Platform constraint |
| "rustc does it differently" | Not galvanic's concern | ARM64 or FLS |
| Token size grew past 8 bytes | Cache-line domain | ARM64 instruction set |
| Test fails on CI but passes locally | Platform ABI (Linux vs. macOS) | Codegen bug |

When debugging, name which domain owns the bug before proposing a fix. A fix in the wrong domain (adding an FLS citation to a syscall number bug) will not resolve the problem.
