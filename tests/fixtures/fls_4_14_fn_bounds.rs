// FLS §4.14 — Parenthesized trait bounds (`Fn(T) -> R` form)
//
// FLS §4.14 introduces parenthesized syntax for the `Fn`/`FnMut`/`FnOnce`
// trait family. A generic parameter `F: Fn(i32) -> i32` is syntactic sugar
// for `F: Fn<(i32,), Output = i32>`. The same form is valid in a `where`
// clause: `where F: Fn(i32) -> i32`.
//
// AMBIGUOUS: §4.14 — The FLS does not specify whether parenthesized syntax
// is restricted to the three Fn traits or is syntactically valid for any
// trait name. Galvanic accepts it for any trait name at the parse level and
// defers semantic restriction to a future type-checking phase. See
// refs/fls-ambiguities.md §4.14.

// Inline bound form: `F: Fn(i32) -> i32`.
fn apply_inline<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 {
    f(x)
}

// Where-clause bound form: `where F: Fn(i32) -> i32`.
fn apply_where<F>(f: F, x: i32) -> i32
where
    F: Fn(i32) -> i32,
{
    f(x)
}

fn main() -> i32 {
    let double = |x| x * 2;
    apply_inline(double, 5)
}
