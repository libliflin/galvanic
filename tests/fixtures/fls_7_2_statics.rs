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

fn main() -> i32 {
    // get_capacity() returns 10; sum_sides() returns 7; count_to_capacity() returns 10.
    // Return 10 + 7 - 10 = 7. Exit code 7.
    get_capacity() + sum_sides() - count_to_capacity()
}
