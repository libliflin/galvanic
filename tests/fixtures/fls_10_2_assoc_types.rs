// FLS §10.2: Associated Types.
//
// Associated types declare a type placeholder in a trait body. Each
// implementation of the trait provides a concrete type binding. This decouples
// the trait interface from the concrete type chosen by each implementor.
//
// FLS §10.2: "An associated type is a type alias declared in a trait."
// FLS §10.2: "Each implementation of the trait must provide a type binding
// for each abstract associated type declared in the trait."
//
// This fixture is derived from the associated-type pattern in the FLS §10.2
// examples. The methods use concrete types in the impl blocks (not `Self::Item`
// in return type position) because `Self` in type annotations requires
// additional type resolution (future work: see `Self::X` in lower.rs).

trait Shape {
    type Area;
    fn scaled_area(&self, scale: i32) -> i32;
}

struct Square {
    side: i32,
}

impl Shape for Square {
    type Area = i32;
    fn scaled_area(&self, scale: i32) -> i32 {
        self.side * self.side * scale
    }
}

struct Rectangle {
    width: i32,
    height: i32,
}

impl Shape for Rectangle {
    type Area = i32;
    fn scaled_area(&self, scale: i32) -> i32 {
        self.width * self.height * scale
    }
}

fn main() -> i32 {
    let s = Square { side: 3 };
    let r = Rectangle { width: 4, height: 5 };
    s.scaled_area(2) + r.scaled_area(1)
}
