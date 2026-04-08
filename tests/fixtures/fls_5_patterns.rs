// FLS §5 — Pattern examples from the Ferrocene Language Specification.
// Source: https://rust-lang.github.io/fls/patterns.html
//
// Each pattern form is exercised inside a function body.
// Examples are derived from FLS §5; where the spec provides no code example,
// that is noted explicitly.

// FLS §5.1: Wildcard pattern `_` — matches any value without binding.
// FLS §5.1.4: Identifier pattern — binds matched value to a name.
// FLS §5.1.9: Range patterns — `lo..=hi` (inclusive) and `lo..hi` (exclusive).
// FLS §5.1.11: Or patterns — `p0 | p1 | ...`.
// FLS §5.2: Literal patterns — integer literals (including negative).

// FLS §5.1.9: Inclusive range pattern `lo..=hi`.
// The spec does not provide a concrete code example; this is derived from the
// semantic description: "A range pattern matches any value that falls within
// the range's bounds."
fn range_inclusive(x: i32) -> i32 {
    match x {
        1..=3 => 1,
        4..=6 => 2,
        _ => 0,
    }
}

// FLS §5.1.9: Exclusive range pattern `lo..hi`.
// FLS §5.1.9 AMBIGUOUS: The spec describes range patterns but does not
// explicitly distinguish whether `..` and `..=` are both valid in all
// positions. Galvanic supports both forms per the Rust Reference.
fn range_exclusive(x: i32) -> i32 {
    match x {
        1..4 => 1,
        4..7 => 2,
        _ => 0,
    }
}

// FLS §5.1.9: Range pattern with negative lower bound.
// FLS §5.2: Negative literal patterns are valid as range bounds.
fn range_negative(x: i32) -> i32 {
    match x {
        -5..=-1 => 1,
        0 => 2,
        _ => 3,
    }
}

// FLS §6.18: Match arm guard — `if <expr>` after the pattern.
// FLS §6.18: "A match arm guard is an additional condition that must hold
// for the arm to be selected."
// Note: The FLS does not provide a concrete code example; this is derived
// from the semantic description of MatchArmGuard.
fn classify_with_guard(x: i32) -> i32 {
    match x {
        n if n > 0 => 1,
        n if n < 0 => 2,
        _ => 0,
    }
}

// FLS §6.17: if-let expression — `if let Pattern = Expr { Block } [else Block]`.
// FLS §6.17: "An if let expression is syntactic sugar for a match expression
// with a single arm."
// Note: The FLS does not provide a concrete code example; this is derived
// from the semantic description of IfLetExpression.
fn check_exact(x: i32) -> i32 {
    // FLS §6.17 + §5.2: Integer literal pattern in if-let.
    if let 42 = x { 1 } else { 0 }
}

// FLS §6.17 + §5.1.9: Range pattern in if-let.
fn check_range(x: i32) -> i32 {
    if let 1..=10 = x { 1 } else { 0 }
}

// FLS §6.17 + §5.1.4: Identifier pattern in if-let — always matches, binds value.
fn bind_and_use(x: i32) -> i32 {
    if let n = x { n + 1 } else { 0 }
}

// FLS §5.5: Path pattern — matches an enum unit variant by its discriminant.
// FLS §15: Enumerations. Unit variants receive discriminants 0, 1, 2, ...
// in declaration order. The FLS does not provide a concrete path-pattern code
// example; this is derived from the semantic description of PathPattern.
enum Direction {
    North,
    South,
    East,
    West,
}

fn describe_direction(d: i32) -> i32 {
    // FLS §5.5 + §6.18: Match on enum value using path patterns.
    match d {
        Direction::North => 0,
        Direction::South => 1,
        Direction::East => 2,
        Direction::West => 3,
    }
}

// FLS §15: Enum with a tuple variant — stores discriminant + fields.
// FLS §5.4: Tuple struct/variant pattern `Opt::Some(v)` matches the
// variant and binds the first field to `v`.
// FLS §5.4 NOTE: The spec does not provide a self-contained code example
// for tuple struct patterns; this is derived from the semantic description.
enum Opt {
    None,
    Some(i32),
}

