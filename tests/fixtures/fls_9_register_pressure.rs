// FLS §9: Function with many sequential let-bindings.
//
// This fixture exercises galvanic's register allocator by creating a function
// body whose lowering produces more than 30 virtual registers. Without register
// allocation, galvanic would emit architecturally invalid ARM64 assembly such as
// `add x198, x196, x197` (x32+ do not exist on ARM64). The allocator must map
// all virtual registers to the 12-register physical pool (x0–x8, x10, x11, x15).
//
// The 40 sequential let-bindings each generate two IR instructions:
//   LoadImm(VR_N, value) + Store(VR_N, slot_N)
// giving virtual register indices well above 30. Peak simultaneous live VRs
// is ≤ 2 (one LoadImm + one Store at a time), so no spilling is needed.
fn main() -> i32 {
    let a0: i32 = 1;
    let a1: i32 = 2;
    let a2: i32 = 3;
    let a3: i32 = 4;
    let a4: i32 = 5;
    let a5: i32 = 6;
    let a6: i32 = 7;
    let a7: i32 = 8;
    let a8: i32 = 9;
    let a9: i32 = 10;
    let a10: i32 = 11;
    let a11: i32 = 12;
    let a12: i32 = 13;
    let a13: i32 = 14;
    let a14: i32 = 15;
    let a15: i32 = 16;
    let a16: i32 = 17;
    let a17: i32 = 18;
    let a18: i32 = 19;
    let a19: i32 = 20;
    let a20: i32 = 21;
    let a21: i32 = 22;
    let a22: i32 = 23;
    let a23: i32 = 24;
    let a24: i32 = 25;
    let a25: i32 = 26;
    let a26: i32 = 27;
    let a27: i32 = 28;
    let a28: i32 = 29;
    let a29: i32 = 30;
    let a30: i32 = 31;
    let a31: i32 = 32;
    let a32: i32 = 33;
    let a33: i32 = 34;
    let a34: i32 = 35;
    let a35: i32 = 36;
    let a36: i32 = 37;
    let a37: i32 = 38;
    let a38: i32 = 39;
    let a39: i32 = 40;
    a0 + a1 + a2 + a3 + a4 + a5 + a6 + a7 + a8 + a9
        + a10 + a11 + a12 + a13 + a14 + a15 + a16 + a17 + a18 + a19
        + a20 + a21 + a22 + a23 + a24 + a25 + a26 + a27 + a28 + a29
        + a30 + a31 + a32 + a33 + a34 + a35 + a36 + a37 + a38 + a39
}
