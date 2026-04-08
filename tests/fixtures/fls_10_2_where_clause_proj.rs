// FLS §10.2 / §4.14: Where clause predicates on associated type projections.
//
// A where clause of the form `where C::Item: Trait` constrains the associated
// type `Item` of the generic parameter `C` to implement `Trait`. This is
// distinct from `C: Container<Item = i32>` (which equates the associated type
// to a concrete type) — here the associated type is merely required to satisfy
// a trait bound.
//
// FLS §4.14: "Where clause predicates impose constraints on subject types."
// FLS §10.2: "An associated type is a type alias declared in a trait."
//
// FLS §10.2 / §4.14: AMBIGUOUS — The FLS does not specify how the compiler
// must verify `where C::Item: Trait` at call sites, nor how method calls on
// `C::Item`-typed values dispatch when the type is only known through the
// where clause. Galvanic parses the predicate and relies on monomorphization:
// at each call site, the concrete type of `C::Item` is known, and dispatch
// proceeds as for any concrete struct type.
//
// This fixture is derived from the associated-type and where-clause patterns
// in FLS §10.2 and §4.14.

trait Container {
    type Item;
    fn get_val(&self) -> i32;
}

// A marker trait — just a bound, no required methods in this fixture.
trait Marker {}

struct Holder { val: i32 }

impl Container for Holder {
    type Item = i32;
    fn get_val(&self) -> i32 { self.val }
}

// i32 satisfies the Marker bound.
impl Marker for i32 {}

// A generic function with a where clause predicate on the associated type.
// FLS §4.14: `where C::Item: Marker` constrains the projected type.
fn process<C: Container>(c: C) -> i32 where C::Item: Marker {
    c.get_val() * 2
}

fn main() -> i32 {
    let h = Holder { val: 5 };
    process(h)
}
