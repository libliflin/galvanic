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
// Galvanic does not distinguish debug vs release mode. After each `add`, `sub`,
// or `mul` instruction, galvanic emits `sxtw x9, w{dst}` + `cmp x{dst}, x9`
// + `b.ne _galvanic_panic` to detect signed i32 overflow at runtime (Claim 4s).
//
// FLS §6.23 AMBIGUOUS: The FLS does not specify the panic mechanism. Galvanic
// uses `_galvanic_panic` (sys_exit with code 101), matching the convention
// established for divide-by-zero (Claim 4o) and bounds checks (Claim 4p).
//
// FLS §6.23 AMBIGUOUS: Galvanic's BinOp IR has no type annotations, so the
// overflow guard also fires for i64/u32 operations — false positives for
// non-i32 integer types. Documented in refs/fls-ambiguities.md.

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
