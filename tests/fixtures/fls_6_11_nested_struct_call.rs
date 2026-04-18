// FLS §6.11 — Struct expressions: field initializer as struct-returning call.
//
// FLS §6.11: "StructExpression.FieldInitializerList defines individual
// FieldInitializers for the FieldList of the related struct." Field
// initializers are expressions — not restricted to struct literals.
//
// This program tests that a nested struct field initializer may be an
// arbitrary expression: a function call returning the field's struct type.
//
// FLS §6.11 AMBIGUOUS: Evaluation order of field initializers is not
// specified when initializers have side effects. Galvanic evaluates
// left-to-right. See refs/fls-ambiguities.md §6.11.
//
// FLS §6.1.2:37–45: All stores are runtime — no const folding.

struct Point {
    x: i32,
    y: i32,
}

struct Rect {
    top_left: Point,
    width: i32,
    height: i32,
}

fn make_point(x: i32, y: i32) -> Point {
    Point { x, y }
}

fn main() -> i32 {
    // FLS §6.11: nested struct field `top_left` initialised by a call.
    let r = Rect { top_left: make_point(3, 4), width: 10, height: 5 };
    // Access nested fields to confirm values were stored correctly.
    r.top_left.x + r.top_left.y
}
