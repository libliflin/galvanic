// FLS §6.16 — Range expressions from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/range-expressions.html
//
// Covers:
//   §6.16 — Range expression (exclusive): start..end
//   §6.16 — Range expression (inclusive): start..=end
//   §5.1.9 — Range patterns (exclusive and inclusive) in match arms
//   §5.1.9 — Range patterns in if-let and while-let
//
// Note: Open-ended ranges (..end, start.., ..) are not yet implemented in
// galvanic. §6.16 NOTE: RangeFull/RangeTo/RangeFrom forms produce parse
// errors; only two-sided Range and RangeInclusive are accepted.
//
// Parameters are used in bounds to verify runtime codegen intent — the
// range iterator must evaluate bounds at runtime, not constant-fold.

// FLS §6.16: Exclusive range expression (start..end) as for-loop iterator.
// The range evaluates start and end at runtime; iteration emits a runtime loop.
fn for_exclusive_range(n: i32) -> i32 {
    let mut acc = 0;
    // FLS §6.16: start..end — exclusive upper bound (end not included).
    for x in 0..n {
        acc = acc + x;
    }
    acc
}

// FLS §6.16: Inclusive range expression (start..=end) as for-loop iterator.
fn for_inclusive_range(n: i32) -> i32 {
    let mut acc = 0;
    // FLS §6.16: start..=end — inclusive upper bound (end is included).
    for x in 0..=n {
        acc = acc + x;
    }
    acc
}

// FLS §6.16 + §5.1.9: Exclusive range pattern in match.
// A range pattern matches values v where start <= v < end.
fn classify_exclusive(x: i32) -> i32 {
    match x {
        // FLS §5.1.9: RangePatternBound..RangePatternBound (exclusive).
        0..5 => 1,
        5..10 => 2,
        _ => 0,
    }
}

// FLS §6.16 + §5.1.9: Inclusive range pattern in match.
// A range pattern matches values v where start <= v <= end.
fn classify_inclusive(x: i32) -> i32 {
    match x {
        // FLS §5.1.9: RangePatternBound..=RangePatternBound (inclusive).
        1..=5 => 1,
        6..=10 => 2,
        _ => 0,
    }
}

// FLS §5.1.9: Negative range bounds in match pattern.
fn negative_range_pattern(x: i32) -> i32 {
    match x {
        // FLS §5.1.9: range pattern with negative lower bound.
        -10..=-1 => 1,
        0 => 2,
        1..=10 => 3,
        _ => 0,
    }
}

// FLS §5.1.9: Range pattern in if-let expression (§6.17 + §5.1.9).
fn if_let_range(x: i32) -> i32 {
    // FLS §5.1.9: inclusive range pattern used as if-let scrutinee pattern.
    if let 1..=100 = x {
        1
    } else {
        0
    }
}

// FLS §5.1.9: Exclusive range pattern in if-let.
fn if_let_exclusive_range(x: i32) -> i32 {
    // FLS §5.1.9: exclusive range pattern in if-let.
    if let 0..10 = x {
        1
    } else {
        0
    }
}

// FLS §6.16: Range expression with non-zero start.
fn for_range_nonzero_start(lo: i32, hi: i32) -> i32 {
    let mut acc = 0;
    // FLS §6.16: both bounds are runtime values (parameters).
    for x in lo..hi {
        acc = acc + x;
    }
    acc
}

// FLS §6.16: Range expression with non-zero start (inclusive).
fn for_range_nonzero_start_inclusive(lo: i32, hi: i32) -> i32 {
    let mut acc = 0;
    // FLS §6.16: both bounds are runtime values; inclusive upper bound.
    for x in lo..=hi {
        acc = acc + x;
    }
    acc
}

// FLS §5.1.9: Range pattern in while-let.
fn while_let_range_counts(start: i32) -> i32 {
    let mut x = start;
    let mut steps = 0;
    // FLS §5.1.9: range pattern in while-let — loops while x matches 1..=10.
    while let 1..=10 = x {
        steps = steps + 1;
        x = x - 1;
    }
    steps
}

// FLS §5.1.9 + §6.16: Mixed exclusive and inclusive range patterns in match.
// Documents that both forms can coexist in a single match expression.
fn mixed_range_patterns(x: i32) -> i32 {
    match x {
        // FLS §5.1.9: exclusive range pattern — does not include upper bound.
        0..5 => 1,
        // FLS §5.1.9: inclusive range pattern — includes upper bound.
        5..=9 => 2,
        // FLS §5.1.9: single-value range (degenerate case, both bounds equal).
        10..=10 => 3,
        _ => 0,
    }
}

fn main() {
    let _ = for_exclusive_range(5);
    let _ = for_inclusive_range(4);
    let _ = classify_exclusive(3);
    let _ = classify_inclusive(7);
    let _ = negative_range_pattern(-5);
    let _ = if_let_range(50);
    let _ = if_let_exclusive_range(5);
    let _ = for_range_nonzero_start(2, 6);
    let _ = for_range_nonzero_start_inclusive(2, 5);
    let _ = while_let_range_counts(8);
    let _ = mixed_range_patterns(5);
}
