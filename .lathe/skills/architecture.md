# Galvanic Architecture

This file answers: how does the compiler work, where do the phases live, and what are the invariants that every new contribution must respect?

---

## Pipeline

```
source text
   │
   ▼  src/lexer.rs
Token stream  (Vec<Token>)
   │
   ▼  src/parser.rs
SourceFile  (AST, src/ast.rs)
   │
   ▼  src/lower.rs
Module  (IR, src/ir.rs)
   │
   ▼  src/codegen.rs
ARM64 assembly text  (String)
   │
   ▼  main.rs (CLI only)
.s file → aarch64-linux-gnu-as → .o → aarch64-linux-gnu-ld → ELF binary
```

Each phase is a pure function of its input. The library (`src/lib.rs`) exposes all phases; `src/main.rs` is the CLI driver that invokes them and optionally assembles/links.

---

## src/lexer.rs — Tokenization

Implements FLS §2. Produces a `Vec<Token>` terminated by `TokenKind::Eof`.

**Critical invariant: `Token` is 8 bytes.**

```
offset 0 │ kind:  TokenKind  (u8, repr(u8))   — 1 byte
offset 1 │ <pad>                               — 1 byte (reserved for future use)
offset 2 │ start: u32                          — 4 bytes  (wait — check actual layout)
```

Actually verify with `size_of` — the invariant is that 8 tokens fit in one 64-byte cache line. The layout is enforced structurally by the `repr(u8)` on `TokenKind` and the field ordering. There is a unit test `lexer::tests::token_is_eight_bytes` that asserts this.

**Whitespace and comments** are consumed and not emitted. Only meaningful tokens appear in the output.

**Non-ASCII identifiers**: Accepted by the lexer but NFC normalization is not applied (FLS §2.3 gap — documented as AMBIGUOUS).

---

## src/ast.rs — Abstract Syntax Tree

Mirrors the FLS grammar structure. Each node type cites its FLS section.

**Key types:**
- `SourceFile` — root of the tree
- `Item` / `ItemKind` — top-level items (fn, struct, enum, impl, trait, const, static, type alias)
- `Expr` / `ExprKind` — expressions (all of FLS §6)
- `Stmt` / `StmtKind` — statements (let, expression statement)
- `Pat` — patterns (FLS §5)
- `Ty` / `TyKind` — types (FLS §4)
- `Span` — 8-byte source location (start: u32, len: u32)

**Cache-line note on AST**: Currently uses `Box<T>` for recursive nodes — each dereference is a potential cache miss. An arena design with `u32` indices is flagged as future work. Don't refactor this during normal cycles; the research value at this stage is FLS mapping correctness, not traversal performance.

---

## src/ir.rs — Intermediate Representation

Minimal by design. The IR is the smallest representation that can express what codegen needs. Nothing is added before it is needed.

**Module** (top-level IR unit):
- `fns: Vec<IrFn>` — compiled functions
- `statics: Vec<StaticData>` — static variables
- `trampolines: Vec<ClosureTrampoline>` — closure-to-impl-Fn bridges
- `vtable_shims: Vec<VtableShim>` — dyn Trait dispatch shims
- `vtables: Vec<VtableSpec>` — vtable data records

**IrFn** (one function):
- `name: String` — mangled name
- `ret_ty: IrTy` — return type
- `body: Vec<Instr>` — flat instruction list (no basic blocks yet)
- `stack_slots: u8` — 8-byte stack slots for locals
- `saves_lr: bool` — whether to save the link register (non-leaf functions)
- `float_consts: Vec<u64>` — f64 constant pool
- `float32_consts: Vec<u32>` — f32 constant pool

**Instr** — the instruction set grows milestone by milestone:
- `Ret(IrValue)` — return
- `LoadImm(dst, n)` — integer immediate
- `BinOp { op, dst, lhs, rhs }` — integer arithmetic/comparison
- `Store { src, slot }` — store to stack
- `Load { dst, slot }` — load from stack
- `Label(name)` — branch target
- `Branch(label)` — unconditional branch
- `CondBranch { cond, true_label, false_label }` — conditional branch
- `Call { ... }` — function call (FLS §6.12.1)
- And many more — read `ir.rs` for the current full list

**Principle**: Add instructions one at a time, when they are required by the next runnable program. Never speculate ahead.

---

## src/lower.rs — AST → IR Lowering

The most important correctness invariant lives here: **all non-const code emits runtime IR instructions**. No constant folding of non-const functions. See `refs/fls-constraints.md`.

**Const contexts** (where compile-time evaluation IS permitted):
- `const` item initializers
- `static` initializers
- Enum variant discriminants
- Array length expressions
- `const` block expressions

**Everything else** must emit `LoadImm`, `BinOp`, `Store`, `Load`, `Branch`, etc. — real runtime instructions that a CPU will execute.

The litmus test (from `fls-constraints.md`): if replacing a literal with a function parameter would break the implementation, it's an interpreter.

**Local variable tracking**: A `HashMap<String, u8>` maps variable names to stack slot indices. The lowering pass allocates a stack slot for every `let` binding and spills function parameters to stack on entry.

**FLS-tracing convention**: Every new lowering function must cite its FLS section. If a section is ambiguous, document it: `// FLS §X.Y: AMBIGUOUS — <what's unclear>`.

---

## src/codegen.rs — ARM64 Assembly Emission

Translates `Module` → GNU assembler text for `aarch64-linux-gnu-as`.

**Output format**: GAS syntax (`.text`, `.globl`, labels, ARM64 mnemonics).

**Entry point**: `_start` is emitted as the ELF entry point; it calls `main` and issues `exit` syscall with `x0` as the code.

**Calling convention**: Each function argument/local is in a virtual register (x0..xN). For parameters, the lowering pass spills them to stack on entry and loads them for each use — no register allocation pass yet. This is intentionally simple; a real register allocator is future work.

**Cache-line notes**: ARM64 instructions are 4 bytes each; 16 instructions per 64-byte cache line. The codegen documents cache-line budgets inline for key structures.

---

## Invariants that must never break

1. **No unsafe in library code** (`src/` except `main.rs`). The CI `audit` job enforces this.
2. **No `std::process::Command` in library code** (`src/` except `main.rs`). The library must not shell out.
3. **No networking dependencies** in `Cargo.toml`.
4. **`Token` is 8 bytes**. The unit test `lexer::tests::token_is_eight_bytes` asserts this.
5. **`Span` is 8 bytes**. (Enforced by layout: two `u32` fields.)
6. **Non-const lowering emits runtime instructions**. No interpreter behavior.
7. **Every FLS construct traces to a section**. `// FLS §X.Y: description` on every relevant decision.

---

## What "milestone" means in the codebase

The `ir.rs` and `lower.rs` comments refer to milestones: "Milestone 11 adds LoadImm and BinOp." This is the project's development history — each milestone is a runnable program that exercises one new FLS section or feature. The current instruction set grew one milestone at a time; new instructions are added the same way. Don't add an IR instruction until there's a concrete test that requires it.
