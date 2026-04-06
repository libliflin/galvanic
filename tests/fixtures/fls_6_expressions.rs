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

    // FLS §6.5.8 — Lazy boolean expressions
    let _t = true || false;
    let _u = true && false;

    // FLS §6.5.9 — Type cast expressions
    // No explicit code example given in §6.5.9; derived from the semantic
    // description: "A type cast expression converts a value of one type to
    // a value of another type." (FLS §6.5.9)
    let _w: i32 = 5 as i32;       // i32 → i32 identity cast
    let _x: i32 = true as i32;    // bool → i32: true = 1
    let _y: i32 = false as i32;   // bool → i32: false = 0

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

// FLS §4.3 — Boolean type used as parameter and return type.
// No direct FLS example provided in §4.3; derived from the semantic description:
// "The boolean type bool has two values: true and false." (FLS §4.3)
// FLS §6.17: The if expression dispatches on the bool parameter at runtime.
fn bool_param_example(b: bool) -> i32 {
    if b { 1 } else { 0 }
}

fn bool_return_example(x: i32) -> bool {
    x > 0
}

// FLS §6.5.4 — Logical NOT for boolean values.
// FLS §6.5.4: "The type of a negation expression is the type of the operand."
// For bool, `!` is logical NOT (0 → 1, 1 → 0).
// No direct FLS example provided; derived from the semantic description:
// "The negation operator `!` applied to type bool is not supported in the
// same way as integers." (FLS §6.5.4 implies bool and integer NOT are distinct.)
fn bool_not_example(b: bool) -> bool {
    !b
}

// FLS §6.5.11 — Compound assignment expressions
// No direct FLS example provided; derived from the semantic description:
// "A compound assignment expression combines a binary operator expression
//  with an assignment expression." (FLS §6.5.11)
fn compound_assign_example() -> i32 {
    let mut x = 5;
    x += 3;    // x = 8  (FLS §6.5.11: +=)
    x -= 1;    // x = 7  (FLS §6.5.11: -=)
    x *= 2;    // x = 14 (FLS §6.5.11: *=)
    x /= 2;    // x = 7  (FLS §6.5.11: /=)
    x %= 3;    // x = 1  (FLS §6.5.11: %=)
    x &= 3;    // x = 1  (FLS §6.5.11: &=)
    x |= 4;    // x = 5  (FLS §6.5.11: |=)
    x ^= 2;    // x = 7  (FLS §6.5.11: ^=)
    x <<= 1;   // x = 14 (FLS §6.5.11: <<=)
    x >>= 1;   // x = 7  (FLS §6.5.11: >>=)
    x
}
