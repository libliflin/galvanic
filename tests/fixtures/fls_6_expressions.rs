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
    // FLS §6.5.9: Numeric casts to other integer types (milestone 66).
    let _cast_u32: u32 = 10 as u32;      // i32 → u32 reinterpret
    let _cast_i64: i64 = 20 as i64;      // i32 → i64 widening
    let _cast_usize: usize = 5 as usize; // i32 → usize
    let _cast_back: i32 = _cast_u32 as i32; // u32 → i32 reinterpret

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

// FLS §6.15.1 — For loop expression with integer range.
// FLS §6.16 — Range expressions `start..end` and `start..=end`.
//
// No direct for-loop-with-range example in FLS §6.15.1; the section states
// "A for loop expression iterates over the values produced by an IntoIterator."
// This function is derived from the semantic description in §6.15.1 and §6.16.
fn for_loop_sum_example() -> i32 {
    let mut sum = 0;
    for i in 0..5 {     // FLS §6.16: exclusive range 0, 1, 2, 3, 4
        sum += i;        // FLS §6.5.11: compound add-assignment
    }
    sum                  // = 10
}

// FLS §6.16 — Inclusive range `start..=end`.
// No direct FLS §6.16 example for inclusive ranges; derived from the spec's
// definition: "A range expression `..=` represents an inclusive range."
fn for_loop_inclusive_example() -> i32 {
    let mut product = 1;
    for i in 1..=4 {    // FLS §6.16: inclusive range 1, 2, 3, 4
        product *= i;    // FLS §6.5.11: compound multiply-assignment
    }
    product              // = 24
}

// FLS §8.1 — Let statement with no initializer.
//
// FLS §8.1: "A LetStatement may optionally have an Initializer."
// When no initializer is present the variable is declared but not initialized.
// A subsequent assignment expression stores the first value to the slot.
//
// The FLS §8.1 grammar is:
//   LetStatement ::= `let` PatternWithoutAlternation (`:` TypeSpecification)?
//                    (`=` Expression (ExpressionWithoutBlock `else` BlockExpression)?)? `;`
//
// Note: FLS §8.1 does not provide a direct code example for the uninit form;
// this function is derived from the grammar's optional-initializer clause.
fn uninit_let_example() -> i32 {
    let x;               // FLS §8.1: declared, no initializer
    x = 7;               // FLS §6.5.10: assignment stores to slot
    let y;               // FLS §8.1: second uninit binding
    y = x + 1;           // FLS §6.5.5: arithmetic then assignment
    y                    // = 8
}

// FLS §8.1 — Conditional initialization pattern.
//
// A common Rust idiom: declare with `let`, assign in each branch of an
// if/else, then use after the control flow rejoins. The compiler allocates
// the slot at the `let` point; the branches store distinct values.
//
// FLS §8.1 NOTE: Full Rust requires definite initialization analysis
// (every path must assign before use). Galvanic does not yet enforce this.
fn conditional_init_example(flag: bool) -> i32 {
    let result;
    if flag {
        result = 1;      // FLS §8.1: first possible initializer
    } else {
        result = 0;      // FLS §8.1: second possible initializer
    }
    result               // FLS §6.3: path expression reads the assigned slot
}

// FLS §6.18 — Match expressions.
//
// FLS §6.18: "A match expression branches on a pattern."
// Arms are tested in source order; the first matching arm executes.
//
// FLS §5.1: Wildcard pattern `_` — matches any value.
// FLS §5.2: Literal patterns — integer and boolean literals.
//
// Note: FLS §6.18 does not provide a direct code example in the spec text;
// this function is derived from the semantic description of match arms
// and the grammar in §6.18.
fn match_example(n: i32) -> i32 {
    match n {
        0 => 0,     // FLS §5.2: integer literal pattern 0
        1 => 1,     // FLS §5.2: integer literal pattern 1
        _ => 2,     // FLS §5.1: wildcard — matches any remaining value
    }
}

// FLS §6.18 — Match on a boolean scrutinee.
//
// FLS §4.3: The boolean type `bool` has two values: `true` and `false`.
// FLS §5.2: Boolean literal patterns.
// No FLS §6.18 example provided for bool scrutinees; derived from the spec.
fn match_bool_example(b: bool) -> i32 {
    match b {
        true  => 1,  // FLS §5.2: bool literal pattern true
        false => 0,  // FLS §5.2: bool literal pattern false
    }
}

