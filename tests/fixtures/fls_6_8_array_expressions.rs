// FLS §6.8 — Array expressions from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/expressions/array-expressions.html
//
// Covers:
//   §6.8 — Array expressions: `[e1, e2, ..., eN]` (array literal)
//   §6.8 — Array repeat expressions: `[expr; N]`
//
// All examples use function parameters to verify runtime codegen, not constant
// folding (FLS Constraint 1: only const contexts permit compile-time evaluation).

// FLS §6.8: Array literal expression.
// "[e1, e2, ..., eN] is an array expression that constructs an array of N
// elements." — FLS §6.8
fn array_literal_sum(a: i32, b: i32, c: i32) -> i32 {
    let arr = [a, b, c];
    // Access all three elements — each must be a runtime load, not a folded constant.
    arr[0] + arr[1] + arr[2]
}

// FLS §6.8: Array repeat expression.
// "[expr; N] is an array repeat expression that constructs an array of N copies
// of the value of expr." — FLS §6.8
fn array_repeat_first(x: i32) -> i32 {
    let arr = [x; 4];
    // All elements are copies of x; the first is a runtime load.
    arr[0]
}

// FLS §6.8: Array repeat with constant count.
// The count `N` in `[expr; N]` is a constant, but the value `expr` is a
// runtime parameter — the array must be constructed at runtime.
fn array_repeat_sum(x: i32) -> i32 {
    let arr = [x; 3];
    arr[0] + arr[1] + arr[2]
}

// FLS §6.8: Multi-element array with mixed expressions.
// Each element is independently computed from parameters.
fn array_computed_elements(a: i32, b: i32) -> i32 {
    let arr = [a + b, a - b, a * b];
    arr[0]
}

// FLS §6.8: Array passed as a function parameter.
// FLS §9: Array parameters are supported alongside scalar parameters.
fn array_param_first(arr: [i32; 3]) -> i32 {
    arr[0]
}

fn main() {
    let _ = array_literal_sum(10, 20, 30);
    let _ = array_repeat_first(7);
    let _ = array_repeat_sum(5);
    let _ = array_computed_elements(4, 2);
    let _ = array_param_first([1, 2, 3]);
}
