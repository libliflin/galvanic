// FLS §11: impl Trait in argument position.
//
// `impl Trait` in argument position is syntactic sugar for an anonymous generic
// type parameter with a trait bound. The FLS calls this "argument-position impl Trait".
//
// FLS §11: AMBIGUOUS — The FLS does not precisely specify lifetime capture rules
// for argument-position impl Trait, or how it interacts with higher-ranked trait
// bounds. Galvanic treats each impl Trait parameter as an independent implicit
// anonymous generic type parameter, monomorphized at each call site.
trait Value {
    fn get(&self) -> i32;
}

struct Num { val: i32 }

impl Value for Num {
    fn get(&self) -> i32 {
        self.val
    }
}

fn extract(x: impl Value) -> i32 {
    x.get()
}

fn main() -> i32 {
    let n = Num { val: 42 };
    extract(n)
}
