// FLS §2.4 — Literal examples from the Ferrocene Language Specification.
// Each literal is placed in a let binding inside main() so it forms a valid
// program that galvanic should be able to lex and (eventually) compile.
//
// Source: https://rust-lang.github.io/fls/lexical-elements.html

fn main() {
    // FLS §2.4.4.1 — Integer literals
    let _a = 0b0010_1110_u8;
    let _b = 1___2_3;
    let _c = 0x4D8a;
    let _d = 0o77_52i128;

    // FLS §2.4.4.2 — Float literals
    let _e = 8E+1_820;
    let _f = 3.14e5;
    let _g = 8_031.4_e-12f64;

    // FLS §2.4.7 — Boolean literals
    let _h = true;
    let _i = false;
}