// FLS §15 + §5.4: Construct a tuple variant, match it, extract the field.
fn unwrap_or_zero(o: Opt) -> i32 {
    match o {
        Opt::Some(v) => v,
        Opt::None => 0,
    }
}

// FLS §5.10.2: Struct pattern — `StructName { field1, field2 }` in a let statement.
// FLS §8.1: Let statements accept any irrefutable pattern, including struct
// patterns. FLS §5.10.2: "A struct pattern is a pattern that matches a struct
// or enum struct variant."
//
// Note: The FLS does not provide a concrete code example for struct patterns
// in let position; this is derived from the semantic description in §5.10.2
// and the let statement grammar in §8.1.
struct Vec2 { x: i32, y: i32 }

fn magnitude_sq(v: Vec2) -> i32 {
    // FLS §5.10.2: Destructure a struct variable — binds `x` and `y`.
    let Vec2 { x, y } = v;
    x * x + y * y
}

// FLS §5.10.3: Tuple pattern — `(p0, p1, ...)` in a let statement.
// FLS §8.1: Let statements accept any irrefutable pattern, including tuple
// patterns. FLS §5.10.3: "A tuple pattern is a pattern that matches a tuple
// which satisfies all criteria defined by its subpatterns."
//
// Note: The FLS does not provide a concrete code example for tuple patterns
// in let position; this is derived from the semantic description in §5.10.3
// and the let statement grammar in §8.1.
fn swap(x: i32, y: i32) -> i32 {
    // FLS §5.10.3: Destructure a tuple literal — binds `a` and `b`.
    let (a, b) = (y, x);
    // Returns `a - b` = `y - x` as a sanity check that elements were swapped.
    a - b
}

fn sum_pair(x: i32, y: i32) -> i32 {
    // FLS §5.10.3: Destructure into named bindings, then use in arithmetic.
    let (p, q) = (x, y);
    p + q
}

// FLS §5.10.3: Nested tuple pattern — `(p0, (p1, p2))` in a let statement.
// FLS §5.10.3: "A tuple pattern is a pattern that matches a tuple which
// satisfies all criteria defined by its subpatterns." Sub-patterns may
// themselves be tuple patterns, giving recursive destructuring.
//
// Note: The FLS does not provide a concrete code example for nested tuple
// patterns in let position; this is derived from the recursive sub-pattern
// grammar in §5.10.3 and the let statement grammar in §8.1.
fn nested_sum(x: i32, y: i32, z: i32) -> i32 {
    // FLS §5.10.3: Nested tuple literal — binds `a`, `b`, `c` from two levels.
    let (a, (b, c)) = (x, (y, z));
    a + b + c
}

// FLS §5.10.4: Tuple struct pattern — `TupleStructName(p0, p1, ...)` in a
// let statement. FLS §8.1: Let statements accept any irrefutable pattern,
// including tuple struct patterns. FLS §5.10.4: "A tuple struct pattern is a
// pattern that matches a tuple struct or enum variant."
//
// Note: The FLS does not provide a concrete code example for tuple struct
// patterns in let position; this is derived from the semantic description in
// §5.10.4 and the let statement grammar in §8.1.
struct Coord(i32, i32);

fn dist_sq(c: Coord) -> i32 {
    // FLS §5.10.4: Destructure a tuple struct variable — binds `x` and `y`.
    let Coord(x, y) = c;
    x * x + y * y
}

// FLS §5.10.2, §9.2: Struct pattern in parameter position.
// `Point { x, y }: Point` binds `x` and `y` directly from the incoming
// registers — equivalent to `p: Point` followed by `let Point { x, y } = p;`.
//
// FLS §9.2 AMBIGUOUS: The spec allows arbitrary irrefutable patterns in
// parameter position but does not enumerate them independently — the reader
// must cross-reference §5. No concrete code example is provided in the spec;
// this is derived from §5.10.2 semantics (struct patterns are irrefutable and
// bind each named field) applied to the parameter context from §9.2.
struct Rect { w: i32, h: i32 }

