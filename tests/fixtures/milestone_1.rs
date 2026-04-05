// Milestone 1: the minimal Rust program that galvanic can compile to a
// running ARM64 binary.
//
// FLS §9: Functions — the entry point of a Rust program is `main`.
// FLS §2.4.4.1: Integer literals — `0` is a decimal integer literal.
// FLS §6.2: Literal expressions — an integer literal is a literal expression.
// FLS §18.1: Crate entry point — the `main` function is the program entry point.
//
// Expected behavior: galvanic compiles this to an ARM64 ELF binary that
// exits with code 0.
//
// FLS §9 NOTE: The spec does not provide a specific example of the minimal
// main function; this program is the simplest well-formed Rust entry point
// derivable from §9 and §18.1.
fn main() -> i32 {
    0
}
