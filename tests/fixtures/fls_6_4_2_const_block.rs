// FLS §6.4.2: Const Block Expressions
//
// A const block expression is a block expression preceded by the keyword
// `const`. The block body is evaluated in a const context at compile time.
//
// Fixture derived from FLS §6.4.2. The spec provides the following grammar:
//
//   ConstBlockExpression → `const` BlockExpression
//
// The block body must be fully const-evaluable. Named const items in scope
// are visible inside the const block (FLS §7.1).

const BASE: i32 = 10;

fn demonstrate_const_block() -> i32 {
    // Simple arithmetic const block.
    let a = const { 2 + 3 };

    // Const block with let bindings.
    let b = const {
        let x = 6;
        let y = 7;
        x * y
    };

    // Const block referencing a named const item.
    let c = const { BASE * 4 };

    // Const block used directly as an expression.
    a + b + c
}

fn main() -> i32 {
    demonstrate_const_block()
}
