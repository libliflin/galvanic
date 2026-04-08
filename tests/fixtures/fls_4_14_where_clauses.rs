// FLS §4.14: Trait and lifetime bounds — where clauses.
//
// A `where` clause moves trait bounds out of the generic parameter list and
// into a separate clause after the function signature or impl header:
//
//   fn apply<T>(t: T, n: i32) -> i32 where T: Scalable { t.scale(n) }
//   impl<T> Wrapper<T> where T: Scalable { ... }
//
// This is equivalent to inline bounds (`fn apply<T: Scalable>(...)`) and
// enables more complex multi-predicate constraints.
//
// FLS §4.14: AMBIGUOUS — The spec does not specify whether where-clause bounds
// are checked at parse time, type-check time, or monomorphization time.
// Galvanic parses and discards where-clause bounds; the concrete type is
// inferred from the call-site argument type during monomorphization.

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

// `where` clause on a function — equivalent to inline bound
fn apply_scale<T>(t: T, n: i32) -> i32 where T: Scalable {
    t.scale(n)
}

// `where` clause with multiple bounds (parsed and discarded)
trait Getter {
    fn get(&self) -> i32;
}

struct Bar {
    x: i32,
}

impl Getter for Bar {
    fn get(&self) -> i32 { self.x }
}

fn get_and_scale<T>(t: T) -> i32 where T: Getter {
    t.get()
}

fn main() -> i32 {
    let f = Foo { val: 3 };
    let b = Bar { x: 7 };
    apply_scale(f, 4) + get_and_scale(b)
}
