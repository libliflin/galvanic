// FLS §12.1 — Generic impl blocks: `impl<T> Type<T> { ... }`
//
// Methods in a generic impl block may reference the impl's type parameters
// in their signatures and bodies. At this milestone, all type params
// monomorphize to i32.

struct Pair<T> {
    first: T,
    second: T,
}

impl<T> Pair<T> {
    fn get_first(&self) -> T {
        self.first
    }
    fn get_second(&self) -> T {
        self.second
    }
    fn sum(&self) -> i32 {
        self.first + self.second
    }
}

fn main() -> i32 {
    let p = Pair { first: 3, second: 7 };
    p.get_first()
}
