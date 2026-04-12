# FLS Ambiguity Findings — Galvanic

This document aggregates the `AMBIGUOUS` annotations scattered across
`src/lexer.rs`, `src/parser.rs`, `src/ast.rs`, `src/ir.rs`, `src/lower.rs`,
and `src/codegen.rs`. Each entry names the spec gap, galvanic's chosen
resolution, and the source location where the annotation lives.

Entries are grouped by FLS section. Generated from annotations present as of
the commit that introduced this file; check the individual source annotations
for full context.

---

## §2.4.4.1 — Integer Literals: Large-Immediate Encoding

**Gap:** The FLS specifies integer literal syntax and types but does not describe
how a compiler should encode integers that exceed 16 bits in ARM64 assembly.

**Galvanic's choice:** Integers that fit in 16 bits use `mov x0, #N`. Larger
values that fit in 32 bits use `movz`/`movk` pairs. Values requiring 64-bit
encoding use up to four `movz`/`movk` instructions. This is standard ARM64
practice but is not mandated by the spec.

**Source:** `src/lower.rs` (search `MOVZ+MOVK`)

---

## §2.4.4.2 — Float Literals: NaN, Infinity, Hex Floats

**Gap:** The FLS does not specify the handling of NaN/infinity literal forms
(Rust has none) or hexadecimal float literals (e.g. `0x1.fp10`).

**Galvanic's choice:** Only decimal float literals with optional `_f32`/`_f64`
suffix are supported. NaN/infinity are not expressible as literals. Hex floats
are rejected at the lexer level.

**Source:** `src/lower.rs:3968`

---

## §2.6 — Keyword Classification: `'static` and `_`

**Two distinct gaps:**

1. **`'static`:** Listed as a "weak keyword" but the spec does not define a
   boundary between lifetime-as-keyword and lifetime-as-identifier. Galvanic
   emits all `'ident` forms as `Lifetime` tokens; the parser gives `'static`
   special meaning at the semantic level.

2. **`_`:** Appears in both the strict-keyword table and the punctuation table.
   The spec does not state an explicit precedence rule. Galvanic emits
   `Underscore` for a bare `_` not followed by an XID_Continue character;
   `_foo` and `__x` are emitted as `Ident`.

**Source:** `src/lexer.rs:185`, `src/lexer.rs:234`

---

## §4.1 — Built-in Associated Constants (MIN, MAX, BITS)

**Gap:** The FLS does not enumerate which associated constants are built into
primitive integer types (e.g. `i32::MAX`, `u8::MIN`, `i32::BITS`).

**Galvanic's choice:** Recognizes `MIN`, `MAX`, and `BITS` for all supported
integer and float types as compile-time constants resolved during lowering.
The set is chosen to match observed Rust usage, not a spec-defined list.

**Source:** `src/lower.rs:1335`

---

## §4.2 / §2.4.5 — `char` Type Encoding

**Gap:** The FLS describes `char` as "the Unicode scalar value type" but does
not provide a section number in the FLS TOC that specifies its in-memory
representation. §2.4.5 covers char literal syntax but is absent from the
main TOC.

**Galvanic's choice:** Maps char literals to their Unicode code point as a
`u32` (4 bytes). Stored and loaded as 32-bit integers on the stack.

**Source:** `src/lower.rs:4000`

---

## §4.8 / §4.9 — Fat Pointer ABI for `&str` and `&[T]`

**Gap:** The FLS specifies that `&str` is a slice of bytes (fat pointer) and
`&[T]` is a slice reference, but does not define the ABI — which registers
carry the pointer and length, or how they are passed and returned.

**Galvanic's choice:** Two consecutive stack slots (or two consecutive
registers x0/x1 for parameters): slot N = base pointer, slot N+1 = byte
length (usize). For `&str`, length is the byte count of the UTF-8 encoding.
For `&[T]`, length is the element count.

**Source:** `src/lower.rs:3572`, `src/lower.rs:3636`, `src/lower.rs:4692`

---

## §4.9 — Bounds Checking Mechanism

**Gap:** The FLS requires that indexing out of bounds panics (§6.9), but does
not specify the panic mechanism — whether it is a library call, a trap
instruction, or something else.

