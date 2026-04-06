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

    // FLS §2.4.5 — Character literals
    // Simple ASCII characters, escape sequences, and a Unicode escape.
    // FLS §2.4.5: A character literal is a char-typed expression whose value
    // is a Unicode scalar value.
    let _j = 'a';
    let _k = 'Z';
    let _l = '0';
    let _m = '\n';
    let _n = '\t';
    let _o = '\\';
    let _p = '\u{1F600}';

    // FLS §2.4.1 — Byte literals
    // A byte literal has the form `b'...'` and type `u8`.
    // Galvanic maps `u8` to `IrTy::U32` (zero-extended in 64-bit register).
    let _q: u8 = b'A';
    let _r: u8 = b'0';
    let _s: u8 = b'\n';
    let _t: u8 = b'\t';
    let _u: u8 = b'\\';

    // FLS §2.4.6 — String literals
    // A string literal has type `&str`.  The FLS does not provide standalone
    // example programs for `.len()`; these are derived from the section's
    // semantic description (§2.4.6: "a sequence of Unicode characters").
    // Galvanic materialises the UTF-8 byte length as a runtime immediate.
    let _v = "hello";           // 5 bytes
    let _w: &str = "world!";    // 6 bytes
    let _x = "";                // 0 bytes
    let _y = "a\nb";            // 3 bytes (\n is 1 byte)
    let _z = "a\tb";            // 3 bytes (\t is 1 byte)
}
