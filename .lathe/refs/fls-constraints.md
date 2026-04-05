# FLS Constraints — What the Compiler Must NOT Do

This document captures **restrictions** from the Ferrocene Language Specification
that constrain how galvanic generates code. Features tell you what to build.
Constraints tell you what you must not do. Read this before every cycle.

These constraints are load-bearing. Violating them produces code that appears
to work (tests pass, exit codes are correct) but is semantically wrong.

---

## Constraint 1: Const evaluation is only permitted in const contexts

**Source:** FLS §6.1.2:37–45, §6.1.2:48

Compile-time evaluation happens ONLY in these contexts:

- `const` item initializers (`const X: i32 = 1 + 2;`)
- `const fn` bodies when called from a const context
- `const` block expressions (`const { ... }`)
- `static` initializers
- Enum variant discriminant initializers
- Array length operands (`[T; N]`, `[expr; N]`)
- Const generic arguments and defaults

**Everything else is runtime code.** A regular `fn main()` body is NOT a const
context. Even if every value in the function is statically known, the compiler
must emit runtime instructions. The fact that inputs happen to be constant
does not make code const-evaluable.

**The litmus test:** If your implementation of a language feature cannot handle
the case where a literal is replaced with a function parameter, you have built
an interpreter, not a compiler. `fn main() { while x < 5 { ... } }` and
`fn foo(n: i32) { while x < n { ... } }` must use the same codegen path.

**Anti-pattern:** Evaluating a while loop at compile time inside `fn main()`
and emitting `mov x0, #result`. This produces the correct exit code but
violates the spec — the loop must execute at runtime via branch instructions.

### What this means for galvanic

The current lowering pass (`src/lower.rs`) performs compile-time evaluation
of ALL code, including non-const functions. This is incorrect. The lowering
pass must emit runtime IR instructions (branches, comparisons, stack
operations) for non-const code. Constant folding of non-const code is only
valid as an optimization pass applied AFTER correct runtime IR is generated.

---

## Constraint 2: Const fn outside const context runs as normal code

**Source:** FLS §9:41–43, Rust Reference

A `const fn` CAN be evaluated at compile time, but only when called from a
const context. When called from regular (non-const) code, it must emit
runtime instructions like any other function.

```rust
const fn add(a: i32, b: i32) -> i32 { a + b }

const X: i32 = add(1, 2);        // const context → compile-time eval OK
fn main() -> i32 { add(1, 2) }   // NOT const context → must emit runtime call
```

---

## Constraint 3: Arithmetic overflow semantics differ by context

**Source:** FLS §6.1.2:49–50, §6.5.6, Rust Reference

- **In const contexts:** Overflow is a compile-time error (static error).
- **At runtime (debug mode):** Overflow panics.
- **At runtime (release mode):** Overflow wraps (two's complement).

**Exception:** Division by zero and signed `MIN / -1` ALWAYS panic, even
in release mode with overflow checks disabled.

A compiler that const-folds `255_u8 + 1` in a non-const function to `0`
has applied release-mode wrapping semantics. In debug mode, this should
panic at runtime. The compiler must not silently choose one behavior.

---

## Constraint 4: Evaluation order is left-to-right with side effects

**Source:** FLS §6:3, §6:11, §6.4:14–15, §6.5.9:7–14

- Expressions with multiple operands evaluate left-to-right.
- Statements execute in declaration order.
- Lazy boolean operators (`&&`, `||`) short-circuit.
- Side effects (FLS §6:3: "may have side effects at run-time") must occur
  in the specified order at runtime.

A compile-time evaluator can reorder or skip evaluation because it sees no
side effects. A runtime codegen must preserve evaluation order because
future features (function calls with side effects, I/O, panics) depend on it.

---

## Constraint 5: Place expressions vs value expressions

**Source:** FLS §6.1.4, Rust Reference

- A place expression represents a memory location (variable, field, index, deref).
- A value expression represents a value.
- When a place expression is used in a value context: if `Copy`, the value
  is copied; if `Sized`, it may be moved (deinitializing the source).

This means variables must actually exist in memory (or registers). A compiler
cannot treat all variables as compile-time constants in a `HashMap` — they
are places that can be borrowed, moved, and have their address taken.

---

## Constraint 6: Constants are substituted, statics have identity

**Source:** FLS §7.1:10, §7.2:15

- `const` items: every use is replaced with the value (no memory location).
- `static` items: all references point to the same memory location.

This affects codegen: constants can be inlined as immediates, but statics
must have an address in the data section.

---

## Constraint 7: No explicit iteration limit in the spec

**Source:** Neither FLS nor Rust Reference specifies a limit.

The spec does not say "const evaluation may loop at most N times." Rustc's
MIRI evaluator has an implementation-defined step limit, but this is not
spec-mandated. A conforming compiler may choose any limit (or none) for
const evaluation.

However, this is only relevant for actual const contexts. Non-const loops
must emit runtime code and have no compile-time iteration limit because
they don't execute at compile time at all (see Constraint 1).

---

## How to use this document

Before implementing a language feature, check:

1. **Am I generating runtime code, or evaluating at compile time?**
   If evaluating at compile time, am I in a const context? If not, I must
   emit runtime instructions.

2. **Would my implementation break if a literal were replaced with a parameter?**
   If yes, I'm interpreting, not compiling.

3. **Am I making assumptions about overflow behavior?**
   The behavior depends on context (const vs runtime) and mode (debug vs
   release). Don't hardcode one.

4. **Am I preserving evaluation order and side-effect timing?**
   Even if today's test cases have no side effects, the codegen must preserve
   order for correctness when side effects are added later.
