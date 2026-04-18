// FLS §6.11, §6.4 — Struct expressions: nested struct field initializer as block expression.
//
// FLS §6.11: Field initializers are arbitrary expressions. When the field type
// is itself a struct, the initializer may be a block expression whose tail
// expression evaluates to the nested struct type.
//
// FLS §6.4: "A block expression evaluates to the value of its final expression."
// Statements within the block execute in order before the tail is evaluated.
//
// This fixture exercises `store_nested_struct_lit` with ExprKind::Block,
// including statements inside the block that execute before the tail struct
// literal is stored.
//
// FLS §6.1.2:37–45: All stores are runtime — no const folding.

struct Inner { a: i32 }
struct Outer { inner: Inner, c: i32 }

fn compute(x: i32) -> i32 { x + 1 }

fn main() -> i32 {
    // The block contains a statement (`let y = ...`) and a tail struct literal.
    // Statements must lower before the tail is stored into the nested field slot.
    let o = Outer {
        inner: {
            let y = compute(6);
            Inner { a: y }
        },
        c: 3,
    };
    o.inner.a
}
