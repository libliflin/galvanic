// FLS §6.18 — Match Expressions
// Source: https://rust-lang.github.io/fls/expressions/match-expr.html
//
// Each function uses a parameter as scrutinee so galvanic must emit runtime
// comparison instructions rather than constant-fold the match at compile time.
// This is the "compiler not interpreter" requirement from fls-constraints.md.
//
// FLS §6.18: "A match expression evaluates a subject expression and compares the
// resulting value with the patterns of each match arm. The first arm whose pattern
// matches the value is selected."

// FLS §6.18: Match on i32 literal patterns — single-value arms.
// FLS §6.18: "A match arm guard is an additional condition that must hold for the
// arm to be selected." (no guard here — plain literal arms)
fn match_i32_literals(x: i32) -> i32 {
    match x {  // FLS §6.18: subject expression is a place expression used in value context
        0 => 10,
        1 => 20,
        2 => 30,
        _ => 0,  // FLS §6.18: wildcard pattern matches any remaining value
    }
}

// FLS §6.18: Match as a value-producing expression (tail position = returned value).
// FLS §6.18: "A match expression is a value expression."
fn match_as_value(x: i32) -> i32 {
    // FLS §6.18: the match expression itself is the returned value — not stored first.
    match x {
        n if n > 0 => 1,   // FLS §6.18: arm guard `if n > 0`
        n if n < 0 => -1,  // FLS §6.18: arm guard on second arm
        _ => 0,
    }
}

// FLS §6.18: Match on boolean scrutinee.
// FLS §6.18: "The subject expression may be of any type."
fn match_bool(b: bool) -> i32 {
    match b {  // FLS §6.18: bool scrutinee
        true => 1,
        false => 0,
    }
}

// FLS §6.18: Match with or-patterns in a single arm.
// FLS §5.1.11: Or-pattern `p0 | p1` — matches if either sub-pattern matches.
fn match_or_pattern(x: i32) -> i32 {
    match x {
        1 | 2 | 3 => 1,  // FLS §5.1.11: or-pattern as match arm pattern
        4 | 5 => 2,
        _ => 0,
    }
}

// FLS §6.18: Match on enum variants — unit variants.
// FLS §6.18: "An enum variant pattern matches a specific enum variant."
enum Direction { North, South, East, West }

fn match_unit_enum(d: Direction) -> i32 {
    match d {
        Direction::North => 0,
        Direction::South => 1,
        Direction::East => 2,
        Direction::West => 3,
    }
}

// FLS §6.18: Match on enum with tuple variant — destructures fields.
// FLS §5.1.7: Tuple struct pattern matches a tuple-like enum variant.
enum Payload { Value(i32), Empty }

fn match_tuple_variant(p: Payload) -> i32 {
    match p {
        Payload::Value(n) => n,   // FLS §5.1.7: tuple variant pattern binds field
        Payload::Empty => -1,
    }
}

// FLS §6.18: Match on enum with named-field variant.
// FLS §5.1.6: Struct pattern matches named fields by name.
enum Shape { Circle { radius: i32 }, Square { side: i32 } }

fn match_named_variant(s: Shape) -> i32 {
    match s {
        Shape::Circle { radius } => radius,  // FLS §5.1.6: named field binding
        Shape::Square { side } => side,
    }
}

// FLS §6.18: Match expression in a let binding (not at tail position).
// FLS §6.18: match is an expression and may appear wherever an expression is valid.
fn match_in_let(x: i32) -> i32 {
    let result = match x {  // FLS §6.18: match used as initializer of let
        0 => 100,
        _ => 200,
    };
    result + 1
}

// FLS §6.18: Match on a tuple scrutinee.
// FLS §5.1.5: Tuple pattern — `(p0, p1, ...)`.
fn match_tuple(x: i32, y: i32) -> i32 {
    match (x, y) {  // FLS §6.18: tuple expression as scrutinee
        (0, 0) => 0,
        (1, _) => 1,  // FLS §5.1: wildcard in tuple position
        (_, 1) => 2,
        _ => 3,
    }
}

// FLS §6.18: Match with range patterns.
// FLS §5.1.9: Range pattern `lo..=hi` (inclusive).
fn match_ranges(x: i32) -> i32 {
    match x {
        -1000..=-1 => -1,  // FLS §5.1.9: range pattern with negative lower bound
        0 => 0,
        1..=100 => 1,      // FLS §5.1.9: inclusive range pattern
        _ => 2,
    }
}

// FLS §6.18: Match arm guard that references a binding from the pattern.
// FLS §6.18: "The arm guard may reference the bindings introduced by the pattern."
fn match_guard_uses_binding(x: i32) -> i32 {
    match x {
        n if n % 2 == 0 => 0,  // FLS §6.18: guard references binding `n`
        n if n % 3 == 0 => 1,  // FLS §6.18: second guard on different arm
        _ => 2,
    }
}

// FLS §6.18: Nested match expressions.
// FLS §6.18: A match arm body is a block or expression — may itself be a match.
fn nested_match(x: i32, y: i32) -> i32 {
    match x {
        0 => match y {  // FLS §6.18: nested match in arm body
            0 => 0,
            _ => 1,
        },
        _ => 2,
    }
}

fn main() {}
