// FLS §19 — Unsafe traits derived from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/unsafety.html
//
// FLS §19: "An unsafe trait is a trait that is not safe to implement."
// An `unsafe trait` can only be implemented using `unsafe impl`. The `unsafe`
// qualifier signals that implementing the trait correctly requires satisfying
// some invariant that the compiler cannot verify.
//
// FLS §19 AMBIGUOUS: The spec requires `unsafe impl` when implementing an
// `unsafe trait`, but does not specify how the compiler verifies this pairing.
// Galvanic records the `unsafe` qualifier on both `TraitDef` and `ImplDef`
// and defers enforcement — no type-system check is performed.
//
// FLS §19 NOTE: The spec does not provide self-contained code examples for
// `unsafe trait` in isolation; examples below are derived from the semantic
// description in §19.

// FLS §19: Simple `unsafe trait` — declares a contract that implementors
// must uphold manually. The `value` method returns an i32.
unsafe trait UnsafeValue {
    fn value(&self) -> i32;
}

// FLS §19: `unsafe impl` — implements the unsafe trait for a concrete type.
// The implementor asserts they are upholding the trait's invariant.
struct Safe(i32);

unsafe impl UnsafeValue for Safe {
    fn value(&self) -> i32 {
        self.0
    }
}

// FLS §19: `unsafe trait` with multiple methods.
unsafe trait UnsafePair {
    fn first(&self) -> i32;
    fn second(&self) -> i32;
}

struct Pair(i32, i32);

unsafe impl UnsafePair for Pair {
    fn first(&self) -> i32 {
        self.0
    }
    fn second(&self) -> i32 {
        self.1
    }
}

// FLS §19: `unsafe impl` for a second concrete type of the same unsafe trait.
struct DoubleValue(i32);

unsafe impl UnsafeValue for DoubleValue {
    fn value(&self) -> i32 {
        self.0 * 2
    }
}

fn main() -> i32 {
    let s = Safe(7);
    let p = Pair(3, 4);
    let d = DoubleValue(5);
    s.value() + p.first() + p.second() + d.value()
    // 7 + 3 + 4 + 10 = 24
}
