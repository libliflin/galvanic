// FLS §4.14: Trait and lifetime bounds — lifetime predicates in where clauses.
//
// Where clauses may contain lifetime bounds of three forms:
//
//   `where T: 'static`     — type parameter must outlive 'static
//   `where T: 'static + Copy` — type parameter must outlive 'static and impl Copy
//   `where 'static: 'static` — lifetime outlives predicate (trivially true)
//
// Galvanic discards all where-clause bounds; they are parsed and consumed
// silently so that the caller can continue parsing the item body.
//
// FLS §4.14: AMBIGUOUS — The spec does not specify when lifetime bounds in
// where clauses are checked relative to monomorphization. Galvanic defers
// all lifetime checking; this is noted as a known gap.

// `where T: 'static` — lifetime bound on a type parameter
fn require_static<T>(x: i32) -> i32 where T: 'static {
    x
}

// `where T: 'static + Copy` — mixed lifetime and trait bound
fn require_static_copy<T>(x: i32) -> i32 where T: 'static + Copy {
    x
}

// Multiple predicates, one of which is a lifetime bound
fn multi_predicate<T>(x: i32) -> i32 where T: 'static, T: Copy {
    x
}

// `where 'static: 'static` — bare lifetime as LHS (outlives predicate)
fn lifetime_outlives(x: i32) -> i32 where 'static: 'static {
    x
}

fn main() -> i32 {
    require_static(1)
        + require_static_copy(2)
        + multi_predicate(3)
        + lifetime_outlives(4)
}
