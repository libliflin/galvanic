// FLS §6.23: Arithmetic Overflow
//
// This fixture demonstrates integer arithmetic at and near the i32 overflow
// boundary. It is derived from the semantic constraints in FLS §6.23.
//
// FLS §6.23 AMBIGUOUS: The spec requires:
//   - In a const context: overflow is a compile-time error.
//   - At runtime in debug mode: overflow panics.
//   - At runtime in release mode: overflow wraps (two's complement).
//
// Galvanic does not distinguish debug vs release mode. It uses ARM64 64-bit
// registers (xN) for i32 arithmetic, so integer overflow at the i32 boundary
// produces a large positive 64-bit value rather than wrapping to i32::MIN.
// This is non-conforming with Rust's release-mode 32-bit two's complement
// wrapping semantics and is documented here as a research output.
//
// FLS §6.23 AMBIGUOUS: The FLS does not specify the mechanism by which a
// panic should be raised (there is no standard panic runtime in no_std
// environments). Galvanic currently omits overflow checks entirely.

fn add_large(x: i32, y: i32) -> i32 {
    x + y
}

fn sub_from_large(x: i32, y: i32) -> i32 {
    x - y
}

fn mul_large(x: i32, y: i32) -> i32 {
    x * y
}

fn main() -> i32 {
    let a = add_large(1_000_000, 1_000_000);
    let b = sub_from_large(a, 999_900);
    mul_large(b, 1)
}