// FLS §5.2 — Negative integer literal patterns.
//
// FLS §5.2: "A LiteralPattern matches a value if the value equals the
// literal value." Negative integer literals are valid literal patterns
// (e.g., `-1`, `-42`).
//
// Note: FLS §5.2 describes literal patterns but does not provide a direct
// code example for negative literals. This function is derived from the
// spec's description and is equivalent to how rustc handles it.
fn match_negative_pattern(n: i32) -> i32 {
    match n {
        -2 => 10,   // FLS §5.2: negative integer literal pattern -2
        -1 => 20,   // FLS §5.2: negative integer literal pattern -1
        0  => 30,   // FLS §5.2: integer literal pattern 0
        _  => 40,   // FLS §5.1: wildcard — matches remaining values
    }
}

// FLS §5.1.4 — Identifier patterns.
//
// FLS §5.1.4: "An identifier pattern matches any value and optionally binds
// it to the identifier." When used as a catch-all arm in a match expression,
// an identifier pattern both matches and binds the scrutinee value, making it
// available as a local variable in the arm body.
//
// Note: FLS §5.1.4 describes identifier patterns but does not provide a
// direct code example. This function is derived from the spec's description.
fn match_ident_pattern(x: i32) -> i32 {
    match x {
        0 => 0,     // FLS §5.2: literal pattern — matches zero exactly
        n => n * 2, // FLS §5.1.4: identifier pattern — binds x to n, doubles it
    }
}


// FLS §6.11 — Struct expressions.
//
// FLS §6.11: "A struct expression constructs an instance of a struct type."
// The field initialisers may appear in any order; the fields are stored in
// declaration order.
//
// Note: FLS §6.11 does not provide a self-contained runnable code example.
// This function is derived from the spec's semantic description.
struct TestPoint { x: i32, y: i32 }

fn struct_expr_example() -> i32 {
    let p = TestPoint { x: 10, y: 20 };
    p.x + p.y   // FLS §6.13: field access — loads x then y
}

// FLS §6.11 — Shorthand field initialization.
//
// FLS §6.11: "If a StructExpressionField consists of only an identifier,
// that identifier is both the field name and the value of the field."
// `Point { x, y }` is equivalent to `Point { x: x, y: y }`.
//
// Note: FLS §6.11 describes shorthand field initialization as a syntactic
// convenience; no separate code example is provided. This is derived from
// the spec's semantic description.
fn struct_expr_shorthand_example(x: i32, y: i32) -> i32 {
    let p = TestPoint { x, y };   // FLS §6.11: shorthand — x resolves to param x
    p.x + p.y
}

// FLS §6.13 — Field access expressions.
//
// FLS §6.13: "A field access expression evaluates the receiver operand and
// then accesses one of its fields."
//
// Note: FLS §6.13 does not provide a self-contained runnable code example.
// This function is derived from the spec's semantic description.
fn field_access_example() -> i32 {
    let r = TestPoint { x: 6, y: 7 };
    r.x * r.y   // FLS §6.13: field access on both fields
}

// FLS §6.12.2 + §10.1 — Method call expressions and associated functions.
//
// FLS §6.12.2: "A method call expression is a call expression whose function
// is a method, that is, a function associated with a type."
// FLS §10.1: "Methods are functions associated with a specific type. Methods
// are defined within impl blocks."
//
// Note: FLS §6.12.2 provides no self-contained runnable example.
// This is derived from the spec's semantic description (§10.1 + §6.12.2).
struct MethodPoint { x: i32, y: i32 }

impl MethodPoint {
    fn sum(&self) -> i32 { self.x + self.y }   // FLS §10.1: &self method
    fn scale_x(&self, n: i32) -> i32 { self.x * n }  // extra param beyond self
}

fn method_call_example() -> i32 {
    let p = MethodPoint { x: 3, y: 4 };
    p.sum()   // FLS §6.12.2: method call — passes self fields as leading args
}

fn method_call_with_arg_example() -> i32 {
    let p = MethodPoint { x: 5, y: 0 };
    p.scale_x(3)   // FLS §6.12.2: 5 * 3 = 15
}

