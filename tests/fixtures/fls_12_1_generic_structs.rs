// Fixture: Generic struct definitions — FLS §12.1
//
// FLS §12.1: "A generic struct may declare one or more type parameters."
// The type parameters appear in angle brackets after the struct name.
// Each field whose type is a type parameter is monomorphized to the
// concrete type at each use site.
//
// Galvanic currently supports scalar (integer/bool) type parameters.
// Float-typed generic structs are deferred to a later milestone.
//
// FLS §12.1 AMBIGUOUS: The spec does not specify the exact disambiguation
// rule for `<` after a struct name (generic list vs. less-than). Galvanic
// follows rustc's precedent: `<` immediately after a struct name always
// opens a generic parameter list.

/// Single type parameter — the simplest generic struct.
struct Wrapper<T> {
    value: T,
}

/// Two type parameters — both fields are generic.
struct Pair<T, U> {
    first: T,
    second: U,
}

/// Generic struct mixed with a concrete field.
struct Tagged<T> {
    tag: i32,
    data: T,
}

fn main() -> i32 {
    let w = Wrapper { value: 42 };
    let p = Pair { first: 3, second: 7 };
    let t = Tagged { tag: 1, data: 10 };
    w.value + p.first + p.second + t.tag + t.data
}
