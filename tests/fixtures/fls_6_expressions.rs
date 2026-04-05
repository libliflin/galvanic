// FLS §6 — Expression examples from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/expressions.html
//
// Adapted to the subset galvanic currently handles. Each expression form
// is exercised inside a function body.

fn main() {
    // FLS §6.2 — Literal expressions
    let _a = 5;
    let _b = true;

    // FLS §6.5.5 — Negation expressions
    let _c = -42;
    let _d = !42;
    let _e = !false;

    // FLS §6.5.6 — Arithmetic expressions
    let _f = 1 + 2;
    let _g = 10 % 4;
    let _h = 3 - 2;

    // FLS §6.5.7 — Bit expressions
    let _i = 0b1010 & 0b1100;
    let _j = 0b1010 | 0b0011;
    let _k = 0b1010 ^ 0b1001;
    let _l = 13 << 3;
    let _m = -10 >> 2;

    // FLS §6.5.8 — Comparison expressions
    let _n = 12 == 12;
    let _o = 42 > 12;
    let _p = 42 >= 35;
    let _q = 42 < 109;
    let _r = 42 <= 42;
    let _s = 12 != 42;

    // FLS §6.5.9 — Lazy boolean expressions
    let _t = true || false;
    let _u = true && false;

    // FLS §6.4 — Block expressions
    let _v = {
        42
    };
}

// FLS §6.17.1 — If expressions
fn classify_age(age: i32) -> i32 {
    if age <= 14 {
        0
    } else if age <= 24 {
        1
    } else if age <= 64 {
        2
    } else {
        3
    }
}

// FLS §6.15.2 / §6.15.3 — Loop and while expressions
// NOTE: FLS example uses `let mut counter = 0;` but galvanic does not yet
// handle `mut` in let bindings. Adapted to use reassignment.
fn count_to_ten() -> i32 {
    let counter = 0;
    while counter < 10 {
        counter = counter + 1;
    }
    counter
}

// FLS §6.12.1 — Call expressions
fn use_call() -> i32 {
    let three: i32 = add_two(1, 2);
    three
}

fn add_two(a: i32, b: i32) -> i32 {
    a + b
}
