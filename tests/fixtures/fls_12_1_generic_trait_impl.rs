// FLS §12.1: Generic trait implementations.
//
// A generic impl block can implement a trait for a generic type:
//   `impl<T> SomeTrait for SomeType<T> { ... }`
//
// This fixture is derived from the FLS §12.1 generic parameter examples
// combined with FLS §11.1 trait implementation syntax.
//
// FLS §12.1 AMBIGUOUS: The spec does not describe how `<T>` after `impl`
// interacts with trait impl syntax. Galvanic treats `impl<T> Trait for Type<T>`
// as a generic trait impl block with `T` substituted to `i32` at all call sites.

trait Getter {
    fn get(&self) -> i32;
}

struct Wrapper<T> {
    inner: T,
}

impl<T> Getter for Wrapper<T> {
    fn get(&self) -> i32 {
        self.inner
    }
}

fn use_it(w: Wrapper<i32>) -> i32 {
    w.get()
}
