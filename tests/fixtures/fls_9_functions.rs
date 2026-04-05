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

// FLS §6.12.1 — Call expressions.
// Source: https://rust-lang.github.io/fls/expressions.html#call-expressions
//
// A function calling another function with no arguments.
// The FLS does not provide a minimal example here; this is a derived case
// from §6.12.1 and §9 illustrating the simplest non-trivial call.
fn constant_answer() -> i32 {
    42
}

fn call_no_args() -> i32 {
    constant_answer()
}

// FLS §6.12.1: Call with arguments — argument expressions evaluated left-to-right
// (FLS §6.12.1 AMBIGUOUS: evaluation order is not explicitly specified).
fn add_two(a: i32, b: i32) -> i32 {
    a + b
}

fn call_with_args() -> i32 {
    add_two(20, 22)
}
