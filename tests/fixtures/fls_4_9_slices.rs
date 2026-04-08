// FLS §4.9: Slice types — dynamically sized views into a contiguous sequence.
//
// A slice type `[T]` is an unsized type. It is almost always used through a
// reference `&[T]` or mutable reference `&mut [T]`, which are fat pointers:
// a (data pointer, length) pair.
//
// FLS §4.9: AMBIGUOUS — The spec defines `&[T]` as a "reference to a slice"
// but does not specify the fat pointer ABI. Galvanic passes `&[T]` as two
// consecutive ARM64 registers (data pointer + element count), matching
// the callee's two-slot spill layout. This is an implementation choice.
//
// FLS §4.9: AMBIGUOUS — Index bounds checking is described as causing a panic
// in the spec, but the mechanism (trap, call to runtime, explicit compare) is
// not specified. Galvanic omits bounds checking at this milestone.
//
// FLS §6.9: Indexing a slice `s[i]` is defined as accessing the element at
// offset `i`, which requires pointer arithmetic through the fat pointer.

// Return the number of elements in a slice parameter.
fn slice_len(s: &[i32]) -> i32 {
    s.len()
}

// Sum all elements in a slice parameter.
fn slice_sum(s: &[i32]) -> i32 {
    let mut total = 0;
    let mut i = 0;
    while i < s.len() {
        total += s[i];
        i += 1;
    }
    total
}

// Return the first element of a slice.
fn slice_first(s: &[i32]) -> i32 {
    s[0]
}

// Pass a fixed array as a slice argument.
fn main() -> i32 {
    let arr = [10, 20, 30];
    let n = slice_len(&arr);
    let total = slice_sum(&arr);
    let first = slice_first(&arr);
    // n == 3, total == 60, first == 10
    // return 0 if all correct
    if n == 3 { if total == 60 { if first == 10 { 0 } else { 3 } } else { 2 } } else { 1 }
}
