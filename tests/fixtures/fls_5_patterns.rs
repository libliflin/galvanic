// FLS §5 — Pattern examples from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/patterns.html
//
// Each pattern form is exercised inside a function body.
// Examples are derived from FLS §5; where the spec provides no code example,
// that is noted explicitly.

// FLS §5.1: Wildcard pattern `_` — matches any value without binding.
// FLS §5.1.4: Identifier pattern — binds matched value to a name.
// FLS §5.1.9: Range patterns — `lo..=hi` (inclusive) and `lo..hi` (exclusive).
// FLS §5.1.11: Or patterns — `p0 | p1 | ...`.
// FLS §5.2: Literal patterns — integer literals (including negative).

// FLS §5.1.9: Inclusive range pattern `lo..=hi`.
// The spec does not provide a concrete code example; this is derived from the
// semantic description: "A range pattern matches any value that falls within
// the range's bounds."
fn range_inclusive(x: i32) -> i32 {
    match x {
        1..=3 => 1,
        4..=6 => 2,
        _ => 0,
    }
}

// FLS §5.1.9: Exclusive range pattern `lo..hi`.
// FLS §5.1.9 AMBIGUOUS: The spec describes range patterns but does not
// explicitly distinguish whether `..` and `..=` are both valid in all
// positions. Galvanic supports both forms per the Rust Reference.
fn range_exclusive(x: i32) -> i32 {
    match x {
        1..4 => 1,
        4..7 => 2,
        _ => 0,
    }
}

// FLS §5.1.9: Range pattern with negative lower bound.
// FLS §5.2: Negative literal patterns are valid as range bounds.
fn range_negative(x: i32) -> i32 {
    match x {
        -5..=-1 => 1,
        0 => 2,
        _ => 3,
    }
}

fn main() -> i32 {
    // FLS §5.1.9: inclusive range — value 2 in [1,3] → 1.
    let a = range_inclusive(2);
    // FLS §5.1.9: exclusive range — value 5 in [4,7) → 2.
    let b = range_exclusive(5);
    // FLS §5.1.9: negative range — value -3 in [-5,-1] → 1.
    let c = range_negative(-3);
    // a=1, b=2, c=1 → sum=4; exit 4 to signal correct execution.
    a + b + c
}