**Resolution (Claim 4p):** Every array and slice index emits a runtime bounds
check before the address computation:
- `cmp x{idx}, x{len}` — unsigned comparison; negative indices also trigger the
  guard via wraparound.
- `b.hs _galvanic_panic` — branches if idx >= len (unsigned).
- The `_galvanic_panic` trampoline executes `exit(101)`.
This matches Rust's debug-mode behavior: out-of-bounds indexing panics.

**Source:** `src/codegen.rs` (bounds check emission), `src/lower.rs` (IndexAccess IR)

---

## §4.13 — `dyn Trait` Vtable Layout and Fat Pointer Return ABI

**Three distinct gaps:**

1. **Vtable layout:** The FLS does not specify vtable layout — offset of each
   method, whether a destructor slot exists at offset 0, alignment. Galvanic
   uses 8-byte slots starting at offset 0 for the first trait method, offset 8
   for the second, etc. No destructor slot is emitted.

2. **Fat pointer return ABI:** When a function returns `dyn Trait`, the spec
   does not define how the fat pointer (data ptr + vtable ptr) is returned.
   Galvanic allocates two stack slots in the caller and passes their addresses
   as hidden output parameters.

3. **Vtable shim layout:** The spec does not define how a concrete type's
   methods are wrapped into vtable shim functions. Galvanic emits a dedicated
   shim label that adjusts the receiver and dispatches to the concrete method.

**Source:** `src/ir.rs:984`, `src/codegen.rs:119`, `src/codegen.rs:252`,
`src/lower.rs:3281`, `src/lower.rs:9784`

---

## §5.1.4 — Pattern Binding and Or-Patterns: Evaluation Order

**Gap:** The FLS does not specify the order in which alternatives in an
or-pattern are evaluated, or whether identically-named bindings in different
alternatives must have the same type (enforcement mechanism unspecified).

**Galvanic's choice:** Alternatives are evaluated left-to-right at runtime.
Each alternative that matches stores to the same binding slot (identified by
name). Type consistency is not verified at this milestone.

**Source:** `src/ast.rs:1733`, `src/lower.rs:7821`, `src/parser.rs:3409`

---

## §5.1.8 — Rest Patterns (`..`) Inside Slice Patterns

**Gap:** The FLS allows `..` inside slice patterns (`[a, .., b]`) but does
not specify the evaluation order or how many elements the rest pattern
consumes when the slice has fewer elements than the pattern's fixed positions.

**Galvanic's choice:** The rest pattern consumes zero or more elements from
the middle. Pattern match fails if the slice is shorter than the fixed
elements (a + b positions). No elements are bound from the rest.

**Source:** `src/parser.rs:3612`

---

## §6.1.2 — Overflow in Const Contexts

**Gap:** The FLS states overflow in const contexts should be a compile-time
error (§6.1.2:49–50), but does not specify the exact diagnostic or whether
all subexpressions must be error-checked.

**Galvanic's choice:** Const expressions are evaluated in `i32`; narrow types
(`u8`, `i8`, `u16`, `i16`) have the result wrapped (truncated/sign-extended)
rather than rejected. This is a pragmatic choice for FLS-faithful runtime
codegen rather than full const-eval diagnostics.

**Source:** `src/lower.rs:414`

---

## §6.4.2 — Const Block: Permitted Expression Forms

**Gap:** The FLS permits "constant expressions" inside `const { ... }` blocks
but does not enumerate precisely which expression forms qualify. The spec
says const expressions may appear in const contexts; it does not list every
expressly disallowed form.

**Galvanic's choice:** Const blocks are evaluated using the same `eval_const_expr`
path as `const` items. If an expression is not evaluable at compile time
(e.g., a function call to a non-const fn), lowering returns an error. `const fn`
calls are permitted if the callee was declared `const`.

**Source:** `src/lower.rs:613`, `src/lower.rs:627`

---

## §6.4.4 — Unsafe Block: Permitted vs Required Operations

**Gap:** The FLS enumerates what is *permitted* inside an unsafe block
(raw pointer dereference, unsafe fn calls, etc.) but does not specify what
happens if an unsafe block contains only safe operations — i.e., whether
the compiler must warn/error on a trivially-safe unsafe block.

