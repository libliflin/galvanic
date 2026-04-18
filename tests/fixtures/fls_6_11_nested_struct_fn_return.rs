// FLS §6.11 — Nested struct field initializer in function-return position.
//
// `fn f() -> Outer { Outer { inner: make_inner(), ... } }` requires
// lower_struct_expr_into's StructLit arm to route nested-struct-typed fields
// through store_nested_struct_lit rather than lower_expr.
//
// Derived from FLS §6.11 (Struct Expressions): field initializers are
// arbitrary expressions.

struct Inner {
    a: i32,
    b: i32,
}

struct Outer {
    inner: Inner,
    c: i32,
}

fn make_inner(x: i32) -> Inner {
    Inner { a: x, b: x + 1 }
}

fn make_outer(n: i32) -> Outer {
    Outer { inner: make_inner(n), c: n + 10 }
}

fn main() -> i32 {
    let o = make_outer(3);
    o.inner.a + o.c
}
