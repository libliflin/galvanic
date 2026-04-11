// FLS §6.14 — Closure expressions from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/closure-expressions.html
//
// Covers:
//   §6.14 — Closure expression syntax: |params| body
//   §6.14 — Zero-parameter closures: || expr
//   §6.14 — Closures with typed parameters: |x: i32| expr
//   §6.14 — Closures with explicit return type: |x| -> i32 { stmts }
//   §6.14 — Closures with block bodies
//   §6.14 — Move closures: move |params| body
//   §6.22 — Capturing: closures that capture variables from the enclosing scope
//
// §6.14 NOTE: Async closures (async |params| expr) are not yet implemented.
// §6.14 NOTE: Closures that capture by mutable reference require mut bindings —
// documented in §6.22.
//
// Parameters are used as inputs to document that closures execute at runtime,
// not constant-folded from surrounding context.

// FLS §6.14: A closure expression with no parameters and no captures.
// The simplest closure form: `|| expr`.
fn apply_no_param(f: impl Fn() -> i32) -> i32 {
    f()
}

fn closure_zero_params(x: i32) -> i32 {
    // FLS §6.14: `|| x + 1` is a closure with no parameters; the body is
    // an expression without a block.
    let val = x + 1;
    // FLS §6.22: `val` is captured from the enclosing scope.
    let f = || val;
    apply_no_param(f)
}

// FLS §6.14: A closure expression with a single parameter.
fn apply_one_param(f: impl Fn(i32) -> i32, x: i32) -> i32 {
    f(x)
}

fn closure_one_param(x: i32) -> i32 {
    // FLS §6.14: `|n| n + 1` is a closure with one inferred-type parameter.
    let f = |n| n + 1;
    apply_one_param(f, x)
}

// FLS §6.14: A closure expression with two parameters.
fn apply_two_params(f: impl Fn(i32, i32) -> i32, a: i32, b: i32) -> i32 {
    f(a, b)
}

fn closure_two_params(a: i32, b: i32) -> i32 {
    // FLS §6.14: `|x, y| x + y` is a closure with two inferred-type parameters.
    let f = |x, y| x + y;
    apply_two_params(f, a, b)
}

// FLS §6.14: A closure expression with explicitly typed parameters.
// ClosureParam ::= Pattern (":" Type)
fn closure_typed_params(x: i32) -> i32 {
    // FLS §6.14: `|n: i32| n * 2` — the parameter has an explicit type annotation.
    let f = |n: i32| n * 2;
    apply_one_param(f, x)
}

// FLS §6.14: A closure with a block body (multiple statements).
// The body is ExpressionWithBlock rather than ExpressionWithoutBlock.
fn closure_block_body(x: i32) -> i32 {
    // FLS §6.14: The body `{ let y = n + 1; y * 2 }` is a block expression.
    let f = |n| {
        let y = n + 1;
        y * 2
    };
    apply_one_param(f, x)
}

// FLS §6.14: A closure with an explicit return type annotation.
// ClosureExpression ::= "move"? "|" ClosureParam* "|" ("->" Type)?
fn closure_explicit_return_type(x: i32) -> i32 {
    // FLS §6.14: `-> i32` is the optional return type annotation.
    let f = |n: i32| -> i32 { n + 10 };
    apply_one_param(f, x)
}

// FLS §6.22: A capturing closure — the closure body references a variable
// from the enclosing scope. The variable is captured by copy (for Copy types).
fn closure_captures_local(x: i32, y: i32) -> i32 {
    // FLS §6.22: `base` is defined in the enclosing scope and captured.
    let base = x * 2;
    // FLS §6.14: The closure parameter `n` is distinct from the captured `base`.
    // FLS §6.22: `base` is captured into the closure's environment.
    let f = |n| base + n;
    apply_one_param(f, y)
}

// FLS §6.22: A capturing closure that captures a parameter from the function.
fn closure_captures_parameter(a: i32, b: i32) -> i32 {
    // FLS §6.22: `a` (a function parameter) is captured by the closure.
    let f = |n| a + n;
    apply_one_param(f, b)
}

// FLS §6.14: A move closure transfers ownership of captured variables.
// The `move` keyword forces all captures to be by move (or copy for Copy types).
fn apply_move_closure(f: impl Fn(i32) -> i32, x: i32) -> i32 {
    f(x)
}

fn closure_move(x: i32, y: i32) -> i32 {
    let captured = x;
    // FLS §6.14: `move |n| captured + n` — the `move` keyword forces the
    // capture of `captured` by value (copy, since i32: Copy).
    // FLS §6.22: Moved captures: the variable is copied into the closure env.
    let f = move |n| captured + n;
    apply_move_closure(f, y)
}

// FLS §6.14: A closure used inline as a call argument.
fn closure_inline_argument(x: i32) -> i32 {
    // FLS §6.14: The closure `|n| n + 5` is passed directly as an argument.
    apply_one_param(|n| n + 5, x)
}

// FLS §6.14: A closure with a conditional body.
fn closure_conditional_body(x: i32) -> i32 {
    // FLS §6.14: The body uses an if-else expression — still a single expression body.
    let f = |n| if n > 0 { n } else { -n };
    apply_one_param(f, x)
}

// FLS §6.14: Two closures defined in the same scope, each capturing different variables.
fn two_closures(a: i32, b: i32) -> i32 {
    // FLS §6.22: Each closure captures a different variable from the scope.
    let f = |n| n + a;
    let g = |n| n + b;
    apply_one_param(f, apply_one_param(g, 0))
}

fn main() {
    let _ = closure_zero_params(10);
    let _ = closure_one_param(5);
    let _ = closure_two_params(3, 4);
    let _ = closure_typed_params(7);
    let _ = closure_block_body(3);
    let _ = closure_explicit_return_type(2);
    let _ = closure_captures_local(5, 3);
    let _ = closure_captures_parameter(10, 20);
    let _ = closure_move(8, 2);
    let _ = closure_inline_argument(6);
    let _ = closure_conditional_body(-4);
    let _ = two_closures(1, 2);
}
