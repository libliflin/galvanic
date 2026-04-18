//! # Galvanic — Compiler Pipeline
//!
//! Galvanic is a clean-room ARM64 Rust compiler built from the Ferrocene Language
//! Specification (FLS). Its two research questions: (1) where is the FLS ambiguous or
//! silent? (2) what does cache-line-aware codegen look like as a first-class constraint?
//!
//! ## Pipeline
//!
//! ```text
//! source text
//!     │
//!     ▼
//! lexer::tokenize()     →  Vec<Token>           (FLS §2 — Lexical Elements)
//!     │
//!     ▼
//! parser::parse()       →  SourceFile (AST)     (FLS §5–§6, §7–§14, §18)
//!     │
//!     ▼
//! lower::lower()        →  Module (IR)           (FLS semantics — all language rules)
//!     │
//!     ▼
//! codegen::emit_asm()   →  String (ARM64 GAS)   (AAPCS64 ABI, cache-line discipline)
//!     │
//!     ▼
//! aarch64-linux-gnu-as  →  .o object file
//!     │
//!     ▼
//! aarch64-linux-gnu-ld  →  ELF binary (Linux ARM64)
//! ```
//!
//! Each stage has one job and a clean boundary. Nothing earlier in the pipeline knows
//! about later stages. The IR is the contract between language semantics ([`lower`]) and
//! machine instructions ([`codegen`]).
//!
//! ## Modules
//!
//! | Module | Role | FLS sections |
//! |--------|------|--------------|
//! | [`lexer`] | Source text → `Vec<Token>`. Each `Token` is 8 bytes (8 per cache line). | §2 |
//! | [`ast`] | AST type definitions — no logic, just types (`Item`, `Expr`, `Pat`). | §5–§6, §7–§14 |
//! | [`parser`] | `Vec<Token>` → `SourceFile` (AST). Recursive descent. | §5, §6, §7–§14, §18 |
//! | [`ir`] | IR type definitions (`Module`, `IrFn`, `Instr`, `IrValue`, `IrTy`). Every node traces to an FLS section. | §4, §6.19, §8, §9 |
//! | [`lower`] | AST → IR. FLS semantic rules live here. Emits runtime IR for non-const code. | all language semantics |
//! | [`codegen`] | IR → ARM64 GAS assembly. Cache-line discipline lives here. | ARM64 ISA, AAPCS64 |
//!
//! `src/main.rs` is the CLI driver — the only module that shells out to external processes.
//!
//! ## Adding a new language feature
//!
//! 1. **Find the FLS section.** See `refs/fls-ambiguities.md` for known gaps.
//! 2. **New syntax?** Add AST types to [`ast`], a parser case to [`parser`].
//! 3. **New runtime behavior?** Add an `Instr` or `IrValue` variant to [`ir`] with an
//!    FLS traceability comment and a cache-line note. Add a size assertion test.
//! 4. **Lowering case** in [`lower`] — translates the AST node to the IR node using the
//!    FLS semantic rule. When it fails, the error must name the function, FLS section,
//!    and specific construct.
//! 5. **Codegen case** in [`codegen`] — translates the IR node to ARM64 instructions.
//!    Comment register usage and cache-line reasoning.
//! 6. **Tests:** fixture in `tests/fixtures/fls_<section>_<topic>.rs`, parse acceptance
//!    test in `tests/fls_fixtures.rs`, assembly inspection test in `tests/e2e.rs`.
//!
//! ## Key invariants (enforced by CI)
//!
//! - **No `unsafe` code** anywhere in `src/`. The `audit` job enforces this.
//! - **No `Command` in library code.** Only `src/main.rs` may shell out.
//! - **Every IR node traces to an FLS section.** Format: `// FLS §X.Y — <description>`.
//! - **Cache-line-critical types have size tests.** Types in [`lexer`] and [`ir`] that
//!   have cache-line commentary must have a corresponding `assert_eq!(size_of::<T>(), N)`.
//! - **No const folding in non-const contexts.** FLS §6.1.2 Constraint 1: a regular
//!   `fn` body must emit runtime instructions even when all values are statically known.
//!   Assembly inspection tests in `tests/e2e.rs` enforce this.

pub mod ast;
pub mod codegen;
pub mod ir;
pub mod lexer;
pub mod lower;
pub mod parser;
