// FLS §2.4.4.1 — Large integer literal materialization via MOVZ+MOVK.
//
// Unsigned values greater than i32::MAX require more than a single MOV
// instruction on ARM64. Galvanic emits a MOVZ for the first non-zero
// 16-bit chunk and a MOVK for each subsequent non-zero chunk.
//
// This fixture exercises:
//   - u32 literal just above i32::MAX (2147483648 = 0x8000_0000)
//   - u32 literal at u32::MAX (4294967295 = 0xFFFF_FFFF)
//   - u32 literals with various non-zero chunk patterns
//   - u64 literal requiring lsl #32 (5_000_000_000 = 0x0001_2A05_F200)
//   - u64 literal requiring lsl #48 (0x1234_5678_9ABC_DEF0)
//
// FLS §6.1.2:37–45: Non-const code must emit runtime instructions even
// when all operands are statically known literals.

fn large_u32_hi_only() -> u32 {
    // 0x8000_0000 — chunk1 non-zero only; one MOVZ lsl #16.
    2147483648_u32
}

fn large_u32_two_chunks() -> u32 {
    // 0x8001_86A0 — chunk0 and chunk1 non-zero; MOVZ + one MOVK.
    2147582624_u32
}

fn large_u32_max() -> u32 {
    // 0xFFFF_FFFF — both 16-bit chunks non-zero; MOVZ + one MOVK.
    4294967295_u32
}

fn sum_large(a: u32, b: u32) -> u32 {
    a + b
}

fn large_u64_three_chunks() -> u64 {
    // 0x0001_2A05_F200 — chunks 0, 1, 2 non-zero; MOVZ + two MOVK (lsl#16, lsl#32).
    5000000000_u64
}

fn large_u64_four_chunks() -> u64 {
    // 0x1234_5678_9ABC_DEF0 — all four chunks non-zero; MOVZ + three MOVK (lsl#16, lsl#32, lsl#48).
    1311768467463790320_u64
}

fn main() {
    let hi = large_u32_hi_only();
    let two = large_u32_two_chunks();
    let max = large_u32_max();
    let _ = sum_large(hi, two);
    let _ = sum_large(two, max);
    let _ = large_u64_three_chunks();
    let _ = large_u64_four_chunks();
}
