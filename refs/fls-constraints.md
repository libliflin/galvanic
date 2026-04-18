# FLS Constraints — What Galvanic Must Not Do

This document states the requirements from the Ferrocene Language Specification
that constrain how galvanic generates code. It is the companion to
`refs/fls-ambiguities.md` (where the spec is silent or ambiguous). Here the spec
IS clear — these are the rules a conforming compiler must follow, and the record of
whether galvanic follows them.

---

## Constraint 1: Const evaluation is only permitted in const contexts

**FLS source:** §6.1.2:37–45, §6.1.2:48

Compile-time evaluation is valid ONLY in:
- `const` item initializers (`const X: i32 = 1 + 2;`)
- `const fn` bodies when called from a const context
- `const { ... }` block expressions
- `static` initializers
- Enum variant discriminant initializers
- Array length operands (`[T; N]`, `[expr; N]`)
- Const generic arguments and defaults

**Everything else is runtime code.** A regular `fn` body is not a const context.
Even if every value in the function is statically known, the compiler must emit
runtime instructions.

**Galvanic's status: Satisfied.** The lowering pass emits runtime IR for all
non-const function bodies. 1,700+ assembly inspection tests in `tests/e2e.rs`
assert that runtime instructions are emitted — not folded immediates. Example:

```rust
// The assembly inspection test for addition
let asm = compile_to_asm("fn main() -> i32 { 1 + 2 }\n");
assert!(asm.contains("add"), "expected add instruction");
assert!(!asm.contains("mov     x0, #3"), "must not constant-fold");
```

**The litmus test:** Replace every literal in a function with a function
parameter. If the implementation breaks, it was evaluating at compile time, not
compiling. `fn main() { while x < 5 { ... } }` and `fn foo(n: i32) { while x < n
{ ... } }` must use the same codegen path. The assembly inspection test suite
enforces this.

---

## Constraint 2: `const fn` outside a const context runs as normal code

**FLS source:** §9:41–43, Rust Reference

A `const fn` CAN be evaluated at compile time, but only when called from a const
context. When called from regular code, it must emit runtime instructions.

```rust
const fn add(a: i32, b: i32) -> i32 { a + b }

const X: i32 = add(1, 2);        // const context → compile-time eval OK
fn main() -> i32 { add(1, 2) }   // NOT const context → must emit runtime call
```

**Galvanic's status: Satisfied.** The test `runtime_const_fn_runtime_call_emits_bl_not_folded`
confirms that calling a `const fn` from a non-const context emits a `bl` (branch-
and-link) runtime call instruction rather than a folded immediate.

---

## Constraint 3: Arithmetic overflow semantics differ by context

**FLS source:** §6.1.2:49–50, §6.5.6, Rust Reference

