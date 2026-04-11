// FLS §6.15 — Loop expressions from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/loop-expressions.html
//
// Covers:
//   §6.15.2 — Infinite loop expressions (loop { })
//   §6.15.3 — While loop expressions (while condition { })
//   §6.15.1 — For loop expressions (for pattern in iterator { })
//   §6.15.4 — While-let loop expressions (while let pattern = expr { })
//   §6.15.6 — Break expressions (break, break value)
//   §6.15.7 — Continue expressions (continue)
//   §6.4.3 — Labeled block expressions with loop labels ('label: loop)
//
// All examples are self-contained (no std imports). Parameters are used
// in loop bounds to verify runtime codegen, not constant folding.

// FLS §6.15.3: While loop with parameter bound — loop condition must be
// evaluated at runtime each iteration.
fn while_loop(n: i32) -> i32 {
    let mut i = 0;
    let mut acc = 0;
    // FLS §6.15.3: while expression evaluates condition each iteration.
    while i < n {
        acc = acc + i;
        i = i + 1;
    }
    acc
}

// FLS §6.15.2: Infinite loop with break expression returning a value.
// FLS §6.15.6: "A break expression with a value transfers the value out
//   of the loop expression."
fn loop_with_break_value(limit: i32) -> i32 {
    let mut count = 0;
    // FLS §6.15.2: An infinite loop expression evaluates the loop body
    // repeatedly until a break expression is encountered.
    let result = loop {
        if count >= limit {
            // FLS §6.15.6: break with an expression — the loop expression
            // evaluates to that value.
            break count * 2;
        }
        count = count + 1;
    };
    result
}

// FLS §6.15.7: Continue expression — skips remaining body, re-evaluates
// loop condition.
fn while_with_continue(n: i32) -> i32 {
    let mut i = 0;
    let mut acc = 0;
    while i < n {
        i = i + 1;
        if i == 3 {
            // FLS §6.15.7: continue skips the rest of the loop body.
            continue;
        }
        acc = acc + i;
    }
    acc
}

// FLS §6.15.1: For loop expression iterating over a range.
// The range expression is a §6.16 range; the for loop desugars to an
// iterator protocol (FLS §6.15.1).
fn for_range_sum(n: i32) -> i32 {
    let mut acc = 0;
    // FLS §6.15.1: for pattern in iterator_expression — binds each
    // element to the pattern and evaluates the loop body.
    for x in 0..n {
        acc = acc + x;
    }
    acc
}

// FLS §6.15.1: For loop with inclusive range (§6.16 RangeInclusive).
fn for_inclusive_range(n: i32) -> i32 {
    let mut acc = 0;
    for x in 0..=n {
        acc = acc + x;
    }
    acc
}

// FLS §6.15.4: While-let loop expression. Loops while the pattern matches.
// Pattern is an identifier pattern binding the matched value.
fn while_let_countdown(start: i32) -> i32 {
    let mut maybe = start;
    let mut steps = 0;
    // FLS §6.15.4: while let pattern = expression — re-evaluates the
    // expression each iteration, loops while the pattern matches.
    while let x = maybe {
        if x <= 0 {
            break;
        }
        steps = steps + 1;
        maybe = maybe - 1;
    }
    steps
}

// FLS §6.15.2 + §6.15.6: Labeled loop with labeled break.
// FLS §6.4.3: A label on a loop expression allows a break/continue
// in a nested loop to target the outer loop.
fn labeled_break_outer(n: i32) -> i32 {
    let mut found = 0;
    // FLS §6.4.3: Label syntax 'label: on a loop expression.
    'outer: for i in 0..n {
        for j in 0..n {
            if i + j == n - 1 {
                found = i * 10 + j;
                // FLS §6.15.6: break 'label exits the loop named by the label.
                break 'outer;
            }
        }
    }
    found
}

// FLS §6.15.7: Continue with a label — skips rest of outer loop body.
fn labeled_continue(n: i32) -> i32 {
    let mut acc = 0;
    'outer: for i in 0..n {
        for j in 0..n {
            if j == 1 {
                // FLS §6.15.7: continue 'label — continues the labeled loop.
                continue 'outer;
            }
            acc = acc + i;
        }
    }
    acc
}

fn main() {
    let _ = while_loop(5);
    let _ = loop_with_break_value(4);
    let _ = while_with_continue(5);
    let _ = for_range_sum(4);
    let _ = for_inclusive_range(3);
    let _ = while_let_countdown(3);
    let _ = labeled_break_outer(5);
    let _ = labeled_continue(4);
}
