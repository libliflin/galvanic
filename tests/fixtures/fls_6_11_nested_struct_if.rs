// FLS §6.11 — Struct Expressions: nested struct field initializer as if-expression.
//
// A field initializer in a struct literal is an arbitrary expression (FLS §6.11).
// When the field type is itself a struct, the initializer may be an if-expression
// that evaluates to the nested struct type at runtime.
//
// This fixture exercises `store_nested_struct_lit` with ExprKind::If.

struct Inner { a: i32, b: i32 }
struct Outer { inner: Inner, c: i32 }

fn main() -> i32 {
    let flag = 1;
    let o = Outer { inner: if flag > 0 { Inner { a: 1, b: 2 } } else { Inner { a: 3, b: 4 } }, c: 5 };
    o.inner.a
}