// FLS §6.8 — Array expressions.
//
// FLS §6.8: "An array expression constructs a value of an array type."
// FLS §6.8: "An array expression consists of a comma-separated list of
// operands of the same type." All elements must have the same type.
//
// Note: FLS §6.8 provides the syntax but no self-contained runnable example.
// This example is derived from the spec's semantic description.

fn array_literal_example() -> i32 {
    let a = [10, 20, 30];  // FLS §6.8: array of three i32 elements
    a[0]                   // FLS §6.9: index expression — returns first element (10)
}

fn array_index_middle_example() -> i32 {
    let a = [10, 20, 30];  // FLS §6.8
    a[1]                   // FLS §6.9: second element (20)
}

// FLS §6.9 — Indexing expressions.
//
// FLS §6.9: "An indexing expression is used to index into an array or slice."
// The index must be of type `usize` (spec); galvanic uses `i32` at this milestone.
//
// FLS §6.9 AMBIGUOUS: The spec requires bounds checking (panic on out-of-bounds),
// but does not specify the panic mechanism without the standard library.
// No bounds check is emitted at this milestone.

fn array_variable_index_example() -> i32 {
    let a = [5, 10, 15];  // FLS §6.8
    let i = 2;
    a[i]                  // FLS §6.9: runtime index — loads element at position i (15)
}

// FLS §6.5.10 + §6.9 — Array element store expressions.
//
// FLS §6.5.10: "An assignment expression assigns a new value to a place."
// When the place is an indexing expression (FLS §6.9), the assignment stores
// to the element at the given index.
//
// FLS §6.9: "An indexing expression is used to index into an array or slice."
//
// FLS §6.8: Array literals construct the initial value.
//
// Note: FLS §6.5.10 provides no self-contained runnable example of indexed
// assignment. This function is derived from the combined semantics of §6.5.10
// (assignment to a place) and §6.9 (indexing as a place expression).
//
// FLS §6.5.10 AMBIGUOUS: The spec does not enumerate which place expression
// forms are valid LHS for assignment. Galvanic restricts the base to a simple
// variable path, consistent with the restriction on plain scalar assignment.
fn array_store_example() -> i32 {
    let mut a = [1, 2, 3];   // FLS §6.8: array literal
    a[0] = 10;               // FLS §6.5.10 + §6.9: store to first element
    a[1] = 20;               // FLS §6.5.10 + §6.9: store to second element
    a[2] = 30;               // FLS §6.5.10 + §6.9: store to third element
    a[0] + a[1] + a[2]       // FLS §6.9: load all three — result is 60
}

// ── FLS §14.2 + §6.10 — Tuple struct construction and field access ────────────
//
// FLS §14.2: "A tuple struct is a struct with positional fields."
// Tuple structs are constructed with call-like syntax: `Point(a, b)`.
// Fields are accessed via integer indices: `.0`, `.1`.
//
// FLS §6.10: "A tuple expression is a parenthesized list of expressions."
// Tuple struct fields use the same positional-index access mechanism as
// anonymous tuples (FLS §6.13 for field access; §6.10 for the index syntax).
//
// Note: The FLS §14.2 entry describes tuple struct syntax but does not provide
// a self-contained runnable example. This function is derived from the
// combined semantics of §14.2 (struct item definition) and §6.10 (field access).
//
// Cache-line note: same slot layout as anonymous tuples — N consecutive
// 8-byte slots, field `.i` at `base_slot + i`.
struct Pair(i32, i32);

fn tuple_struct_example() -> i32 {
    let p = Pair(3, 7);  // FLS §14.2: constructor call syntax
    p.0 + p.1            // FLS §6.10: positional field access (3 + 7 = 10)
}

// ── FLS §6.11 — Struct update syntax ─────────────────────────────────────────
//
// FLS §6.11: "A struct expression with a base struct may specify a subset of
// the fields for the new value; all remaining fields have their values taken
// from the base struct expression."
//
// Syntax: `Struct { field: val, ..base }` — the `..base` fills in all fields
// not explicitly listed, copying them from `base` at runtime.
//
// Note: The FLS §6.11 section describes struct update syntax but does not
// provide a self-contained runnable example. This function is derived from
// the semantic description in §6.11.
//
// Cache-line note: copying a field from base emits one `ldr` + one `str`
// (8 bytes), the same footprint as storing an explicitly initialised field.
struct Origin { x: i32, y: i32 }

