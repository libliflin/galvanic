// FLS §12.1 — Generic enum definitions.
//
// An enum may declare type parameters. Each variant may reference those
// parameters in its fields. Galvanic monomorphizes all type parameters to
// i32 at this milestone.
//
// Source: FLS §12.1 (Generic Parameters), applied to enum definitions.

enum Wrapper<T> {
    Value(T),
    Nothing,
}

enum Either<A, B> {
    Left(A),
    Right(B),
}

fn main() -> i32 {
    let w = Wrapper::Value(7_i32);
    let result = match w {
        Wrapper::Value(x) => x,
        Wrapper::Nothing => 0,
    };
    result
}
