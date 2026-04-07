// FLS §6.4.4 — Unsafe Block Expressions
//
// An unsafe block expression is a block expression preceded by keyword `unsafe`.
// It marks the enclosed code as a context where operations restricted by the
// safety model are permitted.
//
// FLS §6.4.4: "An unsafe block expression is a block expression preceded by
// keyword unsafe."
//
// FLS §6.4.4 AMBIGUOUS: The spec permits raw pointer dereferences, unsafe fn
// calls, mutable static access, and union field access inside unsafe blocks.
// None of these are implemented in galvanic at this milestone. The unsafe marker
// is syntactically accepted; its semantic boundary is not enforced.
//
// Source: FLS §6.4.4 structure; no verbatim example provided by the spec for
// this section.

fn add(a: i32, b: i32) -> i32 {
    unsafe { a + b }
}

fn main() -> i32 {
    let x = unsafe { 3 + 4 };
    add(x, unsafe { 1 })
}
