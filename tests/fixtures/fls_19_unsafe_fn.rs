// FLS §19 — Unsafe functions derived from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/unsafety.html
//
// FLS §19: "An unsafe function is a function that is not safe to call."
// An `unsafe fn` can only be called from an unsafe context (an `unsafe { }`
// block or another `unsafe fn`). Its body may contain unsafe operations
// without requiring an inner `unsafe { }` block.
//
// FLS §19 AMBIGUOUS: The spec requires callers to use an unsafe context, but
// does not define the enforcement mechanism. Galvanic records the `unsafe`
// qualifier and defers static enforcement.
//
// FLS §9.1: Function qualifiers — `const`, `async`, `unsafe`, `extern`.
// An `unsafe fn` is declared with the `unsafe` keyword before `fn`.
//
// FLS §19 NOTE: The spec does not provide self-contained code examples for
// `unsafe fn` in isolation; examples below are derived from the semantic
// description in §19 and the function qualifier grammar in §9.1.

// FLS §19: Simple `unsafe fn` — takes a value and doubles it.
// The caller is responsible for ensuring no invariants are violated.
unsafe fn double(x: i32) -> i32 {
    x * 2
}

// FLS §19: `unsafe fn` with two parameters — adds them.
unsafe fn add(a: i32, b: i32) -> i32 {
    a + b
}

// FLS §19: `unsafe fn` with conditional body — classifies a value.
unsafe fn classify(x: i32) -> i32 {
    if x > 0 { 1 } else if x < 0 { 2 } else { 0 }
}

// FLS §19: `unsafe fn` called from another `unsafe fn`.
// The callee (`double`) is unsafe; the caller (`quad`) is also unsafe and
// does not need an inner `unsafe { }` block to call `double`.
unsafe fn quad(x: i32) -> i32 {
    double(double(x))
}

// FLS §19, §9.1: Regular fn wrapping unsafe fn calls.
// The `unsafe { }` block creates the required unsafe context for calling
// the `unsafe fn` from non-unsafe code.
fn safe_double(x: i32) -> i32 {
    unsafe { double(x) }
}

fn safe_add(a: i32, b: i32) -> i32 {
    unsafe { add(a, b) }
}

fn main() -> i32 {
    // FLS §19: `double(3)` called from unsafe block → 6.
    let a = unsafe { double(3) };
    // FLS §19: `add(2, 4)` called from unsafe block → 6.
    let b = unsafe { add(2, 4) };
    // FLS §19: `classify(5)` → positive → 1.
    let c = unsafe { classify(5) };
    // FLS §19: `quad(2)` → double(double(2)) = double(4) = 8.
    let d = unsafe { quad(2) };
    // Regular fns wrapping unsafe calls.
    let e = safe_double(5); // 10
    let f = safe_add(3, 4); // 7
    // a=6, b=6, c=1, d=8, e=10, f=7 → sum=38
    a + b + c + d + e + f
}
