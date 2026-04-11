// FLS §6.11 — Struct expressions from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/struct-expressions.html
//
// Covers:
//   §6.11 — Named field struct expressions (`S { field: value, ... }`)
//   §6.11 — Struct shorthand field initialization (`S { x }`)
//   §6.11 — Struct base/update syntax (`S { field: val, ..base }`)
//   §6.11 — Unit struct expressions (`Unit`)
//   §6.11 — Tuple struct expressions (`Pair(a, b)`)
//   §6.11 — Nested struct expressions (struct fields that are struct values)
//
// Parameters are used as field values throughout to document that struct
// construction evaluates its field expressions at runtime, not compile time.
//
// §6.11 NOTE: The struct expression syntax is distinct from the struct
// definition syntax (§3). This fixture exercises only the expression forms.

// FLS §6.11: Named field struct expression with explicit field: value syntax.
struct Point {
    x: i32,
    y: i32,
}

fn named_fields(a: i32, b: i32) -> i32 {
    // FLS §6.11: Named field struct expression.
    // Each field is specified as `name: expression`.
    let p = Point { x: a, y: b };
    p.x + p.y
}

// FLS §6.11: Shorthand field initialization when the variable name matches
// the field name. The expression `{ x, y }` is shorthand for `{ x: x, y: y }`.
fn shorthand_init(x: i32, y: i32) -> i32 {
    // FLS §6.11: Struct shorthand — each identifier names both the field and
    // the variable whose value initializes it.
    let p = Point { x, y };
    p.x + p.y
}

// FLS §6.11: Struct update syntax copies fields from a base expression for
// any fields not explicitly named. The `..base` clause must come last.
fn update_syntax(base: Point, new_x: i32) -> i32 {
    // FLS §6.11: `x` is explicitly set; `y` is copied from `base`.
    let p = Point { x: new_x, ..base };
    p.x + p.y
}

// FLS §6.11: Struct update syntax where no fields are explicitly overridden.
// All fields are copied from `base`.
fn update_no_overrides(base: Point) -> i32 {
    // FLS §6.11: All fields come from `base`; none are explicitly specified.
    let p = Point { ..base };
    p.x + p.y
}

// FLS §6.11: Unit struct expression — a struct type with no fields.
// The expression is just the struct name with no brace initializer.
struct Unit;

fn unit_struct_expr() -> i32 {
    // FLS §6.11: Unit struct expression; produces a value of type `Unit`.
    let _u = Unit;
    0
}

// FLS §6.11: Tuple struct expression — positional fields, parenthesized syntax.
struct Pair(i32, i32);

fn tuple_struct_expr(a: i32, b: i32) -> i32 {
    // FLS §6.11: Tuple struct expression with positional field values.
    let p = Pair(a, b);
    p.0 + p.1
}

// FLS §6.11: Nested struct expression — a struct whose fields are themselves
// struct-typed, constructed via struct expressions.
struct Rect {
    top_left: Point,
    bottom_right: Point,
}

fn nested_struct_expr(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    // FLS §6.11: The outer struct expression contains inner struct expressions
    // as the values of its fields.
    let r = Rect {
        top_left: Point { x: x1, y: y1 },
        bottom_right: Point { x: x2, y: y2 },
    };
    // Difference in coordinates — exercises field access on nested values.
    (r.bottom_right.x - r.top_left.x) + (r.bottom_right.y - r.top_left.y)
}

// FLS §6.11: Struct expression where field values are arbitrary expressions,
// not just variables. Any expression is valid as a field value.
fn complex_field_values(a: i32, b: i32) -> i32 {
    // FLS §6.11: Field values are computed expressions using the parameters.
    let p = Point {
        x: a * 2,
        y: b + 1,
    };
    p.x + p.y
}

// FLS §6.11: Mixed explicit fields and shorthand in a single expression.
fn mixed_shorthand(x: i32, b: i32) -> i32 {
    // FLS §6.11: `x` uses shorthand; `y` uses explicit `field: expr` form.
    let p = Point { x, y: b - 1 };
    p.x + p.y
}

fn main() {
    let _ = named_fields(3, 4);
    let _ = shorthand_init(3, 4);
    let _ = update_syntax(Point { x: 10, y: 20 }, 5);
    let _ = update_no_overrides(Point { x: 7, y: 8 });
    let _ = unit_struct_expr();
    let _ = tuple_struct_expr(1, 2);
    let _ = nested_struct_expr(0, 0, 3, 4);
    let _ = complex_field_values(5, 6);
    let _ = mixed_shorthand(3, 5);
}
