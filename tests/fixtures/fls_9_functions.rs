// FLS §9 — Function examples from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/functions.html
//
// These are adapted to the subset galvanic currently handles (no generics,
// no traits, no struct types). Each function should lex, parse, and
// eventually compile.

fn main() {}

// FLS §9 example: a function with parameters and return type.
// (Adapted: original uses &Point and f64 method calls; simplified to
// arithmetic on primitives that galvanic can handle.)
fn add(a: i32, b: i32) -> i32 {
    a + b
}

// FLS §6.19 — Return expressions.
// Source: https://rust-lang.github.io/fls/expressions.html#return-expressions
fn max(left: i32, right: i32) -> i32 {
    if left > right {
        return left;
    }
    return right;
}

// Tail expression vs. semicolon distinction (FLS §6.4 block expressions).
fn returns_value() -> i32 {
    42
}

fn returns_unit() {
    42;
}

// Multiple parameters, nested arithmetic (FLS §6.5.6).
fn quadratic(a: i32, b: i32, c: i32, x: i32) -> i32 {
    a * x * x + b * x + c
}

// FLS §9:3: A function may call itself (recursive functions are permitted).
//
// The spec does not provide a fibonacci example, but recursive functions are
// explicitly permitted. This is the canonical example from the spec's
// discussion of function calls and recursion.
//
// FLS §9: Functions — recursive calls are ordinary call expressions.
// FLS §6.12.1: Call expressions.
// FLS §6.5.5: Arithmetic operator expressions (+, -).
// FLS §6.5.3: Comparison operator expressions (<=).
// FLS §6.17: If expressions.
//
// NOTE: no FLS example exists for fibonacci specifically. This function is
// derived from the spec's permission for recursion (§9:3) and the semantics
// of the operators it uses.
fn fib(n: i32) -> i32 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}

// FLS §10.1: Associated functions and methods.
//
// Associated functions are functions defined in an impl block that do not
// have a `self` parameter. They are called via `TypeName::fn_name(args)`.
//
// FLS §10.1: "An associated function is a function defined in an
// implementation." Associated functions without a self parameter are
// analogous to static methods in other languages.
//
// FLS §6.12.1: Two-segment path call expression `TypeName::fn_name(args)`.
//
// NOTE: The FLS does not provide a self-contained code example for associated
// functions in §10.1; this example is derived from the section's semantic
// description.

struct Rect { w: i32, h: i32 }

impl Rect {
    // Associated function (no self): constructs a Rect.
    // FLS §10.1: Associated functions with struct return type.
    fn new(width: i32, height: i32) -> Rect {
        Rect { w: width, h: height }
    }

    // Associated function (no self): scalar return.
    // FLS §10.1: Associated functions with primitive return type.
    fn area_of(width: i32, height: i32) -> i32 {
        width * height
    }

    // &self method — for accessing fields after construction.
    // FLS §10.1: Methods with a self parameter.
    fn area(&self) -> i32 {
        self.w * self.h
    }

    // FLS §10.1: &self method returning the impl type (struct value).
    //
    // The FLS §10.1 does not provide a code example for methods returning
    // struct types. This example is derived from the semantic description:
    // "An associated function is a function defined in an implementation."
    // Methods may return any type, including the implementing type.
    //
    // Galvanic uses the same register-packing convention as struct-returning
    // associated functions: the callee returns field values in x0..x{N-1}
    // via RetFields; the call site writes them to a new destination variable's
    // stack slots via CallMut.
    //
    // FLS §10.1 AMBIGUOUS: The spec does not define a calling convention for
    // methods with struct return types.
    fn scale(&self, factor: i32) -> Rect {
        Rect { w: self.w * factor, h: self.h * factor }
    }
}

// FLS §6.10, §9: Function returning a tuple type.
//
// The FLS does not provide a direct code example for tuple-returning functions
// in §9. This example is derived from the semantic description in §6.10
// (Tuple Expressions) combined with §9 (Functions):
// "A function may return any type, including a tuple type."
//
// FLS §6.10 AMBIGUOUS: The spec does not define a calling convention for
// tuple-returning functions. Galvanic extends the struct-return convention:
// element[0] returns in x0, element[1] returns in x1, etc.
fn minmax(a: i32, b: i32) -> (i32, i32) {
    if a < b { (a, b) } else { (b, a) }
}
