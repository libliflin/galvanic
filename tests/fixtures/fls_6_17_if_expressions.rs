// FLS §6.17 — If expressions and if-let expressions from the Ferrocene
// Language Specification.
// Source: https://rust-lang.github.io/fls/if-expressions.html
//
// Covers:
//   §6.17 — If expressions (if condition { } else { })
//   §6.17 — If-let expressions (if let pattern = expr { } else { })
//   §6.17 — Else-if chains (if ... else if ... else { })
//   §6.17 — If expression as a value (block result)
//
// All examples are self-contained (no std imports). Parameters are used
// to verify runtime codegen, not constant folding.

// FLS §6.17: If expression with a boolean condition. The condition is
// evaluated at runtime; the then-block or else-block executes accordingly.
fn if_else_basic(x: i32) -> i32 {
    // FLS §6.17: if BooleanExpression BlockExpression else BlockExpression
    if x > 0 {
        1
    } else {
        0
    }
}

// FLS §6.17: If expression used as a value — the result of the selected
// branch becomes the value of the if expression.
fn if_as_value(flag: i32) -> i32 {
    // FLS §6.17: Both branches must have compatible types when the if
    // expression is used in a value position.
    let result = if flag != 0 { 42 } else { 0 };
    result
}

// FLS §6.17: Else-if chain. Conditions are tested in order; the first
// matching branch executes.
fn classify(x: i32) -> i32 {
    // FLS §6.17: An else block may itself be an if expression, forming
    // an if-else-if chain.
    if x < 0 {
        0
    } else if x == 0 {
        1
    } else if x < 10 {
        2
    } else {
        3
    }
}

// FLS §6.17: If-let expression. The pattern is matched against the
// scrutinee; if it matches, the pattern bindings are in scope in the
// then-block.
fn if_let_some(opt: i32) -> i32 {
    // FLS §6.17: if let Pattern = Scrutinee BlockExpression
    // Here we use a literal-pattern match on an i32 parameter.
    if let 0 = opt {
        99
    } else {
        opt
    }
}

// FLS §6.17: If-let with an identifier pattern — binds the matched value.
fn if_let_bind(x: i32) -> i32 {
    // FLS §6.17: The identifier pattern binds x into `v` in the then-block.
    if let v = x {
        v + 1
    } else {
        0
    }
}

// FLS §6.17: If expression without an else block. When the condition is
// false the expression evaluates to () (unit). Return type must be unit.
fn if_no_else(x: i32) {
    // FLS §6.17: If there is no else block and the then-block does not
    // diverge, the if expression has type ().
    if x > 0 {
        let _ = x + 1;
    }
}

// FLS §6.17: Nested if expressions. Inner if is the scrutinee of the outer.
fn nested_if(a: i32, b: i32) -> i32 {
    if a > 0 {
        if b > 0 {
            a + b
        } else {
            a
        }
    } else {
        0
    }
}

// FLS §6.17: If-let with a range pattern.
fn if_let_range(x: i32) -> i32 {
    if let 1..=5 = x {
        1
    } else {
        0
    }
}

fn main() {
    let _ = if_else_basic(1);
    let _ = if_else_basic(-1);
    let _ = if_as_value(1);
    let _ = if_as_value(0);
    let _ = classify(-3);
    let _ = classify(0);
    let _ = classify(5);
    let _ = classify(100);
    let _ = if_let_some(0);
    let _ = if_let_some(7);
    let _ = if_let_bind(3);
    if_no_else(1);
    if_no_else(0);
    let _ = nested_if(2, 3);
    let _ = nested_if(2, -1);
    let _ = nested_if(-1, 1);
    let _ = if_let_range(3);
    let _ = if_let_range(9);
}
