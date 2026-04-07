// FLS §13: Traits.
//
// This fixture demonstrates the trait system including default method
// implementations (FLS §10.1.1). When a trait method has a default body,
// implementing types that do not override it inherit the default.
//
// FLS §13 AMBIGUOUS: The FLS does not provide concrete code examples for
// trait definitions in §13 itself; this fixture is derived from the
// semantic descriptions in §13 and from cross-references to §10.1 and §11.1.

// FLS §13: A trait item declares associated items that implementors must provide.
// The method signature has no body — the body is supplied by the impl.
trait Describe {
    fn describe(&self) -> i32;
}

// FLS §13: A trait may declare multiple method signatures.
//
// FLS §13 AMBIGUOUS: The spec does not state the maximum number of methods in
// a trait; we assume no limit.
trait Dimensions {
    fn width(&self) -> i32;
    fn height(&self) -> i32;
}

// FLS §11.1: A trait implementation provides method bodies for each signature.
// The syntax is `impl TraitName for TypeName { ... }`.
struct Box2d { w: i32, h: i32 }

impl Dimensions for Box2d {
    fn width(&self) -> i32 { self.w }
    fn height(&self) -> i32 { self.h }
}

impl Describe for Box2d {
    fn describe(&self) -> i32 { self.w * self.h }
}

// FLS §11: A type may have both inherent and trait impls.
impl Box2d {
    fn area(&self) -> i32 { self.w * self.h }
}

// FLS §13: Multiple types may implement the same trait independently.
struct Point { x: i32, y: i32 }

impl Describe for Point {
    fn describe(&self) -> i32 { self.x + self.y }
}

// FLS §10.1.1: Default method implementations.
//
// A trait may provide a default body for a method. Implementing types that
// do not override the method inherit the default behaviour. The default body
// may call other trait methods (via `self.method()`) — galvanic resolves
// these to the implementing type's concrete method at compile time.
//
// FLS §13 AMBIGUOUS: The FLS does not provide a concrete code example for
// default methods; this is derived from §10.1.1 semantic description.
trait Scaled {
    fn value(&self) -> i32;
    // Default method: calls self.value() — resolved statically.
    fn doubled(&self) -> i32 {
        self.value() * 2
    }
}

struct Half { n: i32 }
impl Scaled for Half {
    fn value(&self) -> i32 { self.n }
    // `doubled` is not overridden — default implementation is used.
}

// Entry point: exercises both explicit and default trait method calls.
//
// FLS §6.12.2: Method call expressions. `b.describe()` resolves to
// `Box2d__describe` at compile time (static dispatch).
fn main() -> i32 {
    let b = Box2d { w: 3, h: 4 };
    let p = Point { x: 1, y: 2 };
    let h = Half { n: 7 };
    // b.describe()=12, p.describe()=3, b.area()=12, h.doubled()=14
    b.describe() - p.describe() - b.area() + 9 + h.doubled() - 10
}
