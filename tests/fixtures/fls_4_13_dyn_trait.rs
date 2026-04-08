// FLS §4.13: Trait objects — dyn Trait.
//
// A value of type `&dyn Trait` is a fat pointer: a pair of (data pointer,
// vtable pointer), each 8 bytes, occupying 2 consecutive ARM64 registers.
//
// The vtable is a read-only array of function pointers in `.rodata`, one per
// trait method in declaration order. Method dispatch loads the function pointer
// from the vtable at `method_index * 8` and calls it indirectly via `blr`.
//
// A vtable shim adapts the vtable calling convention (single data pointer in
// x0) to the concrete method's calling convention (N struct fields in
// x0..x{N-1}). The shim saves x0, loads each field, and tail-calls the
// concrete method via `b`.
//
// FLS §4.13: AMBIGUOUS — The FLS does not specify the vtable layout, fat
// pointer representation, or vtable shim calling convention. Galvanic uses the
// design above as an implementation choice.

trait Shape {
    fn area(&self) -> i32;
}

struct Circle { r: i32 }

impl Shape for Circle {
    fn area(&self) -> i32 { self.r * self.r }
}

fn print_area(s: &dyn Shape) -> i32 {
    s.area()
}

fn main() -> i32 {
    let c = Circle { r: 5 };
    print_area(&c)
}