fn area(Rect { w, h }: Rect) -> i32 {
    // FLS §5.10.2: `w` and `h` are bound directly from the incoming registers.
    w * h
}

// FLS §5.10.4, §9.2: Tuple struct pattern in parameter position.
// The spec does not provide a canonical example; this is derived from §5.10.4
// (TupleStructPattern) and §9.2 (FunctionParameters).
struct Scale(i32, i32);

fn scaled_diff(Scale(a, b): Scale) -> i32 {
    // FLS §5.10.4: `a` and `b` are bound from the incoming registers.
    // `Scale(3, 1)` → a=3, b=1 → 3-1 = 2.
    a - b
}

// FLS §5.10.3, §9.2: Nested tuple pattern in parameter position.
// `(a, (b, c)): (i32, (i32, i32))` — three leaves arrive in x0, x1, x2
// and bind to `a`, `b`, `c` respectively.
//
// FLS §5.10.3: "A tuple pattern is a pattern that matches a tuple which
// satisfies all criteria defined by its subpatterns." Sub-patterns may
// themselves be tuple patterns. FLS §9.2: Irrefutable patterns (including
// nested tuple patterns) are valid in parameter position.
//
// FLS §9.2 AMBIGUOUS: The spec does not provide a concrete code example
// for nested tuple patterns in parameter position; this is derived from
// the recursive sub-pattern grammar in §5.10.3 and the parameter grammar
// in §9.2.
fn nested_param_sum((a, (b, c)): (i32, (i32, i32))) -> i32 {
    // FLS §5.10.3: `a`, `b`, `c` are bound from consecutive incoming registers.
    a + b + c
}

// FLS §5.10.2, §9.2: Nested struct pattern in parameter position.
// `Outer { inner: Inner { a, b }, c }: Outer` — three scalar leaves arrive in
// x0 (inner.a), x1 (inner.b), x2 (c) and bind to `a`, `b`, `c`.
//
// FLS §5.10.2: "A struct pattern matches a struct [...] by its field patterns."
// Field patterns may themselves be struct patterns (nested). FLS §9.2:
// Irrefutable patterns (including nested struct patterns) are valid in
// parameter position.
//
// FLS §9.2 AMBIGUOUS: The spec does not provide a concrete code example for
// nested struct patterns in parameter position; this is derived from the
// recursive field-pattern grammar in §5.10.2 and the parameter grammar §9.2.
struct Inner { a: i32, b: i32 }
struct Outer { inner: Inner, c: i32 }

fn nested_struct_param_sum(Outer { inner: Inner { a, b }, c }: Outer) -> i32 {
    // FLS §5.10.2: `a` and `b` are bound from the inner struct's registers;
    // `c` is bound from the register following the inner struct's fields.
    a + b + c
}

// FLS §5.1.8: Slice/array pattern — `[p0, p1, ...]` matches a fixed-size array
// and binds each element to the corresponding sub-pattern.
//
// FLS §5.1.8: "A slice pattern matches an array or slice type and destructures
// its elements."
//
// Note: The FLS does not provide a concrete code example for slice patterns
// in let position; this is derived from the semantic description in §5.1.8
// and the let statement grammar in §8.1.
fn sum_array_destruct(a: i32, b: i32, c: i32) -> i32 {
    // FLS §5.1.8: Destructure a local array variable into three bindings.
    let arr = [a, b, c];
    let [x, y, z] = arr;
    x + y + z
}

// FLS §5.1.8 + §5.1: Slice pattern with wildcard sub-patterns.
// `_` discards an element without binding it.
fn first_of_three(a: i32) -> i32 {
    let arr = [a, 0, 0];
    let [first, _, _] = arr;
    first
}

