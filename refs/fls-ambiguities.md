# FLS Ambiguity Findings — Galvanic

This document aggregates the `AMBIGUOUS` annotations scattered across
`src/lexer.rs`, `src/parser.rs`, `src/ast.rs`, `src/ir.rs`, `src/lower.rs`,
and `src/codegen.rs`. Each entry names the spec gap, galvanic's chosen
resolution, and the source location where the annotation lives.

Entries are grouped by FLS section in numeric order. Generated from annotations
present as of the commit that introduced this file; check the individual source
annotations for full context.

---

## Table of Contents

- [§2.4.4.1 — Integer Literals: Large-Immediate Encoding](#2441--integer-literals-large-immediate-encoding)
- [§2.4.4.2 — Float Literals: NaN, Infinity, Hex Floats](#2442--float-literals-nan-infinity-hex-floats)
- [§2.6 — Keyword Classification: `'static` and `_`](#26--keyword-classification-static-and-_)
- [§4.1 — Built-in Associated Constants (MIN, MAX, BITS)](#41--built-in-associated-constants-min-max-bits)
- [§4.2 — Struct-Typed Enum Variant Field Layout](#42--struct-typed-enum-variant-field-layout)
- [§4.2 / §2.4.5 — `char` Type Encoding](#42--245--char-type-encoding)
- [§4.8 / §4.9 — Fat Pointer ABI for `&str` and `&[T]`](#48--49--fat-pointer-abi-for-str-and-t)
- [§4.9 — Bounds Checking Mechanism](#49--bounds-checking-mechanism)
- [§4.13 — `dyn Trait` Vtable Layout and Fat Pointer Return ABI](#413--dyn-trait-vtable-layout-and-fat-pointer-return-abi)
- [§4.14 — Where-Clause Bounds: When Are They Checked?](#414--where-clause-bounds-when-are-they-checked)
- [§4.14 — Parenthesized Trait Bound Syntax: Restricted to Fn Traits?](#414--parenthesized-trait-bound-syntax-restricted-to-fn-traits)
- [§5.1.4 — Pattern Binding and Or-Patterns: Evaluation Order](#514--pattern-binding-and-or-patterns-evaluation-order)
- [§5.1.8 — Rest Patterns (`..`) Inside Slice Patterns](#518--rest-patterns--inside-slice-patterns)
- [§6.1.2 — Overflow in Const Contexts](#612--overflow-in-const-contexts)
- [§6.4.2 — Const Block: Permitted Expression Forms](#642--const-block-permitted-expression-forms)
- [§6.4.4 — Unsafe Block: Permitted vs Required Operations](#644--unsafe-block-permitted-vs-required-operations)
- [§6.5.3 — NaN Comparison Behavior](#653--nan-comparison-behavior)
- [§6.5.5 — Floating-Point: IEEE 754 Reference Without Encoding Details](#655--floating-point-ieee-754-reference-without-encoding-details)
- [§6.5.7 — Bitwise AND Disambiguation: & as Borrow vs Bit AND](#657--bitwise-and-disambiguation--as-borrow-vs-bit-and)
- [§6.5.7 — Shift Amount Modulo Behavior](#657--shift-amount-modulo-behavior)
- [§6.5.9 — Narrowing Integer Casts and Float-to-Int Casts](#659--narrowing-integer-casts-and-float-to-int-casts)
- [§6.9 / §6.23 — Panic Mechanism](#69--623--panic-mechanism)
- [§6.10 — Tuple Return Calling Convention](#610--tuple-return-calling-convention)
- [§6.11 — Struct Expression Shorthand and Update Syntax](#611--struct-expression-shorthand-and-update-syntax)
- [§6.12.2 — Method Auto-Deref Step Limit](#6122--method-auto-deref-step-limit)
- [§6.13 — Field Access on Temporary Expressions](#613--field-access-on-temporary-expressions)
- [§6.14 — Inner Function Name Visibility](#614--inner-function-name-visibility)
- [§6.15.1 — For Loop: IntoIterator Desugaring](#6151--for-loop-intoiterator-desugaring)
- [§6.15.6 — Break-with-Value: Syntactic or Semantic Restriction?](#6156--break-with-value-syntactic-or-semantic-restriction)
- [§6.16 — Range Expressions: Value or Type?](#616--range-expressions-value-or-type)
- [§6.17 — Struct Literal Restriction in Condition Positions](#617--struct-literal-restriction-in-condition-positions)
- [§6.18 — Match Exhaustiveness](#618--match-exhaustiveness)
- [§6.21 — Comparison Non-Associativity: Chained Comparisons](#621--comparison-non-associativity-chained-comparisons)
- [§6.22 — Closure Capture ABI](#622--closure-capture-abi)
- [§7.1 — Const Evaluation Step Limit and Item Order](#71--const-evaluation-step-limit-and-item-order)
- [§7.2 — Static Data-Section Alignment](#72--static-data-section-alignment)
- [§8.1 — Let Binding: Uninit Memory and Type Inference](#81--let-binding-uninit-memory-and-type-inference)
- [§9 — Function Qualifier Ordering](#9--function-qualifier-ordering)
- [§9.2 — Irrefutable Patterns in Parameter Position](#92--irrefutable-patterns-in-parameter-position)
- [§10.1 — Method and Associated Function Calling Convention](#101--method-and-associated-function-calling-convention)
- [§10.2 — `Self::X` Projection Resolution in Default Methods](#102--selfx-projection-resolution-in-default-methods)
- [§11 — `impl` Generics and `unsafe impl`](#11--impl-generics-and-unsafe-impl)
- [§12.1 — Generic `>>` Token Disambiguation](#121--generic--token-disambiguation)
- [§13 — Trait Definition Order and Default Method Bodies](#13--trait-definition-order-and-default-method-bodies)
- [§14 — Visibility and Name Resolution](#14--visibility-and-name-resolution)
- [§14.1 — Valid Place Expressions for Assignment LHS](#141--valid-place-expressions-for-assignment-lhs)
- [§15 — Discriminant Default Values and Drop Order](#15--discriminant-default-values-and-drop-order)
- [§19 — Unsafety Enforcement Mechanism](#19--unsafety-enforcement-mechanism)

---

## §2.4.4.1 — Integer Literals: Large-Immediate Encoding

**Gap:** The FLS specifies integer literal syntax and types but does not describe
how a compiler should encode integers that exceed 16 bits in ARM64 assembly.

**Galvanic's choice:** Integers that fit in 16 bits use `mov x0, #N`. Larger
values that fit in 32 bits use `movz`/`movk` pairs. Values requiring 64-bit
encoding use up to four `movz`/`movk` instructions. This is standard ARM64
practice but is not mandated by the spec.

**Source:** `src/lower.rs` (search `MOVZ+MOVK`)

**Minimal reproducer:**
```rust
fn main() -> i32 { 65536 }
```
Assembly signature: look for `movz x0, #1, lsl #16` (a 32-bit value that cannot
be encoded in a single 16-bit immediate — requires `movz` with a shift).

---

## §2.4.4.2 — Float Literals: NaN, Infinity, Hex Floats

**Gap:** The FLS does not specify the handling of NaN/infinity literal forms
(Rust has none) or hexadecimal float literals (e.g. `0x1.fp10`).

**Galvanic's choice:** Decimal float literals with optional `f32`/`f64` or
`_f32`/`_f64` suffix are supported (both `1.0f64` and `1.0_f64` parse correctly,
per FLS §2.4.4.2). NaN/infinity are not expressible as literals. Hex floats
are rejected at the lexer level.

**Source:** `src/lower.rs:3968`

**Minimal reproducer:**
```rust
fn add(a: f64, b: f64) -> f64 { a + b }
fn main() -> i32 { let _ = add(1.0_f64, 3.14_f64); 0 }
```
Assembly signature: look for `fadd d0, d0, d1` — confirms decimal f64 literals
are loaded and operated on via D-registers. NaN/infinity/hex-float are not valid
Rust literal forms at any version; their absence is not galvanic-specific.

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

**Minimal reproducer:** Not demonstrable via assembly — lexer token-stream
distinctions (`Underscore` vs `Ident`) are not reflected in ARM64 machine code.
The finding is observable in the token stream, not the emitted output.

---

## §4.1 — Built-in Associated Constants (MIN, MAX, BITS)

**Gap:** The FLS does not enumerate which associated constants are built into
primitive integer types (e.g. `i32::MAX`, `u8::MIN`, `i32::BITS`).

**Galvanic's choice:** Recognizes `MIN`, `MAX`, and `BITS` for all supported
integer and float types as compile-time constants resolved during lowering.
The set is chosen to match observed Rust usage, not a spec-defined list.

**Source:** `src/lower.rs:1335`

**Minimal reproducer:**
```rust
fn main() -> i32 { i32::MAX }
```
Assembly signature: look for `movz`/`movk` sequence or `mov x0, #...` loading
`2147483647` (0x7FFF_FFFF) — confirms `i32::MAX` is resolved to its value at
compile time rather than requiring a runtime lookup.

---

## §4.2 — Struct-Typed Enum Variant Field Layout

**Gap:** The FLS does not specify the in-memory layout for enum variant fields
that are struct types (e.g. `Maybe::Some(Foo { x: 7 })`). Specifically, it
does not say whether the struct's fields are stored inline at the variant field's
slot, or whether an indirection (pointer) is used.

**Galvanic's choice:** Struct fields are stored inline starting at the variant
field's slot. For `Maybe::Some(Foo { x: 7 })` where `Maybe` has one variant
field, the discriminant is at `base_slot`, the struct's first field (`x = 7`)
is at `base_slot + 1`. This matches the nested-struct literal lowering used for
regular struct fields (`store_nested_struct_lit`).

**Source:** `src/lower.rs` — enum tuple variant constructor loop, `_` arm
(AMBIGUOUS annotation in the struct-literal branch).

**Minimal reproducer:**
```rust
struct Foo { x: i32 }
enum Maybe<T> { Some(T), None }
fn main() -> i32 {
    let m = Maybe::Some(Foo { x: 7 });
    match m { Maybe::Some(_) => 1, Maybe::None => 0 }
}
```
Assembly signature: look for `str w<N>, [sp, #<offset>]` after storing the
discriminant — confirms `x = 7` is stored inline in the variant's field slot,
not via a pointer. The wildcard arm (`_`) is used instead of `v => v.x`
because field access on a match-bound variable extracted from an enum variant
is not yet supported (§6.13 limitation: field access is restricted to named
local variables). The inline-storage invariant is confirmed by the `str` for
the struct field; a pointer-indirection layout would emit a different store
sequence.

---

## §4.2 / §2.4.5 — `char` Type Encoding

**Gap:** The FLS describes `char` as "the Unicode scalar value type" but does
not provide a section number in the FLS TOC that specifies its in-memory
representation. §2.4.5 covers char literal syntax but is absent from the
main TOC.

**Galvanic's choice:** Maps char literals to their Unicode code point as a
`u32` (4 bytes). Stored and loaded as 32-bit integers on the stack.

**Source:** `src/lower.rs:4000`

**Minimal reproducer:**
```rust
fn main() -> i32 { 'A' as i32 }
```
Assembly signature: look for `mov w0, #65` — confirms `'A'` is the Unicode
code point U+0041 = 65, stored as a 32-bit integer (not a wider type).

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

**Minimal reproducer:**
```rust
fn byte_len(s: &str) -> usize { s.len() }
fn main() -> i32 { byte_len("hi") as i32 }
```
Assembly signature: look for x0 holding the string pointer and x1 holding the
byte count (`#2` for "hi") arriving as separate register arguments — confirms
the two-slot fat pointer (base, length) ABI.

---

## §4.9 — Bounds Checking Mechanism

**Gap:** The FLS requires that indexing out of bounds panics (§6.9), but does
not specify the panic mechanism — whether it is a library call, a trap
instruction, or something else.

**Galvanic's choice (current):** A `cmp`/`b.hs` bounds check is emitted
before every array and slice index access (Claims 4m/4p). Out-of-bounds
access branches to `_galvanic_panic`, which calls `exit(101)` via a bare
Linux syscall (`svc #0` with `x8=93`). No stack unwinding, no panic message.
See also the §6.9/§6.23 entry for the full mechanism.

**Historical note:** Prior to Claims 4m/4p, no bounds check was emitted.
Out-of-bounds access produced undefined behavior at the assembly level.
That deviation is resolved; this entry documents the gap (what the FLS
leaves unspecified) and galvanic's current resolution.

**Source:** `src/ir.rs:730`, `src/codegen.rs:926`, `src/lower.rs:17880`

**Minimal reproducer:**
```rust
fn get(arr: [i32; 3], i: usize) -> i32 { arr[i] }
```
Assembly signature: `cmp x1, #3` / `b.hs <trap>` before the `ldr`.

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

**Minimal reproducer:**
```rust
trait Sound { fn call(&self) -> i32; }
struct Dog;
impl Sound for Dog { fn call(&self) -> i32 { 1 } }
fn dispatch(a: &dyn Sound) -> i32 { a.call() }
fn main() -> i32 { let d = Dog; dispatch(&d) }
```
Assembly signature: look for `ldr x8, [x1]` (load vtable pointer from second
slot of fat pointer) followed by `blr x8` (indirect dispatch) — confirms
vtable at offset 0 with no destructor slot preceding it.

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

**Minimal reproducer:** The fixture `tests/fixtures/fls_4_14_where_clauses_on_types.rs`
exercises where-clause-bounded structs, enums, and trait impls. At this milestone the
file partially compiles (struct literal args in enum variant constructors are now
lowered). The enforcement mechanism for where-clause bounds (or its absence) is not
observable in assembly output.

---

## §4.14 — Parenthesized Trait Bound Syntax: Restricted to Fn Traits?

**Gap:** FLS §4.14 introduces parenthesized trait bound syntax (`Fn(T) -> R`,
`FnMut(T) -> R`, `FnOnce(T) -> R`) as a shorthand for angle-bracket generics
on the `Fn`/`FnMut`/`FnOnce` traits. The spec does not state whether this
parenthesized syntax is restricted to those three traits or is syntactically
valid for any trait name followed by `(...)`.

**Galvanic's choice:** Accept parenthesized syntax for any trait name at the
parse level. Semantic restriction to `Fn`/`FnMut`/`FnOnce` is deferred to a
future type-checking phase. This matches rustc's grammar, which treats
`TraitName(Args) -> Ret` as a `TraitObjectBound` regardless of the trait name,
and rejects non-Fn uses in a later semantic pass rather than the parser.

**Source:** `src/parser.rs` — `'bound_loop` in `parse_fn_def`,
`'impl_bound_loop` in `parse_impl_def`, and the bound loop in
`parse_where_clause` (all annotated `AMBIGUOUS: §4.14`).

**Minimal reproducer:**
```rust
fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 { f(x) }
fn main() -> i32 { apply(|x| x * 2, 5) }
```
Both the inline bound `F: Fn(i32) -> i32` and a `where F: Fn(i32) -> i32`
form parse successfully. The assembly contains a `blr` indirect call,
confirming the closure dispatch path is exercised at runtime.

---

## §5.1.4 — Pattern Binding and Or-Patterns: Evaluation Order

**Gap:** The FLS does not specify the order in which alternatives in an
or-pattern are evaluated, or whether identically-named bindings in different
alternatives must have the same type (enforcement mechanism unspecified).

**Galvanic's choice:** Alternatives are evaluated left-to-right at runtime.
Each alternative that matches stores to the same binding slot (identified by
name). Type consistency is not verified at this milestone.

**Source:** `src/ast.rs:1733`, `src/lower.rs:7821`, `src/parser.rs:3409`

**Minimal reproducer:**
```rust
fn classify(x: i32) -> i32 {
    match x { 1 | 2 => 10, _ => 0 }
}
```
Assembly signature: look for two separate comparisons (`cmp x0, #1` then
`cmp x0, #2`) before the `mov x0, #10` branch — confirms left-to-right
alternative evaluation with a separate branch for each alternative.

---

## §5.1.8 — Rest Patterns (`..`) Inside Slice Patterns

**Gap:** The FLS allows `..` inside slice patterns (`[a, .., b]`) but does
not specify the evaluation order or how many elements the rest pattern
consumes when the slice has fewer elements than the pattern's fixed positions.

**Galvanic's choice:** The rest pattern consumes zero or more elements from
the middle. Pattern match fails if the slice is shorter than the fixed
elements (a + b positions). No elements are bound from the rest.

**Source:** `src/parser.rs:3612`

**Minimal reproducer:** Not yet demonstrable — rest patterns inside slice
patterns are parsed but not compiled end-to-end at this milestone. The
behavior (element count check + head/tail loads) is not observable in
assembly output until full slice pattern lowering is implemented.

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

**Minimal reproducer:**
```rust
const C: i32 = 1 + 2;
fn main() -> i32 { C }
```
Assembly signature: look for `mov x0, #3` in `main` — confirms the const
expression was evaluated at compile time and the result emitted as an
immediate. Contrast with `fn add(a: i32, b: i32) -> i32 { a + b }` which
emits runtime `add w0, w0, w1` instead.

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

**Minimal reproducer:**
```rust
fn main() -> i32 { const { 2 + 3 } }
```
Assembly signature: look for `mov x0, #5` — confirms the const block is
evaluated at compile time and the result (`5`) is emitted as an immediate
rather than a runtime `add` instruction.

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

**Minimal reproducer:**
```rust
fn main() -> i32 { unsafe { 42 } }
```
Assembly signature: identical to `fn main() -> i32 { 42 }` — emits `mov x0, #42`
with no safety overhead. No warning is produced, confirming the `unsafe` keyword
is a no-op for assembly output when the block contains only safe operations.

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

**Minimal reproducer:**
```rust
fn main() -> i32 {
    let x: f64 = 0.0_f64 / 0.0_f64;
    if x != x { 1 } else { 0 }
}
```
Assembly signature: look for `fdiv d0, d0, d1` (runtime divide producing NaN)
followed by `fcmp d0, d0` then `cset w0, ne` — confirms NaN != NaN yields 1
because `fcmp` raises the unordered flag, and ARM64 `ne` condition is true when
the unordered flag is set.

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

**Minimal reproducer:**
```rust
fn add_f64(a: f64, b: f64) -> f64 { a + b }
fn add_f32(a: f32, b: f32) -> f32 { a + b }
```
Assembly signature: `add_f64` emits `fadd d0, d0, d1` (D-registers = binary64);
`add_f32` emits `fadd s0, s0, s1` (S-registers = binary32) — confirms the
encoding choice (binary64 vs binary32) is implicit in the register width.

---

## §6.5.7 — Bitwise AND Disambiguation: & as Borrow vs Bit AND

**Gap:** FLS §6.5.7 defines `&` as a bitwise AND operator (Bit Expressions),
while FLS §6.5.1 defines `&` as a borrow operator (Borrow Expression). The spec
defines both uses but does not specify how a recursive-descent parser should
distinguish them when `&` appears in expression position.

**Galvanic's choice:** Disambiguation is positional. The parser is structured as
a precedence-climbing descent: `parse_bitand` is only entered after a complete
left-hand operand has been successfully parsed at a higher-precedence level. At
that point, `&` can only be a binary infix operator (bitwise AND). Borrow
expressions (`&expr`, `&mut expr`) are handled in `parse_unary`, which runs
before the binary precedence layer — so `&` in unary position is always
consumed as a borrow before `parse_bitand` is reached.

**Source:** `src/parser.rs` — `fn parse_bitand` (search `FLS §6.5.7 AMBIGUOUS`)

**Minimal reproducer:**
```rust
fn bitand(a: i32, b: i32) -> i32 { a & b }
fn borrow_ref(x: &i32) -> i32 { *x }
```
`galvanic bitand.rs` must emit `and w0, w0, w1` (bitwise AND instruction).
`galvanic borrow_ref.rs` must emit a `ldr` from the argument register (borrow).
The parser must not confuse the two uses of `&`.

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

**Minimal reproducer:**
```rust
fn shl(x: i64, n: i64) -> i64 { x << n }
```
Assembly signature: look for `cmp x1, #64` followed by `b.hs _galvanic_panic`
then `lsl x2, x0, x1` — galvanic panics for shift amounts ≥ 64 rather than
relying on hardware mod-64 wrapping. There is no `and x1, x1, #63` masking
instruction. The ambiguity remains for shifts of narrower types stored in 64-bit
registers, where hardware mod-64 could produce surprising results.

Note: the `**Galvanic's choice**` description above is stale — galvanic now
emits a range guard (panic if n ≥ 64), not a bare `lsl` relying on hardware
behavior.

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

**Minimal reproducer (integer narrowing):**
```rust
fn narrow(x: i32) -> i32 { (x as u8) as i32 }
```
Assembly signature: look for `and w0, w0, #255` — confirms low-8-bit truncation
for `as u8` (the `AND` instruction masks off the upper 24 bits).

**Minimal reproducer (float-to-int):**
```rust
fn f2i(x: f64) -> i32 { x as i32 }
```
Assembly signature: look for `fcvtzs w0, d0` — confirms saturating conversion
(out-of-range values clamp to INT_MIN/INT_MAX per ARM64 `FCVTZS` semantics,
not the FLS-specified truncation behavior).

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
- `+`, `-`, `*` overflow: no overflow check; arithmetic wraps per 64-bit
  hardware. This is a known deviation from debug-mode Rust semantics.
  FLS §6.23 AMBIGUOUS — spec requires debug-mode panic but galvanic uses 64-bit
  arithmetic throughout and does not insert overflow checks for these operators.

The panic primitive `_galvanic_panic` calls `sys_exit(101)` directly. No stack
unwinding, no panic message. This matches the FLS requirement (panics terminate
the program) while keeping the implementation simple.

**Source:** `src/lower.rs` (literal zero check),
`src/codegen.rs` (cbz, MIN/-1 guard, bounds check, `_galvanic_panic`)

**Minimal reproducer (divide-by-zero guard):**
```rust
fn div(x: i32, y: i32) -> i32 { x / y }
```
Assembly signature: look for `cbz x1, _galvanic_panic` immediately before
`sdiv x0, x0, x1` — confirms the runtime zero-divisor guard.

**Minimal reproducer (MIN/-1 overflow guard):**
```rust
fn div_min(y: i32) -> i32 { i32::MIN / y }
```
Assembly signature: look for `movz`/`sxtw` loading `i32::MIN` then `cmp`/`cmn`
followed by a conditional `b _galvanic_panic` before `sdiv` — confirms the
signed overflow guard.

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

**Minimal reproducer:**
```rust
fn pair() -> (i32, i32) { (10, 20) }
```
Assembly signature: look for `mov x0, #10` and `mov x1, #20` in the function
body — confirms element[0] in x0 and element[1] in x1, following the
"element[i] in register x{i}" convention.

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

**Minimal reproducer:**
```rust
struct Point { x: i32, y: i32 }
fn make(x: i32, y: i32) -> Point { Point { x, y } }
```
Assembly signature: look for two consecutive stores to the Point stack slot —
the x field stored before the y field — confirming shorthand fields are
evaluated in source order.

---

## §6.12.2 — Method Auto-Deref Step Limit

**Gap:** The FLS does not specify how many auto-deref steps are legal for
method call receivers, or how auto-deref interacts with `Deref` trait
implementations.

**Galvanic's choice:** Zero auto-deref steps: the receiver must already be
the correct struct type. Method calls on references require explicit
dereferencing. Auto-deref is deferred to a future type-checking phase.

**Source:** `src/lower.rs:17388`, `src/ast.rs:1127`

**Minimal reproducer:**
```rust
struct Wrap(i32);
impl Wrap { fn val(&self) -> i32 { self.0 } }
fn main() -> i32 { let w = Wrap(7); w.val() }
```
Assembly signature: look for `add x0, sp, #N` (address of `w` passed in x0)
before `bl Wrap__val` — confirms the receiver is the struct itself (no
auto-deref step). Calling `w.val()` on `&Wrap` without explicit `*w` would
require the auto-deref that is not yet implemented.

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

**Minimal reproducer:** Not demonstrable via assembly — the finding manifests as
a compile error, not assembly output. `fn make() -> Point { ... }; make().x`
is rejected by the lowering stage before any code is emitted. Assign to a
binding first: `let p = make(); p.x` works correctly.

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

**Minimal reproducer:**
```rust
fn outer() -> i32 {
    fn inner() -> i32 { 7 }
    inner()
}
```
Assembly signature: look for `bl inner` (direct call, not `blr`) in `outer`'s
body and a separate `inner:` function label — confirms inner functions use
direct-call dispatch, not the closure trampoline (`blr xN`).

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

**Minimal reproducer:**
```rust
fn sum(arr: [i32; 3]) -> i32 {
    let mut s = 0;
    for x in arr { s = s + x; }
    s
}
```
Assembly signature: look for a loop counter increment and element `ldr` without
any `bl IntoIterator__into_iter` call — confirms special-cased desugaring that
bypasses the trait dispatch the FLS prescribes.

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

**Minimal reproducer:**
```rust
fn main() -> i32 { loop { break 42; } }
```
Assembly signature: look for `mov x0, #42` followed by `b` to the function
epilogue — confirms break-with-value in `loop` sets the loop result and exits.
For the ambiguity: `while true { break 42; }` also compiles without error,
demonstrating that the syntactic restriction is not enforced.

---

## §6.16 — Range Expressions: Value or Type?

**Gap:** The FLS defines range expressions (`a..b`, `a..=b`, `..`, etc.) as
producing values, but does not specify the runtime representation when ranges
are used as values (e.g., stored in a variable or passed to a function).

**Galvanic's choice:** Range expressions are only supported as loop bounds
in `for` loops (desugared inline). They are not supported as standalone values
that can be stored or passed. The parse fixture accepts them; codegen does not.

**Source:** `src/ast.rs:1148`

**Minimal reproducer:**
```rust
fn sum_to_five() -> i32 {
    let mut s = 0;
    for i in 0..5 { s = s + i; }
    s
}
```
Assembly signature: look for loop counter starting at 0 and a `cmp x0, #5`
(upper bound comparison) — confirms `0..5` is desugared inline as loop bounds.
Attempting `let r = 0..5` (standalone range value) emits a compile error,
confirming ranges are not supported as first-class values.

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

**Minimal reproducer:** Not demonstrable via assembly — enforced at the parser
level as a syntax error. `if Foo { x: 1 } { bar() }` emits a parse error
before any code is generated, confirming the `restrict_struct_lit` flag fires.

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

**Minimal reproducer:**
```rust
fn classify(x: i32) -> i32 {
    match x { 0 => 1, _ => 2 }
}
```
Assembly signature: look for `cmp x0, #0` + conditional branch to two arms —
confirms runtime match dispatch. The wildcard arm (`_`) triggers the
exhaustiveness heuristic's rule 1. A match on integer with no wildcard
(e.g. `match x { 0 => 1, 1 => 2 }`) emits a compile error: "match may not
be exhaustive".

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

**Minimal reproducer:**
```rust
fn bad(a: i32, b: i32, c: i32) -> bool { a < b < c }
```
Assembly signature: no assembly is emitted — the compiler exits with error
"chained comparison: FLS §6.21 — comparison operators are non-associative".
Run `cargo run -- /tmp/bad.rs` and observe the error on stderr.

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

**Minimal reproducer:**
```rust
fn apply(f: impl Fn() -> i32) -> i32 { f() }
fn main() -> i32 {
    let x = 5;
    apply(|| x)
}
```
Assembly signature: look for a trampoline function label (e.g.
`__closure_trampoline_0:`) in the assembly and `x` passed as a hidden leading
register argument before the closure dispatch — confirms captured values are
hidden leading parameters, not heap-allocated.

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

**Minimal reproducer:**
```rust
const A: i32 = 1 + 2;
const B: i32 = A * 3;
fn main() -> i32 { B }
```
Assembly signature: look for `mov x0, #9` in `main` — confirms lazy const
evaluation (B resolved to 9 by referencing A at compile time). No step-limit
guard is emitted; a const item that would loop infinitely would hang the
compiler.

---

## §7.2 — Static Data-Section Alignment

**Gap:** The FLS states all references to a static refer to the same memory
address but does not specify the required alignment for static data in the
output binary.

**Galvanic's choice:** Each static is placed in `.data` with `.align 3`
(8-byte alignment), matching the 64-bit register width. This is sufficient for
all supported types but is not mandated by the spec.

**Source:** `src/ast.rs:182`, `src/codegen.rs:156`

**Minimal reproducer:**
```rust
static X: i32 = 42;
fn main() -> i32 { X }
```
Assembly signature: look for `.align 3` immediately before the `X:` label in
the `.data` section of the emitted `.s` file — confirms 8-byte alignment
regardless of the static's natural alignment (i32 only requires 4 bytes).

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

**Minimal reproducer:**
```rust
fn foo(cond: bool) -> i32 {
    let x;
    if cond { x = 1; } else { x = 2; }
    x
}
```
Assembly signature: look for a stack slot allocated in the prologue
(`sub sp, sp, #N`) with **no** initializing store before the conditional
branches — confirms the slot is allocated but not zeroed, matching the
"uninit" choice.

---

## §9 — Function Qualifier Ordering

**Gap:** The FLS lists `FunctionQualifiers` (`const`, `async`, `unsafe`,
`extern`) but does not specify whether they must appear in a fixed order or
whether all combinations are valid.

**Galvanic's choice:** The parser accepts qualifiers in any order and any
combination. Semantic restrictions (e.g., `const async` being invalid) are
not enforced at this milestone.

**Source:** `src/ast.rs:242`, `src/parser.rs:338`

**Minimal reproducer:** Not directly observable via assembly — the finding is
parser-level permissiveness. To verify: a file containing
`const unsafe fn add(a: i32, b: i32) -> i32 { a + b }` (unusual ordering,
normally written `unsafe const fn`) compiles without a parse error and emits
normal function assembly.

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

**Minimal reproducer:**
```rust
fn add((a, b): (i32, i32)) -> i32 { a + b }
```
Assembly signature: look for the two input integers arriving in x0 and x1
being stored to separate named stack slots (`a` and `b`) before
`add w0, w0, w1` — confirms tuple destructuring in parameter position maps
to the standard two-argument calling convention.

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

**Minimal reproducer:**
```rust
struct Point { x: i32, y: i32 }
impl Point { fn sum(&self) -> i32 { self.x + self.y } }
fn main() -> i32 { let p = Point { x: 3, y: 4 }; p.sum() }
```
Assembly signature: look for `add x0, sp, #N` (address of `p` loaded into x0)
before `bl Point__sum` — confirms `&self` is passed as a pointer in x0, not
a copy of the struct value.

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

**Minimal reproducer:** Demonstrable — `tests/fixtures/fls_10_2_assoc_types.rs`
compiles end-to-end. Key assembly signature: `mul` instructions in
`Square__scaled_area` and `Rectangle__scaled_area` (runtime arithmetic). Run:

```
cargo run -- tests/fixtures/fls_10_2_assoc_types.rs
grep 'mul\|bl ' tests/fixtures/fls_10_2_assoc_types.s
```

The `type Area = i32` binding is resolved at codegen; galvanic ignores the
associated type alias itself and uses the concrete type from the impl block.
`Self::X` projection in return types is still deferred (future work).

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

**Minimal reproducer:** Demonstrable — `tests/fixtures/fls_11_impl_trait.rs`
compiles end-to-end. Key assembly signature: `bl extract__Num` (monomorphized
call site) and `bl Num__get` (trait dispatch via name mangling). Run:

```
cargo run -- tests/fixtures/fls_11_impl_trait.rs
grep 'bl ' tests/fixtures/fls_11_impl_trait.s
```

Generic `impl<T>` disambiguation is observable via `tests/fixtures/fls_12_1_generic_trait_impl.rs`,
which emits `bl Wrapper_i32__get` (monomorphized at `T=i32`). `unsafe impl`
enforcement remains a parser-only concern — galvanic parses but does not enforce.

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

**Minimal reproducer:** The `>>` split in type-annotation position fails to
parse at this milestone. `fls_12_1_generic_trait_impl.rs` has compiled
end-to-end since cycle 011 — the parse-only attribution was stale. The actual
blocker is narrower: `>>` in a `let` type annotation (`let w: Outer<Inner<i32>>`)
triggers a parse error. Run to confirm:

```
echo 'struct W<T>{inner:T} fn main()->i32{let _:W<W<i32>>=W{inner:W{inner:0}};0}' \
  | cargo run -- /dev/stdin
# error: parse error at byte N: expected Semi, found Eof
```

The `>>` split is implemented for generic parameter lists in `impl<T>` and
`fn foo<T>` positions (`src/parser.rs:367,394,518`), but not for type
annotations in `let` bindings. The disambiguation rule is unspecified in the FLS.

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

**Minimal reproducer:**
```rust
trait Animal { fn sound(&self) -> i32; }
struct Cat;
impl Animal for Cat { fn sound(&self) -> i32 { 2 } }
fn main() -> i32 { let c = Cat; c.sound() }
```
Assembly signature: look for `bl Cat__sound` — confirms trait method dispatch
resolves to the concrete impl. To test definition order: place `impl Animal for
Cat` before `trait Animal` in the file; galvanic accepts it without error.

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

**Minimal reproducer:** Not demonstrable via assembly — visibility is not
enforced; a `pub(crate)` field accessed from outside its module compiles
identically to a `pub` field, producing no behavioral difference in output.
The finding is that the enforcement mechanism is absent, which cannot be
confirmed by inspecting assembly.

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

**Minimal reproducer:**
```rust
fn swap(arr: &mut [i32; 2]) {
    let t = arr[0];
    arr[0] = arr[1];
    arr[1] = t;
}
```
Assembly signature: look for `str w1, [x0]` and `str w2, [x0, #4]` — confirms
array index (`arr[0]`, `arr[1]`) is a valid place expression on the LHS of
assignment, emitting `str` instructions to the computed element address.

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

**Minimal reproducer (discriminant defaults):**
```rust
enum Dir { North, South, East, West }
fn main() -> i32 { Dir::South as i32 }
```
Assembly signature: look for `mov x0, #1` — confirms South = 1 (implicit
default: North = 0, South = 0 + 1 = 1). Drop order is not demonstrable since
galvanic emits no destructor calls.

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

**Minimal reproducer:** Not demonstrable via assembly — enforcement is deferred;
an `unsafe fn foo() -> i32 { 0 }` called from a safe context (without
`unsafe { foo() }`) compiles without error and emits identical assembly to a
safe function call. The absence of enforcement is the finding, which cannot be
confirmed by assembly content alone.

---

*Last updated: 2026-04-17. Source annotation count at time of writing: ~155 `AMBIGUOUS` markers across 6 source files. 46 entries, sorted by FLS section number, with linked table of contents. Minimal reproducers added 2026-04-17.*