fn struct_update_example() -> i32 {
    let a = Origin { x: 1, y: 2 };    // FLS §6.11: full struct literal
    let b = Origin { x: 5, ..a };     // FLS §6.11: update syntax — y copied from a
    b.x + b.y                          // 5 + 2 = 7
}

// FLS §6.11: Struct expressions with struct-type fields.
// FLS §6.13: Chained field access expressions `receiver.field.field`.
//
// A struct may have fields whose types are themselves named structs. The
// inner struct's fields occupy consecutive stack slots within the outer
// struct's layout, in declaration order (FLS §4.11).
//
// Note: FLS §6.11 describes struct expressions but provides no example of
// nested struct literals specifically. This example is derived from the
// general struct expression semantics combined with field access (§6.13).
//
// FLS §6.13: "A field access expression accesses a field of a struct value."
// Chained access `outer.inner.x` applies §6.13 twice: first to get `inner`
// (a place of struct type), then again to get `x` (a scalar place).
//
// Cache-line note: A struct with two 2-field sub-structs occupies 4 consecutive
// 8-byte slots (32 bytes = half a cache line). The nested field `inner.x` at
// slot offset 1 within the sub-struct is adjacent to `inner.y` at offset 2 —
// both fit in the same cache line as `outer` itself.
struct Vec2 { x: i32, y: i32 }
struct Segment { start: Vec2, end_pt: Vec2 }

fn chained_field_access_example() -> i32 {
    // FLS §6.11: nested struct literal — `start` and `end_pt` are Vec2 values.
    // FLS §4.11: layout: start.x=slot0, start.y=slot1, end_pt.x=slot2, end_pt.y=slot3.
    let s = Segment {
        start: Vec2 { x: 0, y: 0 },
        end_pt: Vec2 { x: 3, y: 4 },
    };
    // FLS §6.13: `s.end_pt.x` = slot2, `s.end_pt.y` = slot3.
    // Manhattan distance: |end_pt.x - start.x| + |end_pt.y - start.y| = 3 + 4 = 7.
    s.end_pt.x + s.end_pt.y
}

// FLS §8.1 — Let statement with shadowing (variable shadowing via re-binding).
//
// FLS §8.1: "A let statement introduces a new binding for the remainder of
// the enclosing block." The binding is NOT in scope during evaluation of the
// initializer expression. Thus `let x = x + 3` evaluates `x` in the scope
// where `x` refers to the previous binding, not the one being introduced.
//
// The FLS does not provide a direct code example for shadowing; this is
// derived from the semantic description in §8.1 and the scoping rules in §14.
fn shadow_example() -> i32 {
    let x = 5;          // FLS §8.1: x bound to 5 (slot 0)
    let x = x + 3;      // FLS §8.1: RHS reads old x (5), new x = 8 (slot 1)
    let x = x * 2;      // FLS §8.1: RHS reads slot 1 (8), new x = 16 (slot 2)
    x                   // returns 16
}

// FLS §6.5.1 — Borrow expression `&place`.
// FLS §6.5.2 — Dereference expression `*expr`.
//
// FLS §6.5.1: "A borrow expression also called a reference expression is a
// unary operator expression that uses the borrow operator."
// FLS §6.5.2: "A dereference expression also called a deref expression is a
// unary operator expression that uses the dereference operator."
//
// The FLS does not provide a standalone code example for borrow/deref; this
// function is derived from the semantic descriptions in §6.5.1 and §6.5.2.
//
// ARM64 codegen:
//   `&n`  → `add x{dst}, sp, #{slot * 8}`   (one instruction, FLS §6.5.1)
//   `*x`  → `ldr x{dst}, [x{src}]`          (one instruction, FLS §6.5.2)
//
// Cache-line note: each borrow or deref emits exactly one 4-byte instruction.
// A borrow-then-deref round-trip (`*(&n)`) is therefore 2 instructions (8 bytes),
// fitting in the same instruction-slot pair as a load-store.
fn deref_ref_example(n: i32) -> i32 {
    // FLS §6.5.1: `&n` forms a pointer to n's stack slot.
    // FLS §6.5.2: `*x` loads through the pointer.
    let x: &i32 = &n;  // borrow: x holds the address of n
    *x                  // deref: load the value at that address
}