**Galvanic's choice:** Unsafe blocks compile identically to safe blocks;
the `unsafe` keyword affects parse/type-checking only. No warning is emitted
for unnecessary `unsafe`.

**Source:** `src/lower.rs:16418`, `src/ast.rs:1459`

---

## §6.5.3 — NaN Comparison Behavior

**Gap:** The FLS references IEEE 754 semantics for floating-point but does
not explicitly state the behavior of comparisons involving NaN (e.g. whether
`f != f` is guaranteed true for NaN, or what `<`/`>`/`<=`/`>=` return).

**Galvanic's choice:** ARM64 `fcmp` sets flags per IEEE 754. `cset` then
produces 0 or 1. NaN comparisons produce 0 for ordered comparisons (`<`, `>`,
`<=`, `>=`) and 1 for `!=` — matching IEEE 754 but relying on hardware
behavior rather than a spec guarantee.

**Source:** `src/ir.rs:1445`, `src/lower.rs:14875`

---

## §6.5.5 — Floating-Point: IEEE 754 Reference Without Encoding Details

**Gap:** The FLS references IEEE 754 semantics for float arithmetic but does
not specify the binary encoding (single vs double precision), rounding mode,
or treatment of subnormals.

**Galvanic's choice:** `f32` maps to ARM64 32-bit S-registers (IEEE 754
binary32); `f64` maps to 64-bit D-registers (IEEE 754 binary64). The default
ARM64 rounding mode (round-to-nearest, ties-to-even) is used. Subnormals are
passed through unchanged.

**Source:** `src/ir.rs:1265`

---

## §6.5.7 — Shift Amount Modulo Behavior

**Gap:** The FLS states "the shift amount is taken modulo the bit width"
(§6.5.7) but does not specify whether this modulo is performed in the source
language or delegated to the hardware. ARM64 `lsl`/`asr`/`lsr` already
mask the shift amount to 6 bits (mod 64).

**Galvanic's choice:** No explicit masking instruction is emitted; the ARM64
hardware behavior (implicit mod 64) satisfies the spec requirement for
64-bit types. For narrower types whose values are stored in 64-bit registers,
this could produce surprising results — not yet addressed.

**Source:** `src/codegen.rs:594`, `src/lower.rs:10639`

---

## §6.5.9 — Narrowing Integer Casts and Float-to-Int Casts

**Two distinct gaps:**

1. **Float-to-int (`as` cast):** The spec says narrowing integer casts
   "truncate the value" but does not specify behavior for out-of-range float
   values. ARM64 `FCVTZS` saturates (clamps) out-of-range values to INT_MIN
   or INT_MAX; the Rust reference says behavior is saturating since Rust 1.45,
   but the FLS does not state this.

2. **Wrapping for integer-to-integer narrowing:** `as u8` truncates to the
   low 8 bits; `as i8` sign-extends. The spec does not enumerate which bits
   are kept.

**Galvanic's choice:** Uses ARM64 `FCVTZS` (saturating) for float-to-int;
uses `AND`/`SXTB`/`SXTH` for integer truncation/sign-extension. Matches
Rust's de-facto behavior.

**Source:** `src/ir.rs:1337`, `src/codegen.rs:1392`, `src/lower.rs:16898`

---

## §6.9 / §6.23 — Panic Mechanism

**Gap:** The FLS requires panics for divide-by-zero (§6.23), out-of-bounds
indexing (§6.9), and integer overflow in debug mode (§6.23), but does not
specify the panic mechanism — library call, trap instruction, signal handler.

**Galvanic's choice (updated — Claims 4m, 4o, 4p, 4q):**
- Divide-by-zero with a literal 0 divisor: **caught at compile time** in
  `src/lower.rs`. The lowering pass rejects integer `/` and `%` expressions
  whose RHS is `LitInt(0)` before emitting any IR. (Claim 4m)
- Non-literal zero divisors (`x / y` where `y` may be zero at runtime):
  a `cbz xRHS, _galvanic_panic` guard is emitted before every `sdiv`, `srem`,
  and `udiv` instruction. (Claim 4o)
