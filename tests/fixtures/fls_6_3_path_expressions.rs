// FLS §6.3 — Path expressions from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/path-expressions.html
//
// Covers:
//   §6.3 — Path expression: resolves a path to the value it denotes
//   §6.3 — Simple path expressions: single-segment identifier paths
//   §6.3 — Multi-segment path expressions: two-segment qualified paths
//   §6.3 — Function item paths used as function pointer values
//   §6.3 — Enum variant paths in expression position
//   §6.3 — Constant item paths
//
// §6.3 NOTE: Qualified paths (<Type as Trait>::item syntax) are not yet
// implemented in galvanic. Only simple and two-segment paths are supported.
//
// Parameters are used as inputs to document that paths resolve at runtime,
// not constant-folded from surrounding context.

// FLS §6.3: A simple path expression is a single-segment identifier that
// resolves to a local variable or parameter binding.
fn path_simple_ident(x: i32) -> i32 {
    // FLS §6.3: `x` is a simple path expression resolving to the parameter.
    let y = x;
    // FLS §6.3: `y` is a simple path expression resolving to the local binding.
    y
}

// FLS §6.3: A path expression resolving to a constant item.
const BASE: i32 = 100;

fn path_const_item(x: i32) -> i32 {
    // FLS §6.3: `BASE` is a path expression resolving to the constant item.
    x + BASE
}

// FLS §6.3: A path expression resolving to a static item.
static OFFSET: i32 = 5;

fn path_static_item(x: i32) -> i32 {
    // FLS §6.3: `OFFSET` is a path expression resolving to the static item.
    x + OFFSET
}

// FLS §6.3: A two-segment path expression resolves to an associated function.
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn new(px: i32, py: i32) -> Point {
        Point { x: px, y: py }
    }

    fn sum(&self) -> i32 {
        self.x + self.y
    }
}

fn path_assoc_fn(a: i32, b: i32) -> i32 {
    // FLS §6.3: `Point::new` is a two-segment path expression in call position.
    let p = Point::new(a, b);
    // FLS §6.3: `p` is a simple path resolving to the local binding.
    p.sum()
}

// FLS §6.3: A path expression can resolve to an enum variant.
enum Direction {
    North,
    South,
    East,
    West,
}

fn path_enum_variant(d: Direction) -> i32 {
    match d {
        // FLS §6.3: `Direction::North` etc. are two-segment path expressions
        // resolving to unit enum variants.
        Direction::North => 0,
        Direction::South => 1,
        Direction::East => 2,
        Direction::West => 3,
    }
}

// FLS §6.3: A path expression resolves to a tuple enum variant constructor.
enum Wrapper {
    Val(i32),
    Empty,
}

fn path_tuple_variant(n: i32) -> i32 {
    // FLS §6.3: `Wrapper::Val` is a two-segment path expression that, when called,
    // constructs the tuple variant.
    let w = Wrapper::Val(n);
    match w {
        // FLS §6.3: `Wrapper::Val` in pattern position resolves to the variant.
        Wrapper::Val(v) => v,
        Wrapper::Empty => 0,
    }
}

// FLS §6.3: A path expression can resolve to a function item, usable as a
// function pointer value (FLS §4.6, fn pointer types).
fn square(n: i32) -> i32 {
    n * n
}

fn path_fn_item_as_value(x: i32) -> i32 {
    // FLS §6.3: `square` is a path expression resolving to a function item,
    // coerced to a function pointer. The function is called at runtime.
    let f: fn(i32) -> i32 = square;
    f(x)
}

// FLS §6.3: Multiple path forms composed together. Each identifier is a
// separate path expression resolved at each point.
fn path_multiple_bindings(a: i32, b: i32) -> i32 {
    // FLS §6.3: `a` and `b` are path expressions resolving to the parameters.
    let x = a;
    let y = b;
    // FLS §6.3: `x` and `y` are path expressions resolving to local bindings.
    let z = x + y;
    // FLS §6.3: `z` is a path expression as the tail expression.
    z
}

fn main() {
    let _ = path_simple_ident(42);
    let _ = path_const_item(3);
    let _ = path_static_item(7);
    let _ = path_assoc_fn(10, 20);
    let _ = path_enum_variant(Direction::North);
    let _ = path_tuple_variant(99);
    let _ = path_fn_item_as_value(5);
    let _ = path_multiple_bindings(3, 4);
}
