// FLS §6.5 — Operator Expressions from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/operator-expressions.html
//
// Covers:
//   §6.5.1  — Borrow expressions (&expr, &mut expr)
//   §6.5.2  — Dereference expressions (*expr)
//   §6.5.4  — Negation expressions (-expr, !expr)
//   §6.5.5  — Arithmetic expressions (+, -, *, /, %)
//   §6.5.6  — Bit expressions (&, |, ^)
//   §6.5.7  — Shift expressions (<<, >>)
//   §6.5.8  — Comparison expressions (==, !=, <, >, <=, >=)
//   §6.5.9  — Lazy boolean expressions (&&, ||)
//   §6.5.10 — Type cast expressions (expr as Type)
//   §6.5.11 — Assignment expressions (place = value)
//   §6.5.12 — Compound assignment expressions (+=, -=, *=, /=, %=, &=, |=, ^=, <<=, >>=)
//
// §6.5.3 (error propagation / ? operator) requires Result and is not covered here.
//
// All examples are self-contained (no std imports). Parameters are used
// to verify runtime codegen, not constant folding.

// FLS §6.5.1: Borrow expression — creates a reference to a place expression.
fn borrow_immutable(x: i32) -> i32 {
    // FLS §6.5.1: &OperandExpression — borrows the operand immutably.
    let r: &i32 = &x;
    *r + 1
}

// FLS §6.5.1: Mutable borrow — creates a mutable reference.
fn borrow_mutable(mut x: i32) -> i32 {
    // FLS §6.5.1: &mut OperandExpression — borrows the operand mutably.
    let r: &mut i32 = &mut x;
    *r += 10;
    x
}

// FLS §6.5.2: Dereference expression — accesses the value at the memory location.
fn deref_ref(r: &i32) -> i32 {
    // FLS §6.5.2: *OperandExpression — dereferences r to obtain the i32 value.
    *r + 1
}

// FLS §6.5.2: Dereference through mutable reference.
fn deref_mut_ref(r: &mut i32) -> i32 {
    // FLS §6.5.2: Dereference in a place context for assignment.
    *r = *r * 2;
    *r
}

// FLS §6.5.4: Arithmetic negation — negates a numeric value.
fn negate_i32(x: i32) -> i32 {
    // FLS §6.5.4: -OperandExpression (NegationExpression, ArithmeticNegation)
    -x
}

// FLS §6.5.4: Logical negation — inverts a boolean value.
fn negate_bool(b: bool) -> bool {
    // FLS §6.5.4: !OperandExpression (NegationExpression, LogicalNegation)
    !b
}

// FLS §6.5.5: Arithmetic expressions — addition, subtraction, multiplication,
// division, remainder.
fn arithmetic(a: i32, b: i32) -> i32 {
    // FLS §6.5.5: AdditionExpression
    let sum = a + b;
    // FLS §6.5.5: SubtractionExpression
    let diff = a - b;
    // FLS §6.5.5: MultiplicationExpression
    let prod = a * b;
    // FLS §6.5.5: DivisionExpression
    let quot = a / b;
    // FLS §6.5.5: RemainderExpression
    let rem = a % b;
    sum + diff + prod + quot + rem
}

// FLS §6.5.6: Bit expressions — bitwise AND, OR, XOR.
fn bitwise(a: i32, b: i32) -> i32 {
    // FLS §6.5.6: BitwiseAndExpression
    let and = a & b;
    // FLS §6.5.6: BitwiseOrExpression
    let or = a | b;
    // FLS §6.5.6: BitwiseXorExpression
    let xor = a ^ b;
    and + or + xor
}

// FLS §6.5.7: Shift expressions — left shift and right shift.
fn shifts(x: i32, n: i32) -> i32 {
    // FLS §6.5.7: ShiftLeftExpression
    let left = x << n;
    // FLS §6.5.7: ShiftRightExpression (arithmetic right shift for i32)
    let right = x >> n;
    left + right
}

// FLS §6.5.8: Comparison expressions.
fn comparisons(a: i32, b: i32) -> i32 {
    // FLS §6.5.8: EqualityExpression
    let eq = if a == b { 1 } else { 0 };
    // FLS §6.5.8: InequalityExpression
    let ne = if a != b { 1 } else { 0 };
    // FLS §6.5.8: LessThanExpression
    let lt = if a < b { 1 } else { 0 };
    // FLS §6.5.8: GreaterThanExpression
    let gt = if a > b { 1 } else { 0 };
    // FLS §6.5.8: LessThanOrEqualExpression
    let le = if a <= b { 1 } else { 0 };
    // FLS §6.5.8: GreaterThanOrEqualExpression
    let ge = if a >= b { 1 } else { 0 };
    eq + ne + lt + gt + le + ge
}

// FLS §6.5.9: Lazy boolean expressions — short-circuit evaluation.
fn lazy_boolean(a: bool, b: bool) -> i32 {
    // FLS §6.5.9: LazyAndExpression — evaluates b only if a is true.
    let and_result = if a && b { 1 } else { 0 };
    // FLS §6.5.9: LazyOrExpression — evaluates b only if a is false.
    let or_result = if a || b { 1 } else { 0 };
    and_result + or_result
}

// FLS §6.5.10: Type cast expression — converts a value to a target type.
fn type_cast(x: i32) -> i32 {
    // FLS §6.5.10: TypeCastExpression — expr as Type
    let as_u32 = x as u32;
    // FLS §6.5.10: Cast back to i32 for return.
    as_u32 as i32
}

// FLS §6.5.10: Cast from bool to integer.
fn bool_as_int(b: bool) -> i32 {
    // FLS §6.5.10: true casts to 1, false casts to 0.
    b as i32
}

// FLS §6.5.11: Assignment expression — assigns the right operand to the place.
fn assignment(mut x: i32) -> i32 {
    // FLS §6.5.11: AssignmentExpression — place = value
    x = x + 1;
    x
}

// FLS §6.5.12: Compound assignment expressions — all ten operators.
fn compound_assignment(mut x: i32) -> i32 {
    // FLS §6.5.12: AddAssignExpression
    x += 1;
    // FLS §6.5.12: SubtractAssignExpression
    x -= 1;
    // FLS §6.5.12: MultiplyAssignExpression
    x *= 2;
    // FLS §6.5.12: DivideAssignExpression
    x /= 2;
    // FLS §6.5.12: RemainderAssignExpression
    x %= 3;
    // FLS §6.5.12: BitAndAssignExpression
    x &= 0xFF;
    // FLS §6.5.12: BitOrAssignExpression
    x |= 1;
    // FLS §6.5.12: BitXorAssignExpression
    x ^= 1;
    // FLS §6.5.12: ShiftLeftAssignExpression
    x <<= 1;
    // FLS §6.5.12: ShiftRightAssignExpression
    x >>= 1;
    x
}

fn main() {
    let _ = borrow_immutable(5);
    let _ = borrow_mutable(5);
    let mut v = 4;
    let _ = deref_ref(&v);
    let _ = deref_mut_ref(&mut v);
    let _ = negate_i32(3);
    let _ = negate_i32(-3);
    let _ = negate_bool(true);
    let _ = negate_bool(false);
    let _ = arithmetic(10, 3);
    let _ = bitwise(0b1010, 0b1100);
    let _ = shifts(4, 1);
    let _ = comparisons(5, 5);
    let _ = comparisons(3, 7);
    let _ = lazy_boolean(true, false);
    let _ = lazy_boolean(false, true);
    let _ = type_cast(42);
    let _ = bool_as_int(true);
    let _ = bool_as_int(false);
    let _ = assignment(10);
    let _ = compound_assignment(10);
}