- `i32::MIN / -1` and `i32::MIN % -1` overflow guard: emitted before `sdiv`
  for both division and remainder. Uses `movz`/`sxtw` to materialise
  `i32::MIN` as a 64-bit sign-extended constant, then `cmp`/`cmn` to detect
  the overflow case, branching to `_galvanic_panic`. (Claim 4q)
- Out-of-bounds indexing: `cmp`/`b.hs` bounds check before every array/slice
  load and store; out-of-bounds branches to `_galvanic_panic`. (Claim 4p)
- `+`, `-`, `*` overflow (Claim 4s): guarded by `sxtw x9, w{dst}` + `cmp x{dst}, x9`
  + `b.ne _galvanic_panic` after every `add`, `sub`, and `mul` instruction.
  Fires when the 64-bit result does not equal its own sign-extended 32-bit self
  (i.e., the result does not fit in i32).
  FLS §6.23 AMBIGUOUS: the guard treats all arithmetic as i32 because galvanic
  has no type system. False positives for i64 values outside i32 range; false
  negatives for u32 arithmetic that wraps within [0, 2^32) but outside i32 range.
  At this milestone, the test suite exercises i32; documented as a limitation.

The panic primitive `_galvanic_panic` calls `sys_exit(101)` directly. No stack
unwinding, no panic message. This matches the FLS requirement (panics terminate
the program) while keeping the implementation simple.

**Source:** `src/lower.rs` (literal zero check),
`src/codegen.rs` (cbz, MIN/-1 guard, bounds check, `_galvanic_panic`)

---

## §6.11 — Struct Expression Shorthand and Update Syntax

**Two gaps:**

1. **Shorthand evaluation:** The spec does not state whether `Foo { x }` is
   syntactic sugar evaluated at the point of the struct expression, or whether
   `x` can be reordered. Galvanic evaluates shorthand fields in source order.

2. **Update syntax type:** `Foo { a: 1, ..base }` — the FLS does not enumerate
   which types are copyable through struct update syntax. Galvanic copies all
   non-overridden fields as stack loads/stores; no move semantics are enforced.

**Source:** `src/ast.rs:1093`, `src/ast.rs:1272`

---

## §6.15.1 — For Loop: IntoIterator Desugaring

**Gap:** The FLS desugars `for x in expr { body }` via `IntoIterator::into_iter(expr)`,
but does not specify how a compiler without a trait system should handle the
common cases (`&[T]`, `&mut [T]`, arrays). The spec also does not specify
whether `for x in &arr` yields `&T` or `T`.

**Galvanic's choice:** Special-cases four forms without runtime trait dispatch:
- `for x in arr` (owned array): yields `i32` by value.
- `for x in &arr` (immutable borrow): yields `i32` by value (copy semantics).
- `for x in &mut arr` (mutable borrow): yields element address, allows `*x = v`.
- `for x in slice` (slice reference): yields elements by value.

The loop variable `x` holds the element value, not a reference. This satisfies
observable behavior but deviates from the spec's type-level model.

**Source:** `src/lower.rs:4710`, `src/lower.rs:15675`, `src/lower.rs:15830`

---

## §6.16 — Range Expressions: Value or Type?

**Gap:** The FLS defines range expressions (`a..b`, `a..=b`, `..`, etc.) as
producing values, but does not specify the runtime representation when ranges
are used as values (e.g., stored in a variable or passed to a function).

**Galvanic's choice:** Range expressions are only supported as loop bounds
in `for` loops (desugared inline). They are not supported as standalone values
that can be stored or passed. The parse fixture accepts them; codegen does not.

**Source:** `src/ast.rs:1148`

---

## §6.18 — Match Exhaustiveness

**Gap:** The FLS requires that match expressions be exhaustive but does not
specify the compiler mechanism for checking exhaustiveness or the behavior
if exhaustiveness is violated at runtime (the spec says it is a static error,
but provides no algorithm).

**Galvanic's choice:** A conservative compile-time exhaustiveness check is
implemented in `check_match_exhaustiveness` (src/lower.rs). The heuristic
accepts if any of the following holds:
1. Any arm (without guard) has a Wildcard, Ident, or single-segment struct
   pattern — trivially catches all values.
2. Both `true` and `false` literal patterns are present (bool exhaustiveness).
3. All declared variants of a known enum are covered by Path/TupleStruct/
   StructVariant patterns without guards (enum exhaustiveness).
