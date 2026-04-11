// FLS §6.23: Arithmetic Overflow — Division by Zero
//
// This fixture documents the FLS §6.23 divergence for division by zero on ARM64.
//
// FLS §6.23 requires:
//   - Division by zero ALWAYS panics, even in release mode.
//   - Signed integer MIN / -1 ALWAYS panics (would overflow to out-of-range value).
//
// FLS §6.23 AMBIGUOUS: The spec requires a panic but does not specify the mechanism.
// In Rust's standard library, panic! is the mechanism. In a no_std environment like
// galvanic's target (bare ARM64 ELF), there is no standard panic handler.
//
// Galvanic ARM64 divergence (documented research output):
//   - ARM64 `sdiv xD, xN, xM` where xM=0 produces xD=0 WITHOUT raising any exception.
//     This is architecturally defined behaviour (ARM DDI 0487, C3.4.8: "The SDIV
//     instruction performs signed integer division. A division by zero returns a zero
//     result regardless of the value of the numerator.").
//   - This is different from x86/x64 `idiv` which raises SIGFPE on division by zero.
//   - Galvanic does not insert a zero-divisor guard before `sdiv`/`udiv` instructions.
//   - Therefore: `1 / 0` on galvanic/ARM64 returns 0 silently (no panic, no crash).
//   - Expected per FLS: program panics (non-zero exit or signal).
//   - Actual on galvanic: program exits 0 when the quotient is used as the return value.
//
// FLS §6.23 AMBIGUOUS: Signed MIN / -1:
//   - ARM64 `sdiv` clamps the result to the register width rather than trapping.
//   - `i32::MIN / -1` on ARM64 with 64-bit registers: (-2147483648) / (-1) = 2147483648,
//     which fits in a 64-bit register without wrapping. Result is i32-truncated to
//     -2147483648 only if the caller truncates — galvanic does NOT insert such truncation
//     for i32 division results, so the result is 2147483648 (positive, wrong).
//   - FLS §6.23 requires this to panic. Galvanic returns the architecturally-defined
//     64-bit result without a check.
//
// This fixture exists to parse correctly and to serve as a research record.
// An e2e test on this fixture would reveal:
//   compile_and_run("fls_6_23_div_zero.rs") -> exit 0
//   but FLS §6.23 requires: panic (non-zero exit or signal)
//
// When galvanic adds divide-by-zero guards, this fixture and claim C8 should be updated.

fn div_by_zero_param(x: i32, y: i32) -> i32 {
    // FLS §6.23: should panic at runtime when y == 0.
    // Galvanic emits: sdiv xD, xN, xM — returns 0 if xM == 0 (ARM64 defined).
    x / y
}

fn min_div_neg_one(x: i32, y: i32) -> i32 {
    // FLS §6.23: should panic when x == i32::MIN and y == -1.
    // Galvanic emits: sdiv without a check — returns 2147483648 (overflows i32 range).
    x / y
}

fn main() -> i32 {
    // Call with non-zero divisors — these are safe and return correct results.
    // The dangerous cases (y=0, MIN/-1) are not called here to avoid undefined behaviour
    // at the fixture-parse level. The divergence is documented above.
    let a = div_by_zero_param(10, 2);   // 5
    let b = min_div_neg_one(-100, 5);   // -20
    a + b                               // -15 + 20 => actually 5 + (-20) = -15
}
