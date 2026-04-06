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

    // FLS §6.5.6 — Arithmetic expressions (FLS §6.5.5)
    let _f = 1 + 2;
    let _g = 10 % 4;   // remainder: 10 % 4 = 2
    let _ga = 10 / 2;  // division: 10 / 2 = 5
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

// FLS §6.15.3 — While loop expressions (milestone 7: compile-time simulation)
fn count_to_ten() -> i32 {
    let mut counter = 0;
    while counter < 10 {
        counter = counter + 1;
    }
    counter
}

// FLS §6.15.2 — Loop expression (milestone 8: compile-time simulation)
// FLS §6.15.6 — Break expression
fn find_first_over_threshold() -> i32 {
    let mut n = 1;
    loop {
        if n > 100 { break; }
        n = n * 2;
    }
    n
}

// FLS §6.15.2 — Loop as value expression (break with value)
fn loop_returning_value() -> i32 {
    let mut i = 0;
    let result = loop {
        i = i + 1;
        if i >= 7 { break i; }
    };
    result
}

// FLS §6.15.7 — Continue expression (milestone 9: compile-time simulation)
// Sums values 1..=5, skipping 3 via `continue`; returns 12.
// FLS §6.15.7: "A continue expression terminates the current iteration of the
// innermost enclosing loop expression." No FLS example provided; this program
// is derived from the semantic description in §6.15.7.
fn sum_skipping_three() -> i32 {
    let mut i = 0;
    let mut sum = 0;
    while i < 5 {
        i = i + 1;
        if i == 3 { continue; }
        sum = sum + i;
    }
    sum
}

// FLS §6.12.1 — Call expressions
fn use_call() -> i32 {
    let three: i32 = add_two(1, 2);
    three
}

fn add_two(a: i32, b: i32) -> i32 {
    a + b
}
