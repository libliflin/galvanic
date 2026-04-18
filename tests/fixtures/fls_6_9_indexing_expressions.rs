// FLS §6.9 — Indexing expressions from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/expressions/index-expressions.html
//
// Covers:
//   §6.9 — Indexing expressions: `array[index]`
//
// Galvanic's resolution for bounds checking: out-of-bounds access traps via
// the ARM64 hardware fault — no software guard is inserted. See §6.9/§6.23 in
// refs/fls-ambiguities.md.
//
// All examples use function parameters to verify runtime codegen, not constant
// folding (FLS Constraint 1).

// FLS §6.9: Indexing with a constant index.
// The index value is statically known but must not cause the load to be
// constant-folded — the array value itself comes from a runtime parameter.
fn index_constant(arr: [i32; 3]) -> i32 {
    // FLS §6.9: "An index expression denotes an element of an array or slice."
    arr[1]
}

// FLS §6.9: Indexing with a variable index.
// Both the array and the index are runtime values — the element address is
// computed at runtime as `base + index * element_size`.
fn index_variable(arr: [i32; 5], i: usize) -> i32 {
    arr[i]
}

// FLS §6.9: Indexing in an arithmetic expression.
// The indexed element is used as an operand — no constant folding.
fn index_in_arithmetic(a: i32, b: i32, c: i32) -> i32 {
    let arr = [a, b, c];
    arr[0] + arr[2]
}

// FLS §6.9: Multiple indices in one expression.
// Each index expression independently computes an element address at runtime.
fn sum_first_and_last(arr: [i32; 4]) -> i32 {
    arr[0] + arr[3]
}

// FLS §6.9: Index derived from a parameter index expression.
// The index `i` is a function parameter — the load offset is computed at
// runtime from the parameter value.
fn lookup(arr: [i32; 4], i: usize) -> i32 {
    arr[i]
}

// FLS §6.9: Indexing inside a loop.
// The index advances each iteration — runtime address computation each time.
fn sum_array(arr: [i32; 5]) -> i32 {
    let mut s = 0;
    let mut i = 0;
    while i < 5 {
        s = s + arr[i];
        i = i + 1;
    }
    s
}

fn main() {
    let _ = index_constant([10, 20, 30]);
    let _ = index_variable([1, 2, 3, 4, 5], 2);
    let _ = index_in_arithmetic(1, 2, 3);
    let _ = sum_first_and_last([10, 20, 30, 40]);
    let _ = lookup([5, 10, 15, 20], 3);
    let _ = sum_array([1, 2, 3, 4, 5]);
}
