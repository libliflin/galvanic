# Domain Boundaries

Galvanic spans four domains of knowledge. A bug that looks like it belongs in one domain often traces back to a different one. This file maps what each domain covers, what its authoritative source is, and where the boundaries between domains create confusion.

---

## Domain 1: The Ferrocene Language Specification (FLS)

**What it covers.** The semantics of the Rust language: what programs mean, what values they produce, what's undefined behavior, what the type system enforces. The FLS is the source of truth for *what galvanic must implement*.

**Authoritative source.** `https://rust-lang.github.io/fls/` (also at `.lathe/refs/fls-pointer.md` with the full section table of contents).

**Where ambiguities live.** `refs/fls-ambiguities.md` — cases where the FLS is silent or underspecified and galvanic had to make a choice.

**Where constraints live.** `refs/fls-constraints.md` — cases where the FLS is clear and galvanic's implementation must comply.

**Boundary with ARM64 ABI.** The FLS says what operations mean. The ARM64 ABI says how to implement them on this hardware. When the FLS says "pass an argument" but doesn't say how, the ARM64 ABI defines the register convention. When the FLS says "integer overflow is undefined in const contexts" but doesn't say what trap to use at runtime, that's a question for galvanic's design, not the FLS.

**Boundary with Rust (rustc).** Galvanic is a *clean-room* implementation — it is not allowed to look at rustc internals. When a behavior looks like "that's just what Rust does," trace it to an FLS section before implementing it. If you can't find the FLS section, it's either an ambiguity (add to `fls-ambiguities.md`) or a constraint the FLS implies but doesn't state explicitly.

---

## Domain 2: The ARM64 Instruction Set Architecture (ISA)

**What it covers.** The actual machine instructions, register file, memory model, and instruction encoding for the AArch64 (ARM64) architecture. This is where galvanic's codegen decisions live.

**Authoritative source.** The ARM Architecture Reference Manual (ARM ARM). For register conventions, AAPCS64 (linked in `refs/arm64-abi.md`).

**What galvanic's design adds.** Cache-line alignment as a first-class constraint. Every data structure, every instruction layout decision, every loop structure is evaluated against the 64-byte ARM64 cache line. This is galvanic's research contribution on top of the ISA.

**Boundary with FLS.** The FLS says "call this function." ARM64 says "put the first argument in x0, the second in x1, return value in x0." When something looks wrong about a function call, check whether it's a semantic error (FLS boundary) or a calling-convention error (ARM64/ABI boundary).

**Boundary with platform ABI.** The ARM64 ISA is the same across macOS, Linux, and the BSDs. The differences are in how the OS handles syscalls and binary format — not in the instruction set itself.

---

## Domain 3: Platform ABI (macOS / Linux / BSDs)

**What it covers.** How each platform exposes OS services: syscall numbers, binary format (ELF vs. Mach-O), entry point conventions (`_start` vs. `main`), stack alignment requirements on entry, and cross-compilation toolchain requirements.

**Authoritative source.** `refs/arm64-platform-abi.md`. For macOS specifics: Apple's developer documentation. For Linux: SysV ELF ABI docs. For BSDs: the BSD source trees.

**Current target.** Galvanic emits Linux ARM64 ELF binaries (syscall via `svc #0` with syscall number in `x8`). These binaries cannot execute on macOS natively — macOS uses Mach-O format and different syscall conventions. On macOS, e2e tests require `qemu-aarch64` (Linux user-mode emulator).

**Toolchain.** The assembler (`aarch64-linux-gnu-as`) and linker (`aarch64-linux-gnu-ld`) are GNU binutils cross tools. On Linux CI these are installed via `apt-get install binutils-aarch64-linux-gnu qemu-user`. On macOS they are typically absent; assembly inspection tests (`compile_to_asm()`) work everywhere, but binary execution tests are Linux/CI only.

**Boundary with ARM64 ISA.** The instruction set is identical across platforms. Platform ABI differences show up only in: syscall instruction and register convention, binary format, entry point, and stack alignment at program start.

**Boundary with FLS.** The FLS says nothing about platform ABI. When a question is "why does this crash on Linux but not in the assembly inspection test," the answer is almost always in the platform domain — binary format, syscall convention, or missing program entry point.

