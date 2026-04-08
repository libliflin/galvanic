// FLS §4.14 — Supertrait bounds
//
// A trait may declare one or more supertraits. Every type that implements the
// subtrait must also implement all supertraits. Galvanic parses the supertrait
// bounds but does not enforce the constraint at the type-system level — the
// monomorphization system naturally resolves supertrait method calls to the
// concrete type's implementation labels.
//
// FLS §4.14 AMBIGUOUS: The spec does not specify how supertrait method
// availability is propagated to generic call sites. Galvanic's approach:
// `t.base_method()` on a generic `T: Derived` resolves via monomorphization to
// `T__base_method`, which exists because the concrete type implements the
// supertrait.

trait Base {
    fn base_val(&self) -> i32;
}

trait Derived: Base {
    fn derived_val(&self) -> i32;
}

struct Foo { x: i32 }

impl Base for Foo {
    fn base_val(&self) -> i32 { self.x }
}

impl Derived for Foo {
    fn derived_val(&self) -> i32 { self.x + 1 }
}

fn main() -> i32 {
    let f = Foo { x: 5 };
    f.base_val()
}
