// FLS §5.1.4 — @ binding patterns (identifier patterns with sub-patterns).
// Each function exercises a different sub-pattern kind in match and if-let.
// Derived from FLS §5.1.4 examples.

fn at_wildcard_match(x: i32) -> i32 {
    match x {
        n @ _ => n + 1,
    }
}

fn at_litint_match(x: i32) -> i32 {
    match x {
        n @ 5 => n * 2,
        _ => 0,
    }
}

fn at_neg_litint_match(x: i32) -> i32 {
    match x {
        n @ -3 => n + 10,
        _ => 0,
    }
}

fn at_litbool_match(b: bool) -> i32 {
    match b {
        n @ true => {
            let _ = n;
            1
        }
        _ => 0,
    }
}

fn at_wildcard_if_let(x: i32) -> i32 {
    if let n @ _ = x { n * 3 } else { 0 }
}

fn at_litint_if_let(x: i32) -> i32 {
    if let n @ 7 = x { n + 1 } else { 0 }
}

fn at_neg_litint_if_let(x: i32) -> i32 {
    if let n @ -2 = x { n + 5 } else { 0 }
}

fn at_litbool_if_let(b: bool) -> i32 {
    if let n @ true = b {
        let _ = n;
        1
    } else {
        0
    }
}

fn main() {}
