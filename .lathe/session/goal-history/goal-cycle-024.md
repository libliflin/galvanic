# Goal — Cycle 024

**Stakeholder:** Compiler Contributor

**What to change:** Add docstrings to the three context-specific lowering functions —
`lower_enum_expr_into`, `lower_tuple_expr_into`, `lower_struct_expr_into` — and add a
section to the `lower.rs` module docstring (or the FLS-citations comment block at the top)
that explains the two-tier lowering architecture: when `lower_expr` is the path and when
one of the `lower_*_into` functions is called instead. A contributor should be able to open
`lower.rs`, find the new section in two minutes, and know which function to modify for any
given expression-in-context problem.

**Why this stakeholder:** Cycles 023 = Cache-Line Researcher, 022 = Lead Researcher, 021 =
Spec Researcher, 020 = Compiler Contributor. Compiler Contributor is the most under-served
— last served four cycles ago.

**Why now:** Step 4 of the Compiler Contributor's journey: "Open `lower.rs` or `codegen.rs`.
Find the match arm where a new case would go. Look for an existing similar case to
pattern-match from."

I grepped for `ExprKind::Match` to find where match expression lowering lives.
I found it in **8 places**:

```
src/lower.rs:7130:            ExprKind::Match { scrutinee, arms } => {
src/lower.rs:7378:            ExprKind::Match { scrutinee, arms } => {
src/lower.rs:7715:            ExprKind::Match { scrutinee, arms } => {
src/lower.rs:12256:            ExprKind::Match { scrutinee, arms } => {
... (4 more in helper functions)
```

The FLS-citations comment block at line 62 says:
```
// FLS §6.18: Match expressions — `lower_expr` handles `ExprKind::Match`.
```

This is misleading. `lower_expr` handles one path; three context-specific functions
(`lower_enum_expr_into`, `lower_tuple_expr_into`, `lower_struct_expr_into`) handle the
other three. None of the context-specific functions have docstrings. There is no explanation
anywhere in `lower.rs` of when `lower_expr` is called versus when the `lower_*_into`
functions are called, or why the split exists.

**The design that's invisible:**

- `lower_expr` — the generic path; takes an expression and a return type hint, returns an
  `IrValue`. This is the path for any expression used in a scalar or general context.
- `lower_enum_expr_into(expr, base_slot, max_fields)` — called when the return context
  requires writing into pre-allocated enum slots (discriminant + field slots). Called by the
  return-type-is-enum path in `lower_stmt(Let)` and function tail logic.
- `lower_tuple_expr_into(expr, base_slot, n_elems)` — called when the return context
  requires writing into pre-allocated tuple slots. Called when a function returns a tuple
  type or a let binding has a tuple type.
- `lower_struct_expr_into(expr, base_slot, n_fields, struct_name)` — called when the return
  context requires writing into pre-allocated struct slots.

These three functions exist because compound types (enums, tuples, structs) at galvanic's
current milestone are always stored into pre-allocated slots rather than returned in
registers. `lower_expr` cannot return a compound value as `IrValue` — it returns scalars
only. Whenever an expression that could produce a compound value is lowered, the caller
must know whether to call `lower_expr` (scalar path) or one of the `lower_*_into`
functions (compound path). This is the key seam in the lowering pass, and it's not
documented.

**The specific moment:** I was tracing `not yet supported: tuple expression must be bound
to a 'let' variable at this milestone` for the `match_tuple` function in
`tests/fixtures/fls_6_18_match_expressions.rs`. The fixture does `match (x, y) { ... }`.
The error is at line 18465, inside the general `ExprKind::Tuple` arm of `lower_expr`. I
found it. But then I needed to understand why it was there — was this the right place to
fix tuple-scrutinee match support? Would I add a new case to `lower_expr`, or was there
another path? I grepped for `ExprKind::Tuple`, found it in multiple locations, and had no
way to know which function was authoritative for this context. The hollowest moment: after
reading every one of the eight `ExprKind::Match` handler sites across lower.rs, I still
could not answer "which function do I modify?" without reading every call site to understand
when each is invoked.

**The class of fix:** The two-tier lowering architecture (`lower_expr` for scalars,
`lower_*_into` for compound types) is a structural design decision that governs where every
new expression lowering case goes. It is currently implied by function signatures and call
sites but never stated. Making it explicit — in the module docstring and in docstrings for
each of the three context-specific functions — eliminates the entire category of "I don't
know which function to modify" opacity. It applies to every future contributor who opens
`lower.rs` for any expression-kind feature addition, not just tuple scrutinees.

**What the documentation needs to say (the what, not the how):**

1. The `lower.rs` module docstring (or the FLS-citations comment block) needs a new section
   that names the two-tier design: `lower_expr` is the scalar/generic path; the three
   `lower_*_into` functions are the compound-type path. It should state when each is
   called, and note that compound types at this milestone are always written into
   pre-allocated slots, not returned in registers.

2. `lower_enum_expr_into` needs a docstring explaining: its purpose (lower an expression
   into pre-allocated enum slots), when it is called (whenever a function's return type is
   an enum, or a let binding has an enum type), and what `base_slot` / `max_fields` mean in
   relation to the enum discriminant + field layout.

3. `lower_tuple_expr_into` needs a docstring explaining: its purpose (lower an expression
   into pre-allocated tuple slots), when it is called (function returns a tuple type, or let
   binding has a tuple type), and what `base_slot` / `n_elems` mean.

4. `lower_struct_expr_into` needs a docstring explaining: its purpose (lower an expression
   into pre-allocated struct slots), when it is called, and what `base_slot` / `n_fields` /
   `struct_name` mean in relation to the struct field layout.

5. Update the FLS-citations comment block line that says
   `// FLS §6.18: Match expressions — 'lower_expr' handles 'ExprKind::Match'.`
   to correctly reflect that `ExprKind::Match` is handled in both `lower_expr` (scalar
   scrutinee path) and the three `lower_*_into` functions (match expression in compound
   return context).

**Constraint:** Do not add or modify any functional code. This is documentation only. The
goal is navigability — a contributor who finds the docstrings should know where to add a
new case without reading any call sites.

**Lived experience note:** I became the Compiler Contributor. I cloned the repo, ran
`cargo test` (2110 pass, green), found `not yet supported: tuple expression must be bound
to a 'let' variable` by running the match expressions fixture, opened `lower.rs` to find
where to add the fix. The module docstring at lines 1-27 is good: it explains the lowering
philosophy, the FLS constraint compliance, and the key FLS sections. Then I hit the
FLS-citations block at line 62: `lower_expr handles ExprKind::Match`. I looked for
`lower_expr`. Found it at line 10555 — after 10,000 lines of other code. Found `ExprKind::Match` inside it at line 12256. Good. But grepping for all occurrences showed 8 total,
3 of them in functions I had never heard of: `lower_enum_expr_into`, `lower_tuple_expr_into`,
`lower_struct_expr_into`. None of these have docstrings. None are mentioned in the module
docs. I had to read their signatures and then read their call sites (another 10,000+ lines)
to understand what "into" meant and when each is called. The Compiler Contributor's emotional
signal is **confidence** — "I know exactly where to make this change." I did not feel that.
I felt the project's architecture hiding behind 18,764 lines of well-written but
navigation-opaque code.
