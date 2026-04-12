# Goal: §6.21 Comparison Non-Associativity — Compile-Time Chained-Comparison Rejection

## What

Add a compile-time chained-comparison guard to galvanic's lowering pass
(`src/lower.rs`) so that a comparison operator (`<`, `>`, `<=`, `>=`, `==`,
`!=`) whose left or right operand is itself a comparison expression produces a
`LowerError` with a clear message rather than silently compiling to wrong code.

The builder should:

1. In the lowering path for binary comparison operators in `src/lower.rs`,
   after parsing both operands, check if either operand AST node's kind is
   itself a binary comparison (`BinOp` with `Lt`, `Gt`, `Le`, `Ge`, `Eq`,
   `Ne`). If so, return a `LowerError` with the message:
   `"chained comparison is not allowed: use && to combine comparisons (e.g. a < b && b < c)"`.

   - The check applies to all six comparison operators: `<`, `>`, `<=`, `>=`,
     `==`, `!=`.
   - The check does NOT apply to arithmetic or logical operators (`+`, `&&`,
     `||`, etc.) — only to comparison operators.
   - The check is structural (AST-level): if the left or right child of a
     comparison is a comparison-typed `BinOp` node, reject. This catches the
     common case `a < b < c` (parsed as `(a < b) < c`).
   - False negatives (e.g., a comparison result stored in a variable and then
     compared) are acceptable. False positives (rejecting a valid program) are
     not acceptable. Only reject when the AST *directly* has a comparison as a
     sub-expression of another comparison.

2. Add test cases to `tests/e2e.rs`:
   - Reject `fn f(a: i32, b: i32, c: i32) -> i32 { if a < b < c { 1 } else { 0 } }` —
     chained `<` should fail at lowering.
   - Reject `fn f(a: i32, b: i32, c: i32) -> i32 { if a == b == c { 1 } else { 0 } }` —
     chained `==` should fail.
   - Accept `fn f(a: i32, b: i32, c: i32) -> i32 { if a < b && b < c { 1 } else { 0 } }` —
     correct form using `&&` should compile fine.
   - Accept `fn f(a: i32, b: i32) -> i32 { if a < b { 1 } else { 0 } }` —
     simple (non-chained) comparison should compile fine.

3. Update the §6.21 / §6.7 entry in `refs/fls-ambiguities.md`:
   - Change "Enforcement of non-associativity is deferred" to document that
     direct chained comparisons are now caught at compile time.
   - Document what remains deferred: a comparison result stored in a variable
     and then used as a comparand is not caught; full type-checking would be
     needed to close that gap.
   - Note the alignment with the §6.18 and §6.23 methodology: statically-
     obvious violations caught at compile time, complex cases deferred.

## Which stakeholder

**William (the researcher)** and **FLS spec readers (the Ferrocene team)**.

The §6.21/§6.7 entry in `refs/fls-ambiguities.md` currently reads:
> "Galvanic's choice: Comparison operators are left-associative at the grammar
> level. A chained comparison `a < b < c` parses successfully but produces
> incorrect results at runtime. Enforcement of non-associativity is deferred."

This is a documented silent-correctness gap. Galvanic compiles `a < b < c` as
`(a < b) < c`, which compares the integer result of `(a < b)` — 0 or 1 — with
`c`. The FLS states explicitly that comparisons are non-associative and the
Rust type system would reject this (can't compare `bool` with `i32`). Galvanic
silently produces the wrong answer.

For spec readers: this is the clearest possible example of galvanic diverging
from Rust's type rules — and the fix follows the established methodology.

For William: this is the **third** entry in the "conservative compile-time
check" methodology (after §6.18 exhaustiveness and §6.23 literal zero). Three
entries make it a system, not a pattern.

## Why now

The §6.23 literal-zero check (Claim 4m) just landed (commits #266–267). The
build is clean at 1999 tests. The methodology is established:

- §6.18 (Claim 4l): match exhaustiveness — conservative compile-time rejection
- §6.23 (Claim 4m): literal zero divisor — conservative compile-time rejection
- §6.21 (Claim 4n): chained comparison — conservative compile-time rejection

The implementation cost is minimal:
- One check in the comparison-operator lowering path (three or four lines)
- Four tests
- One paragraph update in `refs/fls-ambiguities.md`

No new infrastructure is needed. The AST already represents both sides of
every binary op as `Expr` nodes, so checking if the child is itself a
comparison is a single `matches!` on the child's `ExprKind`.

Doing this now:
1. Closes the third "silent wrong behavior" entry without requiring a type
   system — only AST structure matters.
2. Reinforces the methodology in `refs/fls-ambiguities.md` as a system with
   three examples.
3. The §6.21 gap is explicitly about non-associativity — the spec is
   unambiguous, galvanic's current behavior is unambiguously wrong.

---

## Acceptance criteria

- `cargo build` passes.
- `cargo test` passes (all 1999 existing tests continue to pass).
- At least one new test demonstrates that `a < b < c` (chained `<`) is
  rejected at lowering time with an error result.
- At least one new test demonstrates that `a == b == c` (chained `==`) is
  rejected.
- At least one new test demonstrates that `a < b && b < c` (correct form) is
  accepted and compiles.
- The §6.21/§6.7 entry in `refs/fls-ambiguities.md` is updated to reflect
  the new state: direct chained comparisons caught at compile time, indirect
  (variable-mediated) chains still deferred.
- No new FLS citations are wrong or vague.

## FLS notes

- **§6.21:1**: "Comparison operators are non-associative." The spec is explicit.
  A chained comparison is a *type error* in Rust — comparing a `bool` with an
  `i32` — not merely a logic error. Galvanic has no type system, so the AST
  structural check is the appropriate proxy.
- **§6.7**: Parenthesized expressions should override the non-associativity
  restriction: `(a < b) == true` is valid if `a < b` is a valid sub-expression.
  The check must only fire when the *direct* child (not a parenthesized
  sub-expression) is a comparison. Check the `ExprKind` of the raw child —
  if the parser wraps parenthesized expressions in a `Paren` variant, skip the
  check. If not, only check one level of nesting.
- The check for `bool` operands (`true < false`) is separate and not required
  by this goal — reject only when both sides are comparison-typed operators.
