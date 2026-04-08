// FLS §12.1: Generic type parameters with trait bounds.
// FLS §4.14: Trait and lifetime bounds.
//
// A generic function may declare type parameters with trait bounds:
//   fn apply<T: SomeTrait>(t: T) -> ReturnType { ... }
//
// At call sites, galvanic monomorphizes the function for each concrete
// struct type used as the argument. The bound constrains which types are
// valid, and the method call on `t` dispatches through the concrete type's
// trait implementation.
//
// FLS §12.1: AMBIGUOUS — The spec specifies that type parameters must
// satisfy their bounds, but does not specify the monomorphization strategy.
// Galvanic infers the concrete type from the call-site argument type.

trait Scalable {
    fn scale(&self, factor: i32) -> i32;
}

struct Foo {
    val: i32,
}

impl Scalable for Foo {
    fn scale(&self, factor: i32) -> i32 {
        self.val * factor
    }
}

fn apply_scale<T: Scalable>(t: T, n: i32) -> i32 {
    t.scale(n)
}

fn main() -> i32 {
    let f = Foo { val: 3 };
    apply_scale(f, 4)
}
