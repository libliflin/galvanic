// FLS §9 — Function examples from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/functions.html
//
// These are adapted to the subset galvanic currently handles (no generics,
// no traits, no struct types). Each function should lex, parse, and
// eventually compile.

fn main() {}

// FLS §9 example: a function with parameters and return type.
// (Adapted: original uses &Point and f64 method calls; simplified to
// arithmetic on primitives that galvanic can handle.)
fn add(a: i32, b: i32) -> i32 {
    a + b
}

// FLS §6.19 — Return expressions.
// Source: https://rust-lang.github.io/fls/expressions.html#return-expressions
fn max(left: i32, right: i32) -> i32 {
    if left > right {
        return left;
    }
    return right;
}

// Tail expression vs. semicolon distinction (FLS §6.4 block expressions).
fn returns_value() -> i32 {
    42
}

fn returns_unit() {
    42;
}

// Multiple parameters, nested arithmetic (FLS §6.5.6).
fn quadratic(a: i32, b: i32, c: i32, x: i32) -> i32 {
    a * x * x + b * x + c
}

// FLS §9:3: A function may call itself (recursive functions are permitted).
//
// The spec does not provide a fibonacci example, but recursive functions are
// explicitly permitted. This is the canonical example from the spec's
// discussion of function calls and recursion.
//
// FLS §9: Functions — recursive calls are ordinary call expressions.
// FLS §6.12.1: Call expressions.
// FLS §6.5.5: Arithmetic operator expressions (+, -).
// FLS §6.5.3: Comparison operator expressions (<=).
// FLS §6.17: If expressions.
//
// NOTE: no FLS example exists for fibonacci specifically. This function is
// derived from the spec's permission for recursion (§9:3) and the semantics
// of the operators it uses.
fn fib(n: i32) -> i32 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
