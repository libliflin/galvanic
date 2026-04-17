# Changelog — Customer Champion Cycle 008

## Stakeholder: The Compiler Contributor

**Became:** A Compiler Contributor — a CS student who picked galvanic because it's
spec-driven, small, and has clear contribution paths via parse-only fixtures. Walked
steps 3–6 of the Compiler Contributor journey.

**Rotation rationale:** Cycle 006 served the Lead Researcher. Cycle 007 served the
Spec Researcher. The Compiler Contributor was last served in cycle 005 — two cycles
ago, their turn.

---

## What I experienced

Floor check: build OK, 2055 tests pass, clippy clean. Floor intact.

Step 4: picked `fls_4_14_where_clauses_on_types.rs` — the first parse-only fixture
named in the snapshot. It has a `fn main`, so it should produce assembly if galvanic
can lower it.

```
cargo run -- tests/fixtures/fls_4_14_where_clauses_on_types.rs
```

Output:
```
galvanic: compiling fls_4_14_where_clauses_on_types.rs
parsed 11 item(s)
error: lower failed in 'main': not yet supported: expression kind in non-const context (runtime codegen not yet implemented)
lowered 2 of 3 functions (1 failed)
galvanic: lowered 2 function(s) — no fn main, no assembly emitted
```

**The worst moment:** The error gives me nothing to work with. "expression kind in
non-const context (runtime codegen not yet implemented)" — which expression kind?
The fixture's `main` uses struct literals, enum variant construction, method calls,
function calls, and a `match` expression. Any of these could be the culprit. There
is no search term in the error that maps to anything in the source.

I searched the source for the literal error string and found it at `src/lower.rs:18797`
— a catch-all `_ => Err(...)` arm at the bottom of a match on `&expr.kind` in
`lower_expr`. Since `ExprKind` derives `Debug`, the variant name is available — but
it's not being included in the error.

To identify the actual failing construct, I had to write test programs to isolate the
issue: `Maybe::Some(Foo { x: 7 })` — a struct literal passed inline as an enum tuple
variant argument. The same construct works when the struct is pre-assigned to a
variable (`let f = Foo { x: 7 }; Maybe::Some(f)`). The failing construct is
`ExprKind::StructLit` appearing as an argument in an enum variant call path that calls
`lower_expr(arg, &IrTy::I32)` — which doesn't know how to handle a struct literal.

But the diagnostic gap is the issue: a contributor who sees this error and looks at the
parse-only fixture list doesn't know whether the problem is the struct literal, the
enum variant construction, the `match`, or something else. They cannot find the relevant
match arm in `lower_expr` without knowing the variant name.

---

## Goal

**Include the `ExprKind` variant name in the catch-all error at `src/lower.rs:18797`.**

Before:
```
error: lower failed in 'main': not yet supported: expression kind in non-const context (runtime codegen not yet implemented)
```

After (for the `fls_4_14_where_clauses_on_types.rs` case):
```
error: lower failed in 'main': not yet supported: StructLit expression in non-const context (runtime codegen not yet implemented)
```

The variant name is available from `expr.kind` — `ExprKind` derives `Debug`. The builder
should extract just the variant name (not the full debug tree with all fields), as the full
debug output for a `StructLit` with nested fields would be too verbose. A helper that matches
on `expr.kind` and returns the variant name as a static string, or a single method on
`ExprKind` (e.g., `kind_name() -> &'static str`) that returns the variant name, are both
valid approaches.

**Why this is a class-level fix:** Every unimplemented expression kind that hits the
catch-all — `StructLit`, `EnumVariantLit`, `Range`, `Await`, `ForLoop`, `WhileLet`, and
any future variant — becomes immediately diagnosable. The contributor sees the variant name,
can grep for `ExprKind::VariantName` in `lower.rs`, can see which arms exist and which
don't, and knows exactly where to add the new case. Without this, every catch-all hit is a
dead end requiring manual debug instrumentation.

**The specific moment:** Step 4 of the Compiler Contributor journey. Ran
`galvanic tests/fixtures/fls_4_14_where_clauses_on_types.rs`. Got:
`not yet supported: expression kind in non-const context (runtime codegen not yet implemented)`.
Grepped source for the error string → found `lower.rs:18797` → catch-all `_` arm in
`lower_expr`. No variant name. No search term. Dead end.
