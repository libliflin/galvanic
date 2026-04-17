# Changelog — Customer Champion Cycle 009

## Stakeholder: The Lead Researcher

**Became:** The Lead Researcher — the author using galvanic as a research instrument,
extending the compiler feature by feature, tracking FLS compliance and cache-line
correctness as the two research questions inch toward answers.

**Rotation rationale:** Cycle 006 served the Lead Researcher. Cycle 007 served the
Spec Researcher. Cycle 008 served the Compiler Contributor. The Lead Researcher was
last served two cycles ago — their turn.

---

## Floor check

Build: OK. Tests: 2056 pass, 0 fail. Clippy: OK. Unsafe audit: OK. Floor intact.

---

## What I experienced

Walked steps 3–9 of the Lead Researcher journey. Two parse-only fixtures remain in
the snapshot: `fls_4_14_where_clauses_on_types.rs` and `fls_12_1_generic_trait_impl.rs`.
Chose `fls_4_14_where_clauses_on_types.rs` — it has a `fn main`, so it should produce
assembly when it compiles.

```
cargo run -- tests/fixtures/fls_4_14_where_clauses_on_types.rs
```

Output:
```
galvanic: compiling fls_4_14_where_clauses_on_types.rs
parsed 11 item(s)
error: lower failed in 'main': not yet supported: StructLit expression in non-const context (runtime codegen not yet implemented)
lowered 2 of 3 functions (1 failed)
galvanic: lowered 2 function(s) — no fn main, no assembly emitted
```

The cycle-008 fix (naming `StructLit` in the catch-all error) is working. I can now
grep for `ExprKind::StructLit` in `lower.rs` and understand the pipeline gap. Good.

**The worst moment:** The fixture's `fn main` contains `let m = Maybe::Some(Foo { x: 7 })`.
Galvanic has extensive `StructLit` handling in `lower_stmt` (the `let VAR = Struct { ... }`
path at line ~9100) and in `lower_struct_expr_into`. But when a struct literal appears as
an argument to an **enum variant constructor** — `Maybe::Some(Foo { x: 7 })` — it hits
`lower_expr`, which has no `StructLit` arm.

Traced the failing path manually:

1. `let m = Maybe::Some(Foo { x: 7 })` matches the pattern at `lower_stmt` line 9426–9429:
   `let ExprKind::Call { callee, args } = ...` where callee is `Path[Maybe, Some]`.
2. Galvanic looks up `Maybe::Some` in `enum_defs` (1 field), allocates `1 + 1 = 2` slots.
3. For each arg, calls `self.lower_expr(arg, &IrTy::I32)` at line 9487.
4. `arg` is `Foo { x: 7 }` — a `StructLit`. `lower_expr` has no `StructLit` arm.
5. Falls to the catch-all: "StructLit expression in non-const context."

Confirmed with minimal reproducer:
```rust
struct Foo { x: i32 }
enum Maybe<T> { Some(T), None }
fn main() -> i32 {
    let m = Maybe::Some(Foo { x: 7 });
    0
}
```
→ "StructLit expression in non-const context"

Confirmed that a non-struct enum variant works fine:
```rust
enum Maybe { Some(i32), None }
fn main() -> i32 { let m = Maybe::Some(7); 0 }
```
→ compiles OK.

The existing `store_nested_struct_lit` helper (line 6397) does exactly what's needed
for this case: given a `StructLit` expression and a target base slot, it stores each
field into consecutive slots. This is already used by the `let VAR = OuterStruct { nested_field: InnerStruct { ... } }` path. The same logic applies here.

For the specific fixture, `Maybe<T>` has 1 variant field (`T = Foo { x: i32 }`, 1 field).
The enum allocates `1 + 1 = 2` slots. The struct needs 1 field slot. Slot counts match.

Also noticed: the `fls_12_1_generic_trait_impl.rs` fixture emits "lowered 2 function(s) —
no fn main, no assembly emitted." With the cycle-005 fix in place, this is now at least
informative. But there's still no path to assembly from this fixture without adding a `main`.

---

## Goal

**Support struct literal arguments in enum variant constructor calls in `lower_stmt`.**

When `let m = Maybe::Some(Foo { x: 7 })` is lowered, the enum variant constructor
path in `lower_stmt` (around line 9487) calls `lower_expr(arg, &IrTy::I32)` for each
argument. When the argument is a `StructLit`, this hits the catch-all error. The fix:
detect when an argument expression is a `StructLit`, and instead call
`store_nested_struct_lit(arg, field_slot, struct_name)` with the slot allocated for
that enum field.

### What to change

**In `lower_stmt`'s enum variant constructor branch (around line 9487):**

Before calling `lower_expr(arg, &IrTy::I32)`, check whether `arg` is
`ExprKind::StructLit { name: sname, .. }`. If it is:
- Extract the struct name from `sname.text(self.source)`.
- Look up the struct's size from `struct_sizes` (or `struct_defs.get(name).len()`) to
  determine how many consecutive slots the struct occupies.
- Ensure the enum's slot allocation accounts for the struct's size: allocate enough
  extra slots so the struct's fields can be stored without overlap.
- Call `store_nested_struct_lit(arg, slot, struct_name)` instead of `lower_expr`.

For the simple case (struct with 1 field, enum variant with 1 arg), the existing
slot allocation (`1 + field_count` = 2 total) is already correct. For more general
cases the builder should adjust slot allocation to use `struct_sizes` when the arg
is a struct.

Add an `AMBIGUOUS: §4.2 — ...` annotation near the change: the FLS does not specify
how a variant field that is itself a struct type should be laid out relative to the
discriminant slot. Galvanic stores the discriminant at `base_slot`, then the struct's
fields in consecutive slots `base_slot+1`, `base_slot+2`, etc. This is galvanic's
design choice; the spec is silent.

Add a matching entry to `refs/fls-ambiguities.md` documenting this layout choice with
the minimal reproducer: `let m = Maybe::Some(Foo { x: 7 })`.

### Why this is the most valuable change right now

The Lead Researcher's signal is momentum. The last two cycles served the Spec Researcher
and the Compiler Contributor; the Lead Researcher hasn't seen a new feature compile
since cycle 006.

The `fls_4_14_where_clauses_on_types.rs` fixture is the most feature-rich of the two
remaining parse-only fixtures: it exercises where clauses, generic struct and enum
definitions, trait dispatch, and method calls on generic types. Getting `fn main` to
compile requires unblocking each error in order. The first error is `StructLit in enum
variant constructor`. Fixing it produces assembly (even if partial), moves a parse-only
fixture closer to end-to-end, and reveals the next gap for the next cycle.

The fix also captures a genuine FLS finding: the spec doesn't specify variant field
layout when the field is a struct type. That's a new ambiguity entry in
`refs/fls-ambiguities.md` — a research artifact, not just a code fix.

### The specific moment

Step 6 of the Lead Researcher journey: running
`cargo run -- tests/fixtures/fls_4_14_where_clauses_on_types.rs`.
Error: "not yet supported: StructLit expression in non-const context."
Traced to `lower_stmt` line 9487: `lower_expr(arg, &IrTy::I32)` where arg is
`Foo { x: 7 }` inside `Maybe::Some(Foo { x: 7 })`.
The `store_nested_struct_lit` helper already exists for this exact pattern —
it's just not being called from the enum variant constructor path.