---

## Domain 4: Galvanic's Internal Design

**What it covers.** The compiler's own architecture: pipeline structure, IR design, module boundaries, test conventions, and cache-line discipline. This is the domain that Compiler Contributors need to understand to add features.

**Authoritative source.** The source code and its inline documentation. Key files:

| Module | Role | FLS sections |
|--------|------|--------------|
| `src/lexer.rs` | Tokenization | FLS §2 (Lexical Elements) |
| `src/parser.rs` | Parse tokens → AST | FLS §5 (Patterns), §6 (Expressions), §7–§14 (Items) |
| `src/ast.rs` | AST type definitions | All parsed constructs |
| `src/ir.rs` | IR type definitions | FLS §9 (Functions), §6.19 (Return), §4.4 (Unit), §18.1 (Module) |
| `src/lower.rs` | AST → IR | All language semantics that produce runtime code |
| `src/codegen.rs` | IR → ARM64 assembly | ARM64 ISA, AAPCS64 ABI |
| `src/main.rs` | CLI driver | Invokes the pipeline; also calls assembler/linker |

**Key architectural rule.** Each module has one job. The lowering pass translates language semantics into the IR. The codegen pass translates the IR into machine instructions. Nothing in `lower.rs` knows about ARM64. Nothing in `codegen.rs` knows about Rust semantics. The IR is the contract between them.

**Where new features go.**
- New syntax → new AST node in `ast.rs`, new parser case in `parser.rs`.
- New language construct that produces runtime code → new IR instruction or type in `ir.rs`, new lowering case in `lower.rs`.
- New machine-level behavior → new codegen case in `codegen.rs`.
- New FLS ambiguity discovered → new entry in `refs/fls-ambiguities.md`.
- New FLS constraint verified → entry in `refs/fls-constraints.md`.

**Cache-line discipline.** Every new type added to a module that has cache-line commentary (lexer.rs, ir.rs) should include a cache-line note: what is the type's size, how does it fit in a 64-byte cache line, and what is the tradeoff. Size is enforced by tests (`token_is_eight_bytes`). When adding a new type, add a corresponding size test.

**Test structure.**
- `tests/smoke.rs` — CLI black-box tests for error messages and exit codes.
- `tests/fls_fixtures.rs` — parse-acceptance tests for FLS fixture programs.
- `tests/e2e.rs` — full-pipeline tests including assembly inspection and (on Linux/CI) binary execution.
- `tests/fixtures/` — FLS-derived fixture programs, one per spec section.

---

## Cross-Domain Confusion Points

These are the places where a bug in one domain looks like a bug in another.

**"The assembly is wrong" → which domain?**
- If the instruction is wrong for the Rust operation (e.g., using `add` where `mul` is needed): FLS/lower.rs boundary.
- If the instruction is correct but the registers are wrong: ARM64 ABI boundary (calling convention).
- If the instruction is correct and the registers are correct but the binary doesn't run: platform ABI boundary (binary format, syscall convention).

**"The test passes locally but fails on CI" → which domain?**
- If it's a build/compile error: almost certainly a Rust version or dependency issue (internal design).
- If it's an e2e binary execution test: platform ABI — the test needs `qemu-aarch64` and cross-toolchain, which only CI has.
- If it's a parser/lexer test: should be platform-agnostic; check for path assumptions.

**"FLS says X but galvanic does Y" → which domain?**
- If Y is documented in `fls-ambiguities.md` with galvanic's resolution: this is a known design decision, not a bug.
- If Y is not documented: add an entry to `fls-ambiguities.md` before changing the behavior. The documentation of galvanic's choices is as important as the choices themselves.
- If the FLS is clear (see `fls-constraints.md`) and galvanic violates it: this is a bug in the implementation.

**"Const folding should/shouldn't happen here" → which domain?**
- The rule is in `refs/fls-constraints.md` Constraint 1: only const contexts may fold at compile time.
- The enforcement is in `lower.rs`: non-const function bodies must emit runtime IR.
- The verification is in `tests/e2e.rs`: assembly inspection tests assert that runtime instructions are emitted, not folded immediates.
- If you're unsure whether a context is "const," the FLS §6.1.2 is the authority.
