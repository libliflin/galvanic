// FLS §10.2: Associated Types.
//
// An associated type is a type item declared in a trait body with `type Name;`.
// Implementors of the trait must provide a concrete definition `type Name = Ty;`
// in their impl block.
//
// FLS §10.2 AMBIGUOUS: The spec does not provide standalone code examples for
// associated types in §10.2; this fixture is derived from the semantic
// descriptions in §10.2 and cross-references to §10 (Associated Items),
// §11 (Implementations), and §13 (Traits).
//
// At this milestone, associated type declarations and definitions are parsed
// and stored in the AST but are not used in code generation. Method signatures
// use concrete types directly; the associated type annotation is structural.

// FLS §13, §10.2: A trait may declare a required associated type.
// Implementors must supply a concrete type via `type Name = Ty;`.
trait Container {
    // FLS §10.2: Required associated type — no default.
    type Item;
    fn get(&self) -> i32;
}

struct Wrapper {
    x: i32,
}

// FLS §11.1, §10.2: Trait impl provides the associated type definition.
impl Container for Wrapper {
    type Item = i32;
    fn get(&self) -> i32 {
        self.x
    }
}

// FLS §11.2, §10.2: Inherent impl may also declare associated types
// (as named type aliases scoped to the type).
struct Counter {
    count: i32,
}

impl Counter {
    // FLS §10.2: Associated type definition in an inherent impl.
    type Value = i32;
    fn value(&self) -> i32 {
        self.count
    }
}

// FLS §10.2 + §10.3: A trait may declare both associated types and
// associated constants in the same body.
trait Versioned {
    type Kind;
    const VERSION: i32;
    fn version(&self) -> i32;
}

struct Widget {
    v: i32,
}

impl Versioned for Widget {
    type Kind = i32;
    const VERSION: i32 = 3;
    fn version(&self) -> i32 {
        Widget::VERSION + self.v
    }
}

fn main() -> i32 {
    let w = Wrapper { x: 10 };
    let c = Counter { count: 7 };
    let wg = Widget { v: 2 };
    // w.get()=10, c.value()=7, wg.version()=VERSION(3)+v(2)=5
    w.get() + c.value() + wg.version()
}
