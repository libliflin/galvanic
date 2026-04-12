// FLS §6.23: Arithmetic Overflow — Division by Zero
//
// This fixture documents galvanic's implementation of FLS §6.23 for division by zero.
//
// FLS §6.23 requires:
//   - Division by zero ALWAYS panics, even in release mode.
//   - Signed integer MIN / -1 ALWAYS panics (would overflow to out-of-range value).
//
// FLS §6.23 AMBIGUOUS: The spec requires a panic but does not specify the mechanism.
// Galvanic implements panic as exit(101) via Linux syscall (no unwinding, no stack trace).
// The exit code 101 is galvanic's internal sentinel — not mandated by the FLS.
//
// Galvanic implementation:
//   - A `cbz xRHS, _galvanic_panic` guard is emitted before every `sdiv`/`udiv`.
//   - `_galvanic_panic` is: `mov x0, #101; mov x8, #93; svc #0` (exit(101)).
//   - The guard is emitted only for modules that contain division operations;
//     programs without division do not include `_galvanic_panic` in their assembly.
//
// FLS §6.23 AMBIGUOUS: Signed MIN / -1:
//   - ARM64 `sdiv` with i32::MIN / -1 returns 2147483648 (positive 64-bit value).
//   - This is outside the i32 range and constitutes signed overflow.
//   - FLS §6.23 requires this to panic. Galvanic does NOT insert a MIN/-1 guard.
//   - This divergence is documented as a known FLS §6.23 ambiguity.
//
// Claim 4m: compile-time literal zero divisor → compile error.
// Claim 4o: runtime zero divisor (parameter) → exit(101) via cbz guard.

fn div_by_zero_param(x: i32, y: i32) -> i32 {
    // FLS §6.23: panics at runtime when y == 0.
    // Galvanic emits: cbz xRHS, _galvanic_panic; sdiv xD, xN, xM
    x / y
}

fn min_div_neg_one(x: i32, y: i32) -> i32 {
    // FLS §6.23: should panic when x == i32::MIN and y == -1.
    // FLS §6.23 AMBIGUOUS: Galvanic emits no MIN/-1 guard — returns 2147483648.
    x / y
}

fn main() -> i32 {
    // Call with non-zero divisors — these are safe and return correct results.
    // The zero divisor case is exercised by claim_4o_runtime_div_zero_exits_101.
    let a = div_by_zero_param(10, 2);   // 5
    let b = min_div_neg_one(-100, 5);   // -20
    a + b                               // 5 + (-20) = -15; exit code wraps to 241 on u8
}
