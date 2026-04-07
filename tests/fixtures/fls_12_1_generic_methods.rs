// FLS §12.1 — Generic methods in impl blocks.
//
// A generic method declares one or more type parameters in angle brackets
// after the method name. Each call site is monomorphized with the concrete
// types inferred from the arguments.
//
// FLS §12.1: "A generic function may declare one or more type parameters."
// Methods inside impl blocks are functions in the FLS sense; they may also
// declare type parameters and are subject to the same monomorphization rules.

struct Wrapper {
    val: i32,
}

impl Wrapper {
    // Single type parameter: apply passes through the argument unchanged.
    fn apply<T>(&self, x: T) -> T {
        x
    }

    // Generic method accessing self field alongside the type-erased argument.
    fn add_val<T>(&self, x: T) -> i32 {
        self.val + x
    }

    // Two type parameters: pick_first returns the first argument.
    fn pick_first<T, U>(&self, a: T, _b: U) -> T {
        a
    }
}

fn use_wrapper(n: i32) -> i32 {
    let w = Wrapper { val: 10 };
    w.apply(n)
}

fn main() {
    let w = Wrapper { val: 3 };
    let a = w.apply(7);
    let b = w.add_val(4);
    let c = w.pick_first(1, 99);
    let d = use_wrapper(5);
}