4. Otherwise, if all patterns are integer/bool literals or ranges with no
   catch-all, the match is rejected as definitively non-exhaustive.
5. Patterns too complex to analyse (e.g., nested patterns, mixed types) are
   accepted conservatively (false negatives are acceptable; false positives
   are not).

**Remaining gap:** Complex pattern combinations (e.g., integer ranges that
together tile all i32 values, nested or-patterns with ranges) are not checked
and silently accepted. Full usefulness/completeness analysis is future work.

**Source:** `src/lower.rs` — `check_match_exhaustiveness` (inserted before the
`LowerCtx` impl block); called at all four match-lowering sites.

---

## §6.21 — Comparison Non-Associativity: Chained Comparisons

**Gap:** The FLS (§6.21:1) states that comparison operators (`<`, `<=`, `>`,
`>=`, `==`, `!=`) are non-associative, meaning `a < b < c` is not a valid
expression. However, the spec does not specify whether non-associativity is
enforced at the parser level (syntax error) or at the semantic level (type
error), nor does it describe the diagnostic.

In real Rust, `a < b < c` is a **parse error** — the parser itself rejects it.
Galvanic's recursive-descent parser does not yet enforce non-associativity at
the grammar level; it silently parses `a < b < c` as `(a < b) < c`, producing
an expression that compares a boolean (0 or 1) against `c`.

**Galvanic's choice (Claim 4n):** Enforce non-associativity at the lowering
stage (`src/lower.rs`) by detecting when the LHS of any comparison operator is
itself a comparison operator. Such expressions are rejected at compile time with
the diagnostic "chained comparison: FLS §6.21 — comparison operators are
non-associative". This matches the FLS requirement without requiring parser
changes. It catches the common case (`a < b < c`) but not explicitly
parenthesized forms (`a < (b < c)`), which would require type checking to
detect.

**Source:** `src/lower.rs` (comparison operator lowering, check added before
the f64/f32/i32 dispatch path)

---

## §7.1 — Const Evaluation Step Limit and Item Order

**Two gaps:**

1. **Step limit:** The FLS does not specify a maximum number of evaluation steps
   for const evaluation. Galvanic imposes no limit; unbounded recursion in const
   items will overflow the host stack.

2. **Evaluation order:** The FLS does not specify the order in which top-level
   `const` items are evaluated when one references another. Galvanic evaluates
   each const on first reference (lazy) within the same file.

**Source:** `src/lower.rs:565`, `src/lower.rs:1236`

---

## §7.2 — Static Data-Section Alignment

**Gap:** The FLS states all references to a static refer to the same memory
address but does not specify the required alignment for static data in the
output binary.

**Galvanic's choice:** Each static is placed in `.data` with `.align 3`
(8-byte alignment), matching the 64-bit register width. This is sufficient for
all supported types but is not mandated by the spec.

**Source:** `src/ast.rs:182`, `src/codegen.rs:156`

---

## §8.1 — Let Binding: Uninit Memory and Type Inference

**Two gaps:**

1. **Uninit memory:** The spec does not specify the memory layout for an
   uninitialized `let x;` binding — whether a stack slot is reserved, zeroed,
   or left undefined. Galvanic allocates a stack slot but emits no initializing
   store. The slot holds whatever the stack contained before.

2. **Type inference for uninitialized bindings:** The spec does not describe
   the inference algorithm for `let x;` followed by `x = expr;`. Galvanic
   infers the type from the first assignment site; if the assignment is missing,
   the binding has an unknown type and codegen may panic.

**Source:** `src/lower.rs:7634`, `src/lower.rs:9910`, `src/lower.rs:9999`

---

## §9 — Function Qualifier Ordering

**Gap:** The FLS lists `FunctionQualifiers` (`const`, `async`, `unsafe`,
`extern`) but does not specify whether they must appear in a fixed order or
whether all combinations are valid.

**Galvanic's choice:** The parser accepts qualifiers in any order and any
combination. Semantic restrictions (e.g., `const async` being invalid) are
not enforced at this milestone.

**Source:** `src/ast.rs:242`, `src/parser.rs:338`

---

