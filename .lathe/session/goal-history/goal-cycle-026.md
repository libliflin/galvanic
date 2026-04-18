# Goal — Cycle 026

**Stakeholder:** Lead Researcher

**What to change:** Fix `lower_stmt`'s handling of `StmtKind::Expr` so that expression
statements (FLS §8.2) containing integer literals — and more broadly, any value expression
whose result is discarded — lower successfully. `fn returns_unit() { 42; }` is valid Rust
and must not fail. The current code at `src/lower.rs:10483` passes `IrTy::Unit` to
`lower_expr` for all expression statements; the integer literal handler treats any non-numeric
type hint as an error. An expression statement's discarded result has nothing to do with
`IrTy::Unit` — the expression should be lowered with its own natural type.

**Why this stakeholder:** Cycles 025 = Spec Researcher, 024 = Compiler Contributor,
023 = Cache-Line Researcher, 022 = Lead Researcher. Lead Researcher is the most under-served
— last served four cycles ago.

**Why now:** At step 3 of the Lead Researcher's journey — running `fls_9_functions.rs` and
reading the output — galvanic reports:

```
galvanic: compiling fls_9_functions.rs
parsed 19 item(s)
error: lower failed in 'returns_unit': not yet supported: integer literal with non-integer type
lowered 19 of 20 functions (1 failed)
galvanic: emitted tests/fixtures/fls_9_functions.s (partial — some functions failed)
```

The failing function is:

```rust
fn returns_unit() {
    42;
}
```

This is idiomatic Rust. FLS §8.2 says expression statements evaluate their expression
for side effects and discard the value. FLS §9 says a function with no return type annotation
returns `()`. The expression `42` has type `i32`; the semicolon discards it. There is
nothing ambiguous, unsupported, or unspecified about this — it is a core language construct
that galvanic fails to lower.

The root cause is at `src/lower.rs:10482–10484`:

```rust
StmtKind::Expr(expr) => {
    self.lower_expr(expr, &IrTy::Unit)?;
    Ok(())
}
```

The comment above (lines 10475–10481) explicitly acknowledges that this works for
assignment and call expressions because those handlers "ignore `ret_ty`". But integer
literal expressions do not ignore `ret_ty` — the `LitInt` match arm at line 10654
dispatches on the type hint and returns `Err("integer literal with non-integer type")`
for any type that isn't a recognized numeric type, including `IrTy::Unit`.

**The class of fix:** Any expression can legally appear as a statement in Rust (FLS §8.2).
The expression statement handler must not impose `IrTy::Unit` on expressions that produce
typed values — it should lower the expression with its natural type. The discarded result
of a statement context is not "unit typed" — it is simply discarded. The fix eliminates
the entire category of "expression statement fails because it's not a call or assignment."

Additional observation: the error message "integer literal with non-integer type" has no
FLS citation, violating the architecture invariant. The message should name at minimum
FLS §2.4.4.1 (integer literals) and §8.2 (expression statements). The functional fix is
primary; the message improvement is a natural corollary once the error is rare enough to
name precisely. The static check in `lower_source_all_unsupported_strings_cite_fls`
does not catch this violation because the check looks for the string "not yet supported"
in the source, but the message payload is `"integer literal with non-integer type"` —
the prefix is added by the Display impl. The check has a gap: it cannot enforce citation
discipline on message payloads.

**What success looks like:**

```
galvanic: compiling fls_9_functions.rs
parsed 19 item(s)
galvanic: emitted tests/fixtures/fls_9_functions.s
```

All 20 functions lower successfully. `returns_unit` emits a `LoadImm` for `42` (runtime
instruction, not folded), and the value is not used (the register is discarded). No
assembly change is needed for the caller — there is no return value to pass.

**Constraint:** Do not fold `42;` to a no-op at the lowering stage. The spec requires
expression statements to execute at runtime (FLS §6.1.2:37–45, Constraint 1). Even though
`42;` has no side effects, galvanic must emit the runtime instruction — an optimization
pass could eliminate dead code later, but the lowering pass must not. The litmus test:
`fn foo(x: i32) { x + 1; }` must also lower successfully, emitting a runtime `add`
instruction whose result is discarded.

**Lived experience note:** I became the Lead Researcher. I ran `cargo test` — clean
(2110 pass). I pulled up the fixtures directory: 46 `.rs` files covering FLS sections
from §2 through §19. I picked `fls_9_functions.rs` because §9 (Functions) is the
spine of the spec — if functions don't work, nothing works. I ran it. The output said
"19 of 20" with the error "not yet supported: integer literal with non-integer type". I
looked up `returns_unit` in the fixture — it is three lines:

```rust
fn returns_unit() {
    42;
}
```

The hollowest moment: this function has no corner cases, no unsupported types, no novel
constructs. It is a function that evaluates a literal and discards the result. If this
fails, then any function with intermediate expression statements — logging calls, debug
prints, pure-effect expressions — would fail on the same error. The error message gives
nothing: no FLS section, no context, no hint about what "non-integer type" was received.
I opened `lower.rs` and traced the path: `lower_stmt` passes `IrTy::Unit` to `lower_expr`
for all expression statements; the `LitInt` arm doesn't handle `IrTy::Unit`. The comment
says this is "safe" because assignment and call handlers ignore `ret_ty` — but it isn't
safe for anything else. The Lead Researcher's emotional signal is momentum — each run
tells them something true. This run told them that `fn returns_unit() { 42; }` is broken,
with no spec anchor to understand why. That is not momentum. That is a wall.