fn main() -> i32 {
    // FLS §5.1.9: inclusive range — value 2 in [1,3] → 1.
    let a = range_inclusive(2);
    // FLS §5.1.9: exclusive range — value 5 in [4,7) → 2.
    let b = range_exclusive(5);
    // FLS §5.1.9: negative range — value -3 in [-5,-1] → 1.
    let c = range_negative(-3);
    // FLS §6.18: guard — positive value 5 → 1.
    let d = classify_with_guard(5);
    // FLS §6.17: if-let literal — 42 matches 42 → 1.
    let e = check_exact(42);
    // FLS §6.17: if-let range — 5 in [1,10] → 1.
    let f = check_range(5);
    // FLS §6.17: if-let ident — bind 3 to n, return n+1=4.
    let g = bind_and_use(3);
    // FLS §5.5: path pattern — Direction::East discriminant 2 → match arm → 2.
    let h = describe_direction(Direction::East);
    // FLS §15 + §5.4: tuple variant Some(3) → v=3; None → 0.
    let s = Opt::Some(3);
    let i = unwrap_or_zero(s);
    let n = Opt::None;
    let j = unwrap_or_zero(n);
    // FLS §5.10.3: tuple pattern in let — swap(3,5): (a,b)=(5,3) → a-b=2.
    let k = swap(3, 5);
    // FLS §5.10.3: sum via tuple destructure — sum_pair(1,2)=3.
    let m = sum_pair(1, 2);
    // FLS §5.10.2: struct destructure — magnitude_sq(Vec2 { x:1, y:0 }) = 1.
    let v = Vec2 { x: 1, y: 0 };
    let n2 = magnitude_sq(v);
    // FLS §5.10.4: tuple struct destructure — dist_sq(Coord(1,0)) = 1.
    let p = Coord(1, 0);
    let n3 = dist_sq(p);
    // FLS §5.10.3: nested tuple — nested_sum(1,2,4) = 7.
    let q = nested_sum(1, 2, 4);
    // FLS §5.10.2, §9.2: struct pattern param — area(Rect { w:3, h:4 }) = 12.
    let r = Rect { w: 3, h: 4 };
    let r2 = area(r);
    // FLS §5.10.4, §9.2: tuple struct pattern param — scaled_diff(Scale(3,1)) = 2.
    let sc = Scale(3, 1);
    let r3 = scaled_diff(sc);
    // FLS §5.10.3, §9.2: nested tuple pattern param — nested_param_sum((1,(2,3))) = 6.
    let r4 = nested_param_sum((1, (2, 3)));
    // FLS §5.10.2, §9.2: nested struct pattern param — nested_struct_param_sum(Outer { inner: Inner { a:1, b:2 }, c:3 }) = 6.
    let inner = Inner { a: 1, b: 2 };
    let outer = Outer { inner, c: 3 };
    let r5 = nested_struct_param_sum(outer);
    // FLS §5.1.8: slice pattern — sum_array_destruct(1,2,3) = 6.
    let s1 = sum_array_destruct(1, 2, 3);
    // FLS §5.1.8 + §5.1: wildcard sub-pattern — first_of_three(5) = 5.
    let s2 = first_of_three(5);
    // FLS §5.1.4: @ binding pattern — bind AND check a sub-pattern.
    // `n @ 1..=5` binds the value to `n` only when `n` is in [1, 5].
    // FLS §5.1.4 NOTE: The spec does not provide a concrete code example for
    // @ patterns; this is derived from the semantic description of IdentifierPattern.
    let t1 = match 3 {
        n @ 1..=5 => n * 2,
        _ => 0,
    };
    // FLS §5.1.4: @ with a literal sub-pattern — bind AND check equality.
    let t2 = match 42 {
        n @ 42 => n,
        _ => 0,
    };
    // a=1, b=2, c=1, d=1, e=1, f=1, g=4, h=2, i=3, j=0, k=2, m=3, n2=1, n3=1, q=7,
    // r2=12, r3=2, r4=6, r5=6, s1=6, s2=5, t1=6, t2=42 → sum=115
    a + b + c + d + e + f + g + h + i + j + k + m + n2 + n3 + q + r2 + r3 + r4 + r5 + s1 + s2 + t1 + t2
}