## §10.1 — Method and Associated Function Calling Convention

**Two gaps:**

1. **Self parameter:** The FLS lists `self`, `&self`, `&mut self`, and
   `self: Type` forms but does not specify the calling convention — whether
   `self` is passed in x0 by value, by pointer, or through a dedicated slot.
   Galvanic passes `self` by address for struct receivers (pointer in x0).

2. **Struct return discarding:** When a method returns a struct that the caller
   ignores, the spec does not specify whether the callee still writes to the
   hidden output pointer. Galvanic always writes; the caller allocates the
   space.

**Source:** `src/ast.rs:311`, `src/lower.rs:3675`, `src/codegen.rs:878`,
`src/lower.rs:17800`

---

## §11 — `impl` Generics and `unsafe impl`

**Two gaps:**

1. **Generic impl disambiguation:** The spec allows `impl<T>` with generic
   parameters but does not specify how a compiler disambiguates `impl<T> Foo<T>`
   (generic impl) from `impl Foo<SomeType>` (concrete impl) when `SomeType`
   happens to be a single-letter identifier.

2. **`unsafe impl` enforcement:** The FLS states `unsafe impl` signals the
   implementor satisfies safety invariants, but does not specify what a compiler
   must check when `unsafe impl` is used. Galvanic parses `unsafe impl` but
   enforces nothing.

**Source:** `src/ast.rs:384`, `src/ast.rs:388`

---

## §12.1 — Generic `>>` Token Disambiguation

**Gap:** In generic argument lists like `Vec<Vec<i32>>`, the `>>` is lexed as
a single `Shr` token. The FLS does not specify the disambiguation rule for
splitting `>>` into two `>` tokens at the parser level.

**Galvanic's choice:** When parsing a generic argument list and the depth
counter reaches 1, a `>>` token is split: the first `>` closes the inner
generic, the second `>` is re-examined by the outer context. This is tracked
via a "pending GT" flag in the parser.

**Source:** `src/parser.rs:367`, `src/parser.rs:394`, `src/parser.rs:518`

---

## §15 — Discriminant Default Values and Drop Order

**Two gaps:**

1. **Discriminant defaults:** The FLS specifies that enum discriminants default
   to one more than the previous variant (starting at 0) but does not specify
   the behavior when a variant is given an explicit discriminant that collides
   with an implicit one.

2. **Drop order:** The FLS describes drop semantics (§15) but does not specify
   the exact drop order for locals within a block when multiple locals go out
   of scope. Galvanic emits no drop calls (no destructor support).

**Source:** `src/lower.rs:10564`, `src/lower.rs:3782`

---

## §6.21 / §6.7 — Comparison Operator Non-Associativity

**Gap:** The FLS states that comparison operators (`<`, `>`, `<=`, `>=`, `==`,
`!=`) are non-associative (chaining `a < b < c` is a parse error), but does
not define the parser rule precisely — how many comparison operators may appear
in a single expression before the error is triggered.

**Galvanic's choice:** Comparison operators are left-associative at the grammar
level (like other binary operators). A chained comparison `a < b < c` parses
successfully but produces incorrect results at runtime. Enforcement of non-
associativity is deferred.

**Source:** `src/parser.rs:2119`, `src/parser.rs:2270`

---

## §6.22 — Closure Capture ABI

**Gap:** The FLS specifies that closures capture variables from their
environment (§6.22) but does not specify the ABI — how captures are passed to
the closure body or returned, whether they are on the stack or in a heap-
allocated closure object.

**Galvanic's choice:** Capturing closures are lowered to a trampoline function.
Captured values are passed as hidden leading parameters (before the explicit
closure parameters). Mutable captures (`FnMut`) are passed by address;
immutable captures are passed by value.

**Source:** `src/lower.rs:18078`, `src/lower.rs:18173`

---

## §4.14 — Where-Clause Bounds: When Are They Checked?

**Gap:** The FLS does not specify whether where-clause bounds on trait, struct,
and enum definitions are checked at definition time, implementation time, or
monomorphization time. The spec also does not define how supertrait method
availability is resolved for concrete types at call sites.

