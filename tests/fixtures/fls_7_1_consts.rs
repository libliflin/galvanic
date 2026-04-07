// FLS §7.1: Constant Items
//
// "A constant is a named value with a constant initializer."
// "Every use of a constant is replaced with its value (or a copy of it)."
//
// This fixture derives examples from FLS §7.1. The spec does not provide
// runnable code examples directly; the programs below are derived from
// the semantic description.
//
// FLS §7.1:10 — substitution semantics.
// FLS §6.1.2:37–45 — const initializers are evaluated at compile time;
// the uses are substituted as immediates, not loaded from stack slots.

// FLS §7.1: A constant with an integer literal initializer.
// The name MAX_LEN is replaced by 10 at every use site.
const MAX_LEN: i32 = 10;

// FLS §7.1: A zero constant.
const ZERO: i32 = 0;

// FLS §7.1: Multiple constants in one file.
const A: i32 = 3;
const B: i32 = 4;

// FLS §7.1 + FLS §6.5.5: Constant with arithmetic initializer.
// The spec requires constant initializers to be constant expressions
// (FLS §6.1.2:37–45). Arithmetic operators produce constant expressions
// when all operands are constants.
const BUFFER_SIZE: i32 = 64 * 1024;

// FLS §7.1 + FLS §7.1:10: Constant whose initializer references another const.
// B is already known (= 4), so C = 4 + 1 = 5 at compile time.
const C: i32 = B + 1;

// A function that uses constants as values — each use is substituted.
// FLS §7.1:10: not loaded from memory; inlined as an immediate.
fn use_consts() -> i32 {
    // MAX_LEN is substituted with 10, ZERO with 0.
    // Result: 10 + 0 = 10. But we return MAX_LEN directly.
    MAX_LEN
}

// A function that uses a constant in arithmetic.
// FLS §6.5.5: runtime addition; the constant is materialized as LoadImm.
fn pythag_sum() -> i32 {
    A + B
}

// A function that uses a constant as a loop bound.
// FLS §6.15.3: While loop — condition checked at runtime each iteration.
fn count_to_max() -> i32 {
    let mut i = ZERO;
    while i < MAX_LEN {
        i += 1;
    }
    i
}

fn main() -> i32 {
    // use_consts() returns 10; pythag_sum() returns 7; count_to_max() returns 10.
    // BUFFER_SIZE = 65536; C = 5.
    // Return 10 + 7 - 10 + 65536 - 65536 + 5 - 5 = 7. Exit code 7.
    use_consts() + pythag_sum() - count_to_max() + BUFFER_SIZE - BUFFER_SIZE + C - C
}
