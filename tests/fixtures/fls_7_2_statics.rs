// FLS §7.2: Static Items
//
// "A static is a value that is allocated in memory and has a lifetime for the
// entire program execution."
// FLS §7.2:15: "All references to a static refer to the same memory address."
//
// Key difference from FLS §7.1 (const items):
// - `const`: value is substituted at every use site (no memory address)
// - `static`: value resides at a fixed address in the data section; every
//   reference loads from that address via ADRP + ADD + LDR
//
// This fixture derives examples from FLS §7.2. The spec does not provide
// runnable code examples; programs below are derived from the semantic
// description and FLS §7.2:15.
//
// FLS §7.2 AMBIGUOUS: The spec does not specify the required data section
// alignment. Galvanic uses .align 3 (8-byte) matching the 64-bit LDR
// requirement on ARM64.

// FLS §7.2: An immutable static with an integer literal initializer.
// Unlike `const MAX_LEN: i32 = 10` (which substitutes 10 inline),
// every use of CAPACITY loads from the same data section address.
static CAPACITY: i32 = 10;

// FLS §7.2: A zero-valued static.
static INITIAL: i32 = 0;

// FLS §7.2: Multiple statics in the same file.
// Each occupies its own address in the .data section.
static SIDE_A: i32 = 3;
static SIDE_B: i32 = 4;

// A function that reads a static.
// FLS §7.2:15: The return value is loaded from CAPACITY's fixed address.
// ARM64: ADRP x0, CAPACITY; ADD x0, x0, :lo12:CAPACITY; LDR x0, [x0]
fn get_capacity() -> i32 {
    CAPACITY
}

// A function that uses a static in arithmetic.
// FLS §6.5.5: Runtime addition; static is loaded from memory first.
fn sum_sides() -> i32 {
    SIDE_A + SIDE_B
}

// A function that uses a static as a loop bound.
// FLS §6.15.3: While loop — CAPACITY is loaded from memory on each check.
// FLS §7.2:15: Each loop iteration re-reads CAPACITY's address.
fn count_to_capacity() -> i32 {
    let mut i = INITIAL;
    while i < CAPACITY {
        i += 1;
    }
    i
}

// FLS §7.2, §4.2: f64 static item.
// Stored as raw IEEE 754 bits (.quad) in the .data section.
// Every use loads from the same memory address (ADRP + ADD + LDR d).
static GRAVITY: f64 = 9.0;

// FLS §7.2, §4.2: f32 static item.
// Stored as raw IEEE 754 bits (.word) in the .data section.
// Cache-line note: 4 bytes — half the footprint of an f64 static.
static SCALE_F32: f32 = 2.0_f32;

// A function that reads an f64 static.
// FLS §7.2:15: The return value is loaded from GRAVITY's fixed address.
// ARM64: ADRP x17, GRAVITY; ADD x17, x17, :lo12:GRAVITY; LDR d0, [x17]
fn get_gravity() -> i32 {
    GRAVITY as i32
}

// A function that uses an f32 static in arithmetic.
// FLS §7.2, §6.5.5: f32 static loaded then added to a literal.
fn scale_plus_one() -> i32 {
    (SCALE_F32 + 1.0_f32) as i32
}

fn main() -> i32 {
    // get_capacity() returns 10; sum_sides() returns 7; count_to_capacity() returns 10.
    // get_gravity() returns 9; scale_plus_one() returns 3.
    // Return 10 + 7 - 10 + 9 - 3 = 13 — but we clamp to valid exit code range.
    // Actually: return (10 + 7 - 10) % 100 = 7. Keep original exit code.
    // Add f64/f32 statics: 7 + 9 - 3 = 13. Exit code 13.
    get_capacity() + sum_sides() - count_to_capacity() + get_gravity() - scale_plus_one()
}
