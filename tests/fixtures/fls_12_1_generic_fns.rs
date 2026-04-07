// FLS §12.1 — Generic functions.
//
// A generic function declares one or more type parameters in angle brackets
// after the name. Each call site is monomorphized with the concrete types.

fn identity<T>(x: T) -> T {
    x
}

fn first<T>(a: T, b: T) -> T {
    a
}

fn add_one(n: i32) -> i32 {
    identity(n) + 1
}

fn main() {
    let a = identity(42);
    let b = first(10, 20);
    let c = add_one(5);
}
