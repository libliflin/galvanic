# Goal: Diagnose no-entry-point silently — add `fn main` to `fls_6_19_return_expressions.rs` and its e2e test

## What

Two changes, one goal:

1. Add a `fn main()` to `tests/fixtures/fls_6_19_return_expressions.rs` that exercises all eight return-expression functions in the file. The main must pass parameters that prevent constant folding (Constraint 1 — each callee already takes an `i32` argument, so the calls must pass non-const values). Since parameters can't be truly runtime in `main`, use `let` bindings initialized to literals: the existing functions take parameters and do real branching, so the calls to them emit real `bl` instructions.

2. Add an assembly inspection test in `tests/e2e.rs` for the fixture that asserts: (a) `bl early_return_taken` or equivalent calls are present, (b) the file emits without error, and (c) runtime branch instructions appear (not a constant-folded result).

**Do NOT change `src/main.rs`.** The silent exit is a symptom; the root cause is that the fixture was written as a library without an entry point. Fix the fixture.

## Which stakeholder

**The Compiler Contributor** — a Rust programmer following the contributor journey who wants to add lowering+codegen for a parse-only fixture.

Step 4 of the journey says: "Pick a fixture that has only a parse test and try to add lowering + codegen for it." `fls_6_19_return_expressions.rs` has no `.s` file and appears to be a candidate. The contributor runs:

```
cargo run -- tests/fixtures/fls_6_19_return_expressions.rs
```

And gets:
```
galvanic: compiling fls_6_19_return_expressions.rs
parsed 8 item(s)
```

Nothing else. No error. No assembly. Exit code 0.

The contributor has no idea whether the feature works, failed silently, or is simply missing something. They spend 5–15 minutes reading `lower.rs`, searching for missing `ExprKind::Return` arms, checking the IR — before eventually noticing the file has no `fn main`. At that point they realize the feature IS already implemented; the fixture just never had an entry point.

## Why now

The last four cycles all served the Lead Researcher (FLS compliance, overflow guards, Constraint 8). The Compiler Contributor has not been served. The `fls_6_19_return_expressions.rs` fixture is the sharpest example of the "fixture exists, feature works, but contributor can't tell" problem, because:

- It is parse-only (no `.s` file committed)
- The feature it covers (§6.19 return expressions) IS fully implemented — the functions compile individually
- Running it produces no output and no error — the most confusing possible outcome
- The fix is completely self-contained: add a `main`, add a test

Once fixed, a contributor who picks this fixture will immediately see assembly output, understand the feature is working, and know their job is to verify and extend test coverage — not to implement something that's already done.

## Lived-experience note

**Stakeholder:** The Compiler Contributor.

**What I tried:** Step 4 of the journey — scan `tests/fixtures/` for parse-only fixtures, pick one, try to compile it end-to-end.

**What I felt:** I picked `fls_6_19_return_expressions.rs`. The file is well-commented, has eight functions each with a clear FLS §6.19 citation, and looks like a meaningful test of return expression behavior. I ran `cargo run -- tests/fixtures/fls_6_19_return_expressions.rs`. The compiler said "parsed 8 item(s)" and stopped. No error, no assembly, no explanation.

My first instinct was: something in the lowering is silently failing for this construct. I started reading `lower.rs` searching for `ExprKind::Return`. I found it — at line 5230 and 16902 — and it's clearly implemented. I wrote a minimal program with a `return` expression and it compiled fine. I wrote another with return-from-loop, return-from-nested-block — all fine.

The **worst moment** was realizing I had been debugging for 10 minutes and the feature was working the whole time. The fixture simply had no `fn main`. The compiler exited cleanly without telling me why. That is the hollow moment: an exit code of 0, silence, and wasted effort.

The **specific turn**: when I checked `grep -n "^fn main" fls_6_19_return_expressions.rs` and got no output. The fixture is a library of examples, not a runnable program. Return expressions are implemented. The fixture just needed an entry point.
