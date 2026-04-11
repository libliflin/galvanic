// FLS §6.19 — Return Expressions
// Source: https://rust-lang.github.io/fls/expressions/return-expr.html
//
// Each function uses a parameter so galvanic must emit runtime instructions
// rather than constant-fold at compile time (fls-constraints.md §Constraint 1).
//
// FLS §6.19: "A return expression is an expression that optionally evaluates
// and returns the value of an operand back to the caller of the innermost
// enclosing function."
//
// FLS §6.19: "If the optional operand is absent, the return type of the
// innermost enclosing function shall be the unit type."

// FLS §6.19: Explicit return with a value — transfers control to caller.
// The tail expression path is NOT taken; return exits before it.
fn early_return_taken(x: i32) -> i32 {
    if x > 0 {
        return 1;  // FLS §6.19: return expression with operand value 1
    }
    0  // tail expression — only reached when x <= 0
}

// FLS §6.19: Explicit return in the false branch — return not taken when false.
fn early_return_not_taken(x: i32) -> i32 {
    if x < 0 {
        return -1;  // FLS §6.19: return expression; only fires when x < 0
    }
    x  // FLS §6.19: tail expression (implicit return) for x >= 0
}

// FLS §6.19: Return expression without an operand — returns unit type.
// FLS §6.19: "If the optional operand is absent, the return type … shall be the unit type."
fn return_unit(x: i32) {
    if x == 0 {
        return;  // FLS §6.19: return expression with no operand; return type is ()
    }
    let _ = x + 1;  // side-effect to give the function a non-trivial body
}

// FLS §6.19: Tail expression as implicit return.
// FLS §6.19: "The innermost enclosing function body … its tail expression is
// evaluated and the value is implicitly returned."
fn tail_expression_return(x: i32) -> i32 {
    let doubled = x * 2;
    doubled  // FLS §6.19: tail expression — value implicitly returned to caller
}

// FLS §6.19: Return inside a loop — exits the enclosing function, not just the loop.
// FLS §6.19: "innermost enclosing function" — return crosses loop boundaries.
fn return_from_loop(x: i32) -> i32 {
    let mut i = 0;
    loop {
        if i >= x {
            return i;  // FLS §6.19: return exits the function from inside a loop
        }
        i = i + 1;
    }
}

// FLS §6.19: Multiple return paths — first taken return wins.
// Demonstrates that only one return executes per call.
fn classify(x: i32) -> i32 {
    if x < 0 {
        return -1;  // FLS §6.19: return expression for negative input
    }
    if x == 0 {
        return 0;   // FLS §6.19: return expression for zero input
    }
    1  // FLS §6.19: tail expression (implicit return) for positive input
}

// FLS §6.19: Return expression in a nested block — still exits the enclosing function.
// FLS §6.19: "innermost enclosing function" means blocks do not intercept return.
fn return_from_nested_block(x: i32) -> i32 {
    let _ = {
        if x > 10 {
            return x;  // FLS §6.19: return exits the function, not just the block
        }
        x + 1
    };
    0  // only reached when x <= 10
}

// FLS §6.19: Explicit return as the only return path (no tail expression).
fn explicit_return_only(x: i32) -> i32 {
    return x * 3;  // FLS §6.19: explicit return; no tail expression
}