**Galvanic's choice:**
- Supertrait method availability: resolved naturally at monomorphization;
  `t.base_method()` on a generic `T: Derived` resolves to `T__base_method`,
  which exists because the concrete type implements the supertrait.
- Where-clause bounds on struct/trait/enum definitions: parsed and stored but
  not checked at parse time, type-check time, or monomorphization. The bound
  is present in the AST but has no enforcement mechanism at this milestone.

**Source:** `src/parser.rs:719`, `src/parser.rs:744`, `src/parser.rs:858`,
`src/parser.rs:1133`, `src/parser.rs:1226`

---

## §6.10 — Tuple Return Calling Convention

**Gap:** The FLS defines tuple expressions as producing values but does not
specify the ABI for returning tuples from functions — which registers carry
which elements, or whether tuples are returned on the stack.

**Galvanic's choice:** Extends the struct-return convention: element[i] is
returned in register x{i}. For tuples with more than 8 elements (beyond x0–x7),
this would overflow the register set; only tuples up to 8 elements are currently
supported. This is consistent with the general struct-return convention but is
not mandated by the spec.

**Source:** `src/lower.rs:1923`, `src/lower.rs:3824`

---

## §6.13 — Field Access on Temporary Expressions

**Gap:** The FLS does not specify whether field access on a temporary
(non-place) expression is well-formed, or how the compiler should handle the
lifetime of the temporary.

**Galvanic's choice:** Field access is restricted to named local variables and
chained field access expressions. Temporary struct values returned from
function calls are not yet supported as receivers for field access — the
caller must assign to a named binding first.

**Source:** `src/lower.rs:17213`

---

## §6.14 — Inner Function Name Visibility

**Gap:** The FLS does not distinguish inner functions from closures in terms
of name visibility or calling convention. The spec's treatment of nested
function definitions is under §9 (functions), not §6.14 (closures), but the
distinction is not explicit.

**Galvanic's choice:** Inner function names are direct-call targets compiled
to a separate label (not `blr` indirect dispatch). They are visible only
within the enclosing function body. Closures use trampoline dispatch (`blr`);
inner functions use direct call (`bl`).

**Source:** `src/lower.rs:10101`, `src/parser.rs:3061`

---

## §6.12.2 — Method Auto-Deref Step Limit

**Gap:** The FLS does not specify how many auto-deref steps are legal for
method call receivers, or how auto-deref interacts with `Deref` trait
implementations.

**Galvanic's choice:** Zero auto-deref steps: the receiver must already be
the correct struct type. Method calls on references require explicit
dereferencing. Auto-deref is deferred to a future type-checking phase.

**Source:** `src/lower.rs:17388`, `src/ast.rs:1127`

---

## §6.15.6 — Break-with-Value: Syntactic or Semantic Restriction?

**Gap:** The FLS does not clearly distinguish whether the restriction that
`break expr` is only valid inside `loop` (not `while` or `for`) is a
syntactic constraint (parse error) or a semantic constraint (type error).

**Galvanic's choice:** `break expr` is parsed freely in any loop context.
The restriction is not enforced at the parse level; it is deferred to a
future semantic analysis phase. A `break 5` inside a `while` loop parses
successfully but has unspecified runtime behavior.

**Source:** `src/ast.rs:1242`

---

## §6.17 — Struct Literal Restriction in Condition Positions

**Gap:** The FLS does not explicitly enumerate the positions where struct
literal expressions are forbidden (e.g., `if`, `while`, `for` conditions).
The restriction exists in the Rust grammar but the FLS's treatment is implicit.

**Galvanic's choice:** The parser tracks a `restrict_struct_lit` flag that
is set when entering condition positions. When the flag is set, struct literal
syntax is rejected to avoid ambiguity with block delimiters. This matches
observed Rust behavior but the spec does not state it explicitly.

**Source:** `src/parser.rs:99`

---

## §9.2 — Irrefutable Patterns in Parameter Position

**Gap:** The FLS allows arbitrary irrefutable patterns in function parameter
position (e.g., `fn foo((a, b): (i32, i32))`) but does not enumerate which
patterns are valid there. The reader must cross-reference §5 (patterns)
without a direct statement of the intersection.

**Galvanic's choice:** Supports struct, tuple, and tuple-struct destructuring
patterns in parameter position. Slice patterns and or-patterns in parameter
position are not yet supported. Nested patterns in parameter position are
future work.

