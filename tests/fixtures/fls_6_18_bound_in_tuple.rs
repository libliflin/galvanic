// FLS §6.18, §5.1.4 — @ binding patterns nested inside tuple match arms.
//
// Verifies that `a @ pattern` in a tuple-position sub-pattern correctly:
//   1. checks the sub-pattern against the scrutinee element, and
//   2. binds the matched value to the name `a`.

fn classify(x: i32, y: i32) -> i32 {
    match (x, y) {
        (a @ 1..=10, 0) => a,
        (0, b @ 1..=10) => b,
        _ => -1,
    }
}

fn main() {
    let r0 = classify(5, 0);
    let r1 = classify(0, 7);
    let r2 = classify(0, 0);
    let _ = r0;
    let _ = r1;
    let _ = r2;
}
