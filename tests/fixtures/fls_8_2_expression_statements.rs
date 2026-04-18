// FLS §8.2 — Expression statements from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/statements.html#expression-statements
//
// An expression statement evaluates its operand for side effects and discards
// the result. The semicolon is the marker: `expr;` is a statement, while `expr`
// at block tail position is a value expression.
//
// FLS §8.2: "An expression statement is a statement that consists of an
// expression followed by a semicolon."

fn main() -> i32 {
    0
}

// FLS §8.2: Integer literal as an expression statement. The value 42 is
// evaluated at runtime (emits a LoadImm instruction) and discarded.
// The function returns unit implicitly.
fn discard_integer_literal() {
    42;
}

// FLS §8.2: Binary expression as a statement. x + 1 is evaluated at runtime
// (emits Load + LoadImm + BinOp) and the result is discarded. The function
// returns unit implicitly.
fn discard_binary_expr(x: i32) {
    x + 1;
}

// FLS §8.2: Multiple expression statements in sequence — each evaluated in
// declaration order (FLS §6:3), each result discarded.
fn discard_multiple(a: i32, b: i32) {
    a + b;
    a * b;
}

// FLS §8.2: A function call as an expression statement — the return value is
// discarded. This already worked (call handlers ignore ret_ty); included for
// regression coverage.
fn helper(x: i32) -> i32 {
    x + 1
}

fn discard_call_result(x: i32) {
    helper(x);
}