- **In const contexts:** Overflow is a compile-time error.
- **At runtime (debug mode):** Overflow panics.
- **At runtime (release mode):** Overflow wraps (two's complement).
- **Exception:** Division by zero and `MIN / -1` always panic, regardless of mode.

**Galvanic's status: Partially satisfied, partially deferred.**

- Wrap-on-overflow for narrow types (u8, u16, i8, i16) is implemented. The tests
  `runtime_u8_add_emits_and_truncation`, `runtime_i8_add_emits_sxtb_sign_extension`,
  and related tests verify this.
- Debug-mode panic-on-overflow for arithmetic (+, -, *) is not yet implemented.
  Galvanic currently behaves like release mode for these operators (wrapping), not
  debug mode (panicking). This is a known deviation.
- Division-by-zero: guards ARE emitted. A `cbz xRHS, _galvanic_panic` guard is
  emitted before every `sdiv`, `srem`, and `udiv` instruction. Literal zero divisors
  are rejected by the lowering pass before any IR is produced. The `i32::MIN / -1`
  overflow case is also guarded. Full details in `refs/fls-ambiguities.md`
  (§6.9/§6.23 — Panic Mechanism).

---

## Constraint 4: Evaluation order is left-to-right with side effects preserved

**FLS source:** §6:3, §6:11, §6.4:14–15, §6.5.9:7–14

- Expressions evaluate left-to-right.
- Statements execute in declaration order.
- Lazy boolean operators (`&&`, `||`) short-circuit.
- Side effects must occur in the order specified by the source program.

**Galvanic's status: Structurally satisfied.** The lowering pass processes
expressions in source order and emits IR instructions in that order. No
reordering optimization is applied. The `runtime_and_emits_cbz_for_short_circuit`
and `runtime_or_emits_cbz_for_short_circuit` tests verify that lazy evaluation
is emitted at the instruction level.

---

## Constraint 5: Variables are places, not compile-time constants

**FLS source:** §6.1.4, Rust Reference

A place expression represents a memory location (variable, field, index, dereference).
Variables must actually exist in memory. A compiler cannot treat all variables as
compile-time constants in a lookup table — they are locations that can be borrowed,
mutated, and have their address taken.

**Galvanic's status: Satisfied.** All local variables are allocated stack slots in
the lowering pass. No variable is treated as a named constant that gets inlined.
The `runtime_let_binding_emits_str_and_ldr` test confirms that a let binding
produces an actual store-to-stack followed by a load-from-stack.

---

## Constraint 6: `const` items are substituted; `static` items have identity

**FLS source:** §7.1:10, §7.2:15

- `const` items: every use is replaced with the value at compile time (no memory
  address; cannot be borrowed).
- `static` items: all references point to the same unique memory location (has
  an address in the binary).

**Galvanic's status: Satisfied.** The `runtime_const_emits_load_imm_not_stack_load`
test confirms that `const` items are emitted as immediate loads (not stack loads).
The `runtime_static_emits_adrp_add_ldr` test confirms that `static` items are
emitted as address-materialization via `adrp`/`add`/`ldr` — a real memory location
in the `.data` section.

---

## Constraint 7: The spec imposes no iteration limit on const evaluation

**FLS source:** Neither the FLS nor the Rust Reference specifies a step limit.

The spec does not say "const evaluation may loop at most N times." An
implementation-defined step limit (like rustc's) is conforming but not required.

**Galvanic's status: N/A for non-const code.** Galvanic imposes no iteration limit
on const evaluation. This is only relevant for actual `const` contexts — non-const
loops emit runtime code (see Constraint 1) and have no compile-time iteration limit
because they never execute at compile time.

---

## Summary

| Constraint | Status |
|---|---|
| 1. No const eval outside const context | **Satisfied** (1,700+ asm inspection tests) |
| 2. `const fn` runs normally in non-const context | **Satisfied** (`runtime_const_fn_runtime_call_emits_bl_not_folded`) |
| 3. Overflow semantics differ by context | **Partial** (div-by-zero guarded; +/-/* overflow panic deferred) |
| 4. Left-to-right evaluation order | **Satisfied** (source-order lowering; short-circuit tests) |
| 5. Variables are memory locations | **Satisfied** (`runtime_let_binding_emits_str_and_ldr`) |
| 6. `const` substituted; `static` has identity | **Satisfied** (`runtime_const_emits_load_imm_not_stack_load`, `runtime_static_emits_adrp_add_ldr`) |
| 7. No spec-mandated iteration limit | **N/A** (non-const loops are runtime code) |

The one genuine FLS gap: debug-mode panic-on-overflow for arithmetic operators
(+, -, *). Division-by-zero and bounds-check panics are implemented. The full
panic mechanism is documented in `refs/fls-ambiguities.md` (§6.9/§6.23).

---

## Note: Constraint 8 is a design principle, not an FLS requirement

The entry below is about compiler correctness, not FLS conformance. It is kept
here because the violation caused a real CI incident, but it does not belong in
the FLS compliance summary table above.

---

## Constraint 8: Use Rust's type system to make invalid IR states unrepresentable

**Source:** Project design principle (not FLS — this is about compiler correctness).

Galvanic is written in Rust. We should use Rust's type system to prevent classes
of bugs at compile time, not catch them at runtime or in CI.

### Problem: untyped IR lost semantic information

The IR currently uses `u8` for register indices, slot indices, and array lengths
interchangeably. `BinOp` carries no type tag — the same `IrBinOp::Add` is used
for i32 user arithmetic AND for internal pointer/index calculations. This caused
a real incident: an overflow guard meant for i32 arithmetic fired on address
calculations, breaking 24 tests. The bug was invisible on macOS (tests skipped)
and produced a 75-PR failure loop in CI.

### Required: newtype wrappers and type-tagged operations

**Register newtypes** — distinguish register kinds at the type level:

```rust
pub struct GpReg(pub u8);   // general-purpose register x0-x30
pub struct FpReg(pub u8);   // float/SIMD register d0-d31 / s0-s31
pub struct Slot(pub u16);   // stack slot index
```

Cannot pass a `Slot` where a `GpReg` is expected. Cannot mix float and integer
registers. The compiler catches this at build time.

**Type-tagged BinOp** — codegen knows what guards to emit:

```rust
BinOp {
    op: IrBinOp,
    ty: IrTy,       // ← what type is this operation on?
    dst: GpReg,
    lhs: GpReg,
    rhs: GpReg,
}
```

With a `ty` field, codegen can match: emit overflow guard for `IrTy::I32`,
skip it for address arithmetic (which would use a distinct type or a dedicated
`AddAddr` instruction).

**Dedicated address arithmetic** — alternatively, separate instructions:

```rust
// User arithmetic — may need overflow guards
BinOp { op, ty, dst, lhs, rhs }

// Internal address calculation — never guarded
AddrAdd { dst: GpReg, base: GpReg, offset: GpReg }
```

### Status: Not yet satisfied

The IR still uses bare `u8` and untyped `BinOp`. This is the next structural
improvement needed before adding more runtime guards.

### Principle

Every time you add a field or variant to the IR, ask: **can the type system
prevent misuse?** If two things should never be confused (registers vs slots,
user arithmetic vs address math, signed vs unsigned), they should be different
types. The cost is a few newtype wrappers. The benefit is that entire classes
of codegen bugs become compile errors.
