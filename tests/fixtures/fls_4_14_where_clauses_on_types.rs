// FLS §4.14: Trait and lifetime bounds — where clauses on type definitions.
//
// Where clauses are permitted on struct, enum, and trait definitions in
// addition to functions and impl blocks. They constrain the generic type
// parameters that may be used with that type.
//
// FLS §4.14: AMBIGUOUS — The spec does not specify when where-clause bounds
// on struct/enum/trait definitions are checked (parse time, type-check time,
// or monomorphization time). Galvanic parses and discards where-clause bounds;
// the concrete type is inferred from call-site argument types.

trait Getter {
    fn get(&self) -> i32;
}

// Struct with a where clause on the definition.
// FLS §4.14: `struct Name<T> where T: Bound { ... }`
struct Wrapper<T> where T: Getter {
    val: T,
}

// Enum with a where clause on the definition.
// FLS §4.14: `enum Name<T> where T: Bound { ... }`
enum Maybe<T> where T: Getter {
    Some(T),
    None,
}

// Trait with a where clause on the definition.
// FLS §4.14: `trait Name where Self: Bound { ... }`
trait Transform where Self: Sized {
    fn transform(&self, n: i32) -> i32;
}

struct Foo {
    x: i32,
}

impl Getter for Foo {
    fn get(&self) -> i32 { self.x }
}

impl<T> Wrapper<T> where T: Getter {
    fn inner(&self) -> i32 { self.val.get() }
}

impl Transform for Foo {
    fn transform(&self, n: i32) -> i32 { self.x + n }
}

fn get_maybe<T>(m: Maybe<T>) -> i32 where T: Getter {
    match m {
        Maybe::Some(v) => v.get(),
        Maybe::None => 0,
    }
}

fn apply<T: Transform>(t: T, n: i32) -> i32 { t.transform(n) }

fn main() -> i32 {
    let w = Wrapper { val: Foo { x: 5 } };
    let m = Maybe::Some(Foo { x: 7 });
    let f = Foo { x: 3 };
    w.inner() + get_maybe(m) + apply(f, 4)
}
