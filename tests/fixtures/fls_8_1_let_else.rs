// FLS §8.1: let-else statements.
//
// A `let-else` binding uses a refutable pattern. If the pattern does not
// match, the else block executes; the else block must be a diverging
// expression (return, break, continue, or an infinite loop).
//
// Variables bound by the pattern are in scope after the let-else statement.
//
// Grammar: `"let" Pattern "=" Expression "else" Block ";"`
//
// This fixture is derived from FLS §8.1 examples.

enum Opt {
    Some(i32),
    None,
}

fn get_some(x: i32) -> Opt {
    Opt::Some(x)
}

fn get_none() -> Opt {
    Opt::None
}

fn extract(o: Opt) -> i32 {
    let Opt::Some(v) = o else { return 0 };
    v
}

fn main() -> i32 {
    let o = get_some(7);
    let Opt::Some(v) = o else { return 1 };
    v - 7
}