**Source:** `src/ast.rs:489`

---

## §10.2 — `Self::X` Projection Resolution in Default Methods

**Gap:** The FLS does not fully specify how `Self::X` associated type
projections are resolved when `Self` appears in a default method body or
trait method signature — specifically, whether resolution happens at
trait-definition time or impl-instantiation time.

**Galvanic's choice:** `Self::X` is resolved to the concrete associated type
registered in the impl block (or the trait's default) at codegen time.
Resolution is deferred until monomorphization; if no concrete type is known,
the projection fails at codegen.

**Source:** `src/parser.rs:1786`

---

## §14.1 — Valid Place Expressions for Assignment LHS

**Gap:** The FLS defines assignment expressions as requiring a place expression
on the left-hand side but does not enumerate which expression forms qualify as
place expressions. The categorization must be inferred from §6.1.4.

**Galvanic's choice:** Restricts assignment LHS to:
- Simple variable paths (`x = ...`)
- Field access chains (`s.field = ...`)
- Array index expressions (`arr[i] = ...`)
- Dereference expressions (`*ptr = ...`)

More complex LHS forms (e.g., tuple field assignment via `.0`, method calls
that return mutable references) are not supported at this milestone.

**Source:** `src/lower.rs:14302`, `src/lower.rs:14393`, `src/lower.rs:14604`

---

## §13 — Trait Definition Order and Default Method Bodies

**Two gaps:**

1. **Definition order:** The FLS does not specify whether a trait must be
   defined before its implementations within a crate. Standard Rust requires
   the trait to be defined first for type-checking, but the FLS is silent on
   ordering. Galvanic does not type-check at this milestone; traits and impls
   can appear in any order in the source file.

2. **Default method bodies:** The FLS allows trait methods to have default
   bodies (`fn foo(&self) -> i32 { 0 }`). The spec does not specify whether
   an impl that omits the method silently inherits the default, or whether
   some declaration is required. Galvanic resolves method calls to the
   concrete impl's body if present, otherwise falls back to the trait's
   default body — but the spec's resolution algorithm is not defined.

**Source:** `src/ast.rs:437`, `src/parser.rs:695`

---

## §14 — Visibility and Name Resolution

**Gap:** The FLS does not specify whether visibility modifiers on struct
definitions (`pub struct`) and on individual struct fields interact with
name resolution in a well-defined way across all contexts. For example,
the spec does not state what happens when a `pub(crate)` struct has private
fields accessed from a different module. Galvanic records visibility
annotations in the AST but defers enforcement to a future name-resolution
phase; all fields are currently accessible regardless of visibility.

**Source:** `src/ast.rs:576`, `src/ast.rs:661`

---

## §19 — Unsafety Enforcement Mechanism

**Three distinct gaps:**

1. **`unsafe fn` call enforcement:** The FLS requires that callers of `unsafe fn`
   use an unsafe context (an `unsafe { }` block or another `unsafe fn`). The
   spec does not specify the compiler mechanism for verifying this. Galvanic
   records the `is_unsafe` qualifier but defers call-site enforcement — no
   check is performed at this milestone.

2. **`unsafe impl` pairing:** The FLS states `unsafe impl` signals that the
   implementor satisfies the safety invariants of an unsafe trait, but does
   not specify how a compiler verifies that `unsafe impl Foo for Bar` only
   appears when `Foo` is declared `unsafe trait`. Galvanic parses both but
   does not verify the pairing.

3. **`unsafe trait` contract:** The spec defines an unsafe trait as one whose
   implementations may only be done via `unsafe impl`, but the enforcement
   mechanism is left to the implementation. Galvanic records `is_unsafe` on
   the `TraitDef` node and defers enforcement.

**Source:** `src/ast.rs:266`, `src/ast.rs:388`, `src/ast.rs:442`,
`src/parser.rs:229`, `src/parser.rs:243`, `src/parser.rs:255`

---

*Last updated: 2026-04-11. Source annotation count at time of writing: ~155 `AMBIGUOUS` markers across 6 source files. Covers all sections with annotations; three previously missing sections (§13, §14, §19) added in this revision.*
