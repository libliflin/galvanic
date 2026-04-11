// FLS §6.4.3 — Named Block Expressions
//
// A named block expression is a block expression preceded by a lifetime label.
// The label makes the block a break target: a `break 'label` or
// `break 'label expr` expression inside the block exits it, optionally
// producing a value.
//
// FLS §6.4.3 grammar:
//
//   NamedBlockExpression → LIFETIME_OR_LABEL : BlockExpression
//
// Break expressions that exit named blocks are defined in FLS §6.15.6.
//
// Examples in this fixture are derived from the FLS §6.4.3 structure and
// the general block/break semantics described in §6.4 and §6.15.6.
//
// FLS §6.4.3: The spec does not provide a verbatim code example for named
// blocks in isolation; these examples are constructed from the grammar and
// the semantics of §6.15.6 (break expressions).

// Named block as a let initializer — tail expression is the block value.
fn named_block_tail() -> i32 {
    // FLS §6.4.3: named block whose value is its tail expression (no break).
    let a = 'blk: {
        3 + 4
    };
    a
}

// Named block with an explicit `break 'label expr`.
fn named_block_break_with_value(x: i32) -> i32 {
    // FLS §6.15.6: break expression with a label and a value expression.
    let result = 'compute: {
        if x > 0 {
            break 'compute x * 2;
        }
        0
    };
    result
}

// Named block with a break that carries a non-trivial arithmetic expression.
fn named_block_break_arithmetic(a: i32, b: i32) -> i32 {
    // FLS §6.4.3 / §6.15.6: the break value may be any expression.
    'sum: {
        if a < 0 {
            break 'sum 0;
        }
        a + b
    }
}

// Named block nested inside a for loop — break exits the block, not the loop.
fn named_block_in_loop(limit: i32) -> i32 {
    // FLS §6.4.3: a named block may appear inside other control-flow forms.
    // FLS §6.15.6: break 'label exits the named block with the given value,
    // not the enclosing loop.
    let mut found = -1;
    let mut i = 0;
    while i < limit {
        let hit = 'probe: {
            if i == 3 {
                break 'probe i;
            }
            -1
        };
        if hit >= 0 {
            found = hit;
        }
        i += 1;
    }
    found
}

// Nested named blocks — each label refers to its own enclosing block.
fn nested_named_blocks(flag: bool) -> i32 {
    // FLS §6.4.3: labels are lexically scoped; an inner break exits the
    // innermost block whose label matches.
    'outer: {
        let inner = 'inner: {
            if flag {
                break 'inner 1;
            }
            break 'outer 99;
        };
        inner + 10
    }
}

// Named block used directly as a function argument (not in a let binding).
fn use_value(v: i32) -> i32 {
    v + 1
}

fn named_block_as_argument(n: i32) -> i32 {
    // FLS §6.4.3: a named block expression may appear wherever an expression
    // is expected, including as a function argument.
    use_value('arg: {
        if n > 5 {
            break 'arg n;
        }
        0
    })
}

// Named block with no explicit break — value is the tail expression.
// This is the degenerate case: the label exists but is never targeted.
fn named_block_no_break_needed() -> i32 {
    // FLS §6.4.3: a named block that is never broken out of behaves like a
    // plain block expression. The label is syntactically required but not
    // semantically exercised.
    'idle: {
        let x = 7;
        x * 6
    }
}

fn main() -> i32 {
    let a = named_block_tail();
    let b = named_block_break_with_value(3);
    let c = named_block_break_arithmetic(2, 5);
    let d = named_block_in_loop(10);
    let e = nested_named_blocks(true);
    let f = named_block_as_argument(6);
    let g = named_block_no_break_needed();
    a + b + c + d + e + f + g
}
