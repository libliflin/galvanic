// FLS §6.11, §6.18 — Match expression as nested struct field initializer.
//
// `Outer { inner: match flag { 1 => Inner { a: 1 }, _ => Inner { a: 2 } }, c: 3 }`
// The scrutinee is evaluated once at runtime; arms are tested in source order.
// FLS §6.11: field initializers are arbitrary expressions (not restricted to literals).
// FLS §6.18: match arms are tried in source order; last arm is the default.

struct Inner {
    a: i32,
    b: i32,
}

struct Outer {
    inner: Inner,
    c: i32,
}

fn main() -> i32 {
    let flag = 1;
    let o = Outer {
        inner: match flag {
            1 => Inner { a: 10, b: 20 },
            _ => Inner { a: 30, b: 40 },
        },
        c: 99,
    };
    o.inner.a + o.c
}
