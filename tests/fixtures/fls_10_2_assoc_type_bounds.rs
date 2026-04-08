// FLS §10.2: Associated type bindings in trait bounds.
// FLS §12.1: Generic functions with constrained associated types.
//
// A generic type parameter may constrain an associated type via angle-bracket
// syntax in the trait bound:
//
//   fn extract<T: Container<Item = i32>>(c: T) -> i32
//
// This pattern appears throughout the Rust standard library (Iterator, etc.).
// Galvanic parses and discards the `<Item = i32>` binding — monomorphization
// uses the call-site concrete type, not the bound annotation, to resolve the
// method implementation.
//
// FLS §10.2: AMBIGUOUS — The spec does not specify how associated type bindings
// in bounds interact with type inference or whether a compiler must verify that
// the provided concrete type satisfies the binding. Galvanic does not verify
// this constraint; it trusts the programmer's annotation and resolves methods
// at the monomorphization call site.

trait Container {
    type Item;
    fn get_val(&self) -> i32;
}

struct Wrapper {
    val: i32,
}

impl Container for Wrapper {
    type Item = i32;
    fn get_val(&self) -> i32 {
        self.val
    }
}

struct Doubler {
    val: i32,
}

impl Container for Doubler {
    type Item = i32;
    fn get_val(&self) -> i32 {
        self.val * 2
    }
}

fn extract<T: Container<Item = i32>>(c: T) -> i32 {
    c.get_val()
}

fn main() -> i32 {
    let w = Wrapper { val: 7 };
    let d = Doubler { val: 5 };
    extract(w) + extract(d)
}
