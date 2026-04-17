    .text

    // fn borrow_immutable — FLS §9
    .global borrow_immutable
borrow_immutable:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    add     x0, sp, #0                   // FLS §6.5.1: address of stack slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x2, [x1]           // FLS §6.5.2: deref pointer in x1
    mov     x3, #1                   // FLS §2.4.4.1: load imm 1
    add     x4, x2, x3          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn borrow_mutable — FLS §9
    .global borrow_mutable
borrow_mutable:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    add     x0, sp, #0                   // FLS §6.5.1: address of stack slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x2, [x1]           // FLS §6.5.2: deref pointer in x1
    mov     x3, #10                  // FLS §2.4.4.1: load imm 10
    add     x4, x2, x3          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x4, [x1]           // FLS §6.5.10: store through pointer in x1
    ldr     x5, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x5              // FLS §6.19: return reg 5 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn deref_ref — FLS §9
    .global deref_ref
deref_ref:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [x0]           // FLS §6.5.2: deref pointer in x0
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    add     x3, x1, x2          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn deref_mut_ref — FLS §9
    .global deref_mut_ref
deref_mut_ref:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x2, [x1]           // FLS §6.5.2: deref pointer in x1
    mov     x3, #2                   // FLS §2.4.4.1: load imm 2
    mul     x4, x2, x3          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    str     x4, [x0]           // FLS §6.5.10: store through pointer in x0
    ldr     x5, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x6, [x5]           // FLS §6.5.2: deref pointer in x5
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn negate_i32 — FLS §9
    .global negate_i32
negate_i32:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    neg     x1, x0               // FLS §6.5.4: negate x0
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn negate_bool — FLS §9
    .global negate_bool
negate_bool:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    eor     x1, x0, #1             // FLS §6.5.4: logical NOT x0 (bool)
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn arithmetic — FLS §9
    .global arithmetic
arithmetic:
    sub     sp, sp, #64             // FLS §8.1: frame for 7 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x4, [sp, #8              ] // FLS §8.1: load slot 1
    sub     x5, x3, x4          // FLS §6.5.5: sub; §6.23: 64-bit, no i32 wrap
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x6, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x7, [sp, #8              ] // FLS §8.1: load slot 1
    mul     x8, x6, x7          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    str     x8, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x9, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x10, [sp, #8              ] // FLS §8.1: load slot 1
    cbz     x10, _galvanic_panic         // FLS §6.23: div-by-zero guard
    movz    x9, #0x8000, lsl #16          // FLS §6.23: x9 = 0x0000_0000_8000_0000
    sxtw    x9, w9                        // FLS §6.23: x9 = 0xFFFF_FFFF_8000_0000 (i32::MIN)
    cmp     x9, x9                    // FLS §6.23: is lhs == i32::MIN?
    b.ne    .Lsdiv_ok_arithmetic_0       // FLS §6.23: lhs ≠ MIN → safe
    cmn     x10, #1                    // FLS §6.23: is rhs == -1? (rhs+1==0)
    b.ne    .Lsdiv_ok_arithmetic_0       // FLS §6.23: rhs ≠ -1 → safe
    b       _galvanic_panic                // FLS §6.23: MIN/-1 overflow → panic
.Lsdiv_ok_arithmetic_0:
    sdiv    x11, x9, x10          // FLS §6.5.5: div (signed)
    str     x11, [sp, #40             ] // FLS §8.1: store slot 5
    ldr     x12, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x13, [sp, #8              ] // FLS §8.1: load slot 1
    cbz     x13, _galvanic_panic         // FLS §6.23: rem-by-zero guard
    movz    x9, #0x8000, lsl #16          // FLS §6.23: x9 = 0x0000_0000_8000_0000
    sxtw    x9, w9                        // FLS §6.23: x9 = 0xFFFF_FFFF_8000_0000 (i32::MIN)
    cmp     x12, x9                    // FLS §6.23: is lhs == i32::MIN?
    b.ne    .Lsrem_ok_arithmetic_1       // FLS §6.23: lhs ≠ MIN → safe
    cmn     x13, #1                    // FLS §6.23: is rhs == -1? (rhs+1==0)
    b.ne    .Lsrem_ok_arithmetic_1       // FLS §6.23: rhs ≠ -1 → safe
    b       _galvanic_panic                // FLS §6.23: MIN/-1 remainder → panic
.Lsrem_ok_arithmetic_1:
    sdiv    x14, x12, x13          // FLS §6.5.5: rem step 1: quotient
    msub    x14, x14, x13, x12  // FLS §6.5.5: rem step 2: lhs - q*rhs
    str     x14, [sp, #48             ] // FLS §8.1: store slot 6
    ldr     x15, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x16, [sp, #24             ] // FLS §8.1: load slot 3
    add     x17, x15, x16          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x18, [sp, #32             ] // FLS §8.1: load slot 4
    add     x19, x17, x18          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x20, [sp, #40             ] // FLS §8.1: load slot 5
    add     x21, x19, x20          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x22, [sp, #48             ] // FLS §8.1: load slot 6
    add     x23, x21, x22          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x23              // FLS §6.19: return reg 23 → x0
    add     sp, sp, #64             // FLS §8.1: restore stack frame
    ret

    // fn bitwise — FLS §9
    .global bitwise
bitwise:
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    and     x2, x0, x1          // FLS §6.5.6: bitwise and
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x4, [sp, #8              ] // FLS §8.1: load slot 1
    orr     x5, x3, x4          // FLS §6.5.6: bitwise or
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x6, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x7, [sp, #8              ] // FLS §8.1: load slot 1
    eor     x8, x6, x7          // FLS §6.5.6: bitwise xor
    str     x8, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x10, [sp, #24             ] // FLS §8.1: load slot 3
    add     x11, x9, x10          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x12, [sp, #32             ] // FLS §8.1: load slot 4
    add     x13, x11, x12          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x13              // FLS §6.19: return reg 13 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn shifts — FLS §9
    .global shifts
shifts:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    cmp     x1, #64                    // FLS §6.5.9: shift amount must be < 64
    b.hs    _galvanic_panic                // FLS §6.5.9: panic if shift >= 64 or negative
    lsl     x2, x0, x1          // FLS §6.5.7: shift left
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x4, [sp, #8              ] // FLS §8.1: load slot 1
    cmp     x4, #64                    // FLS §6.5.9: shift amount must be < 64
    b.hs    _galvanic_panic                // FLS §6.5.9: panic if shift >= 64 or negative
    asr     x5, x3, x4          // FLS §6.5.7: arithmetic shift right (signed)
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x7, [sp, #24             ] // FLS §8.1: load slot 3
    add     x8, x6, x7          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x8              // FLS §6.19: return reg 8 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn comparisons — FLS §9
    .global comparisons
comparisons:
    sub     sp, sp, #112            // FLS §8.1: frame for 14 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, eq                    // FLS §6.5.3: x2 = (x0 == x1)
    cbz     x2, .L0                     // FLS §6.17: branch if false
    mov     x3, #1                   // FLS §2.4.4.1: load imm 1
    str     x3, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L1                        // FLS §6.17: branch to end
.L0:                              // FLS §6.17: branch target
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    str     x4, [sp, #24             ] // FLS §8.1: store slot 3
.L1:                              // FLS §6.17: branch target
    ldr     x5, [sp, #24             ] // FLS §8.1: load slot 3
    str     x5, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x6, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x7, [sp, #8              ] // FLS §8.1: load slot 1
    cmp     x6, x7               // FLS §6.5.3: compare (signed)
    cset    x8, ne                    // FLS §6.5.3: x8 = (x6 != x7)
    cbz     x8, .L2                     // FLS §6.17: branch if false
    mov     x9, #1                   // FLS §2.4.4.1: load imm 1
    str     x9, [sp, #40             ] // FLS §8.1: store slot 5
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    mov     x10, #0                   // FLS §2.4.4.1: load imm 0
    str     x10, [sp, #40             ] // FLS §8.1: store slot 5
.L3:                              // FLS §6.17: branch target
    ldr     x11, [sp, #40             ] // FLS §8.1: load slot 5
    str     x11, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x12, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x13, [sp, #8              ] // FLS §8.1: load slot 1
    cmp     x12, x13               // FLS §6.5.3: compare (signed)
    cset    x14, lt                    // FLS §6.5.3: x14 = (x12 < x13)
    cbz     x14, .L4                     // FLS §6.17: branch if false
    mov     x15, #1                   // FLS §2.4.4.1: load imm 1
    str     x15, [sp, #56             ] // FLS §8.1: store slot 7
    b       .L5                        // FLS §6.17: branch to end
.L4:                              // FLS §6.17: branch target
    mov     x16, #0                   // FLS §2.4.4.1: load imm 0
    str     x16, [sp, #56             ] // FLS §8.1: store slot 7
.L5:                              // FLS §6.17: branch target
    ldr     x17, [sp, #56             ] // FLS §8.1: load slot 7
    str     x17, [sp, #48             ] // FLS §8.1: store slot 6
    ldr     x18, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x19, [sp, #8              ] // FLS §8.1: load slot 1
    cmp     x18, x19               // FLS §6.5.3: compare (signed)
    cset    x20, gt                    // FLS §6.5.3: x20 = (x18 > x19)
    cbz     x20, .L6                     // FLS §6.17: branch if false
    mov     x21, #1                   // FLS §2.4.4.1: load imm 1
    str     x21, [sp, #72             ] // FLS §8.1: store slot 9
    b       .L7                        // FLS §6.17: branch to end
.L6:                              // FLS §6.17: branch target
    mov     x22, #0                   // FLS §2.4.4.1: load imm 0
    str     x22, [sp, #72             ] // FLS §8.1: store slot 9
.L7:                              // FLS §6.17: branch target
    ldr     x23, [sp, #72             ] // FLS §8.1: load slot 9
    str     x23, [sp, #64             ] // FLS §8.1: store slot 8
    ldr     x24, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x25, [sp, #8              ] // FLS §8.1: load slot 1
    cmp     x24, x25               // FLS §6.5.3: compare (signed)
    cset    x26, le                    // FLS §6.5.3: x26 = (x24 <= x25)
    cbz     x26, .L8                     // FLS §6.17: branch if false
    mov     x27, #1                   // FLS §2.4.4.1: load imm 1
    str     x27, [sp, #88             ] // FLS §8.1: store slot 11
    b       .L9                        // FLS §6.17: branch to end
.L8:                              // FLS §6.17: branch target
    mov     x28, #0                   // FLS §2.4.4.1: load imm 0
    str     x28, [sp, #88             ] // FLS §8.1: store slot 11
.L9:                              // FLS §6.17: branch target
    ldr     x29, [sp, #88             ] // FLS §8.1: load slot 11
    str     x29, [sp, #80             ] // FLS §8.1: store slot 10
    ldr     x30, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x31, [sp, #8              ] // FLS §8.1: load slot 1
    cmp     x30, x31               // FLS §6.5.3: compare (signed)
    cset    x32, ge                    // FLS §6.5.3: x32 = (x30 >= x31)
    cbz     x32, .L10                    // FLS §6.17: branch if false
    mov     x33, #1                   // FLS §2.4.4.1: load imm 1
    str     x33, [sp, #104            ] // FLS §8.1: store slot 13
    b       .L11                       // FLS §6.17: branch to end
.L10:                              // FLS §6.17: branch target
    mov     x34, #0                   // FLS §2.4.4.1: load imm 0
    str     x34, [sp, #104            ] // FLS §8.1: store slot 13
.L11:                              // FLS §6.17: branch target
    ldr     x35, [sp, #104            ] // FLS §8.1: load slot 13
    str     x35, [sp, #96             ] // FLS §8.1: store slot 12
    ldr     x36, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x37, [sp, #32             ] // FLS §8.1: load slot 4
    add     x38, x36, x37          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x39, [sp, #48             ] // FLS §8.1: load slot 6
    add     x40, x38, x39          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x41, [sp, #64             ] // FLS §8.1: load slot 8
    add     x42, x40, x41          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x43, [sp, #80             ] // FLS §8.1: load slot 10
    add     x44, x42, x43          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x45, [sp, #96             ] // FLS §8.1: load slot 12
    add     x46, x44, x45          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x46              // FLS §6.19: return reg 46 → x0
    add     sp, sp, #112            // FLS §8.1: restore stack frame
    ret

    // fn lazy_boolean — FLS §9
    .global lazy_boolean
lazy_boolean:
    sub     sp, sp, #64             // FLS §8.1: frame for 8 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    cbz     x0, .L14                    // FLS §6.17: branch if false
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #32             ] // FLS §8.1: store slot 4
    b       .L15                       // FLS §6.17: branch to end
.L14:                              // FLS §6.17: branch target
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    str     x2, [sp, #32             ] // FLS §8.1: store slot 4
.L15:                              // FLS §6.17: branch target
    ldr     x3, [sp, #32             ] // FLS §8.1: load slot 4
    cbz     x3, .L12                    // FLS §6.17: branch if false
    mov     x4, #1                   // FLS §2.4.4.1: load imm 1
    str     x4, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L13                       // FLS §6.17: branch to end
.L12:                              // FLS §6.17: branch target
    mov     x5, #0                   // FLS §2.4.4.1: load imm 0
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
.L13:                              // FLS §6.17: branch target
    ldr     x6, [sp, #24             ] // FLS §8.1: load slot 3
    str     x6, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x7, [sp, #0              ] // FLS §8.1: load slot 0
    cbz     x7, .L18                    // FLS §6.17: branch if false
    str     x7, [sp, #56             ] // FLS §8.1: store slot 7
    b       .L19                       // FLS §6.17: branch to end
.L18:                              // FLS §6.17: branch target
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    str     x8, [sp, #56             ] // FLS §8.1: store slot 7
.L19:                              // FLS §6.17: branch target
    ldr     x9, [sp, #56             ] // FLS §8.1: load slot 7
    cbz     x9, .L16                    // FLS §6.17: branch if false
    mov     x10, #1                   // FLS §2.4.4.1: load imm 1
    str     x10, [sp, #48             ] // FLS §8.1: store slot 6
    b       .L17                       // FLS §6.17: branch to end
.L16:                              // FLS §6.17: branch target
    mov     x11, #0                   // FLS §2.4.4.1: load imm 0
    str     x11, [sp, #48             ] // FLS §8.1: store slot 6
.L17:                              // FLS §6.17: branch target
    ldr     x12, [sp, #48             ] // FLS §8.1: load slot 6
    str     x12, [sp, #40             ] // FLS §8.1: store slot 5
    ldr     x13, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x14, [sp, #40             ] // FLS §8.1: load slot 5
    add     x15, x13, x14          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x15              // FLS §6.19: return reg 15 → x0
    add     sp, sp, #64             // FLS §8.1: restore stack frame
    ret

    // fn type_cast — FLS §9
    .global type_cast
type_cast:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn bool_as_int — FLS §9
    .global bool_as_int
bool_as_int:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn assignment — FLS §9
    .global assignment
assignment:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #1                   // FLS §2.4.4.1: load imm 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x2, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn compound_assignment — FLS §9
    .global compound_assignment
compound_assignment:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #1                   // FLS §2.4.4.1: load imm 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x2, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x4, #1                   // FLS §2.4.4.1: load imm 1
    sub     x5, x3, x4          // FLS §6.5.5: sub; §6.23: 64-bit, no i32 wrap
    str     x5, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x6, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x7, #2                   // FLS §2.4.4.1: load imm 2
    mul     x8, x6, x7          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    str     x8, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x9, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x10, #2                   // FLS §2.4.4.1: load imm 2
    cbz     x10, _galvanic_panic         // FLS §6.23: div-by-zero guard
    movz    x9, #0x8000, lsl #16          // FLS §6.23: x9 = 0x0000_0000_8000_0000
    sxtw    x9, w9                        // FLS §6.23: x9 = 0xFFFF_FFFF_8000_0000 (i32::MIN)
    cmp     x9, x9                    // FLS §6.23: is lhs == i32::MIN?
    b.ne    .Lsdiv_ok_compound_assignment_0       // FLS §6.23: lhs ≠ MIN → safe
    cmn     x10, #1                    // FLS §6.23: is rhs == -1? (rhs+1==0)
    b.ne    .Lsdiv_ok_compound_assignment_0       // FLS §6.23: rhs ≠ -1 → safe
    b       _galvanic_panic                // FLS §6.23: MIN/-1 overflow → panic
.Lsdiv_ok_compound_assignment_0:
    sdiv    x11, x9, x10          // FLS §6.5.5: div (signed)
    str     x11, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x12, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x13, #3                   // FLS §2.4.4.1: load imm 3
    cbz     x13, _galvanic_panic         // FLS §6.23: rem-by-zero guard
    movz    x9, #0x8000, lsl #16          // FLS §6.23: x9 = 0x0000_0000_8000_0000
    sxtw    x9, w9                        // FLS §6.23: x9 = 0xFFFF_FFFF_8000_0000 (i32::MIN)
    cmp     x12, x9                    // FLS §6.23: is lhs == i32::MIN?
    b.ne    .Lsrem_ok_compound_assignment_1       // FLS §6.23: lhs ≠ MIN → safe
    cmn     x13, #1                    // FLS §6.23: is rhs == -1? (rhs+1==0)
    b.ne    .Lsrem_ok_compound_assignment_1       // FLS §6.23: rhs ≠ -1 → safe
    b       _galvanic_panic                // FLS §6.23: MIN/-1 remainder → panic
.Lsrem_ok_compound_assignment_1:
    sdiv    x14, x12, x13          // FLS §6.5.5: rem step 1: quotient
    msub    x14, x14, x13, x12  // FLS §6.5.5: rem step 2: lhs - q*rhs
    str     x14, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x15, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x16, #255                 // FLS §2.4.4.1: load imm 255
    and     x17, x15, x16          // FLS §6.5.6: bitwise and
    str     x17, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x18, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x19, #1                   // FLS §2.4.4.1: load imm 1
    orr     x20, x18, x19          // FLS §6.5.6: bitwise or
    str     x20, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x21, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x22, #1                   // FLS §2.4.4.1: load imm 1
    eor     x23, x21, x22          // FLS §6.5.6: bitwise xor
    str     x23, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x24, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x25, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x25, #64                    // FLS §6.5.9: shift amount must be < 64
    b.hs    _galvanic_panic                // FLS §6.5.9: panic if shift >= 64 or negative
    lsl     x26, x24, x25          // FLS §6.5.7: shift left
    str     x26, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x27, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x28, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x28, #64                    // FLS §6.5.9: shift amount must be < 64
    b.hs    _galvanic_panic                // FLS §6.5.9: panic if shift >= 64 or negative
    asr     x29, x27, x28          // FLS §6.5.7: arithmetic shift right (signed)
    str     x29, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x30, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x30              // FLS §6.19: return reg 30 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #5                   // FLS §2.4.4.1: load imm 5
    bl      borrow_immutable         // FLS §6.12.1: call borrow_immutable
    mov     x1, x0              // FLS §6.12.1: return value → x1
    mov     x2, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x2                   // FLS §6.12.1: arg 0
    bl      borrow_mutable           // FLS §6.12.1: call borrow_mutable
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x4, #4                   // FLS §2.4.4.1: load imm 4
    str     x4, [sp, #0              ] // FLS §8.1: store slot 0
    add     x5, sp, #0                   // FLS §6.5.1: address of stack slot 0
    mov     x0, x5                   // FLS §6.12.1: arg 0
    bl      deref_ref                // FLS §6.12.1: call deref_ref
    mov     x6, x0              // FLS §6.12.1: return value → x6
    add     x7, sp, #0                   // FLS §6.5.1: address of stack slot 0
    mov     x0, x7                   // FLS §6.12.1: arg 0
    bl      deref_mut_ref            // FLS §6.12.1: call deref_mut_ref
    mov     x8, x0              // FLS §6.12.1: return value → x8
    mov     x9, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x9                   // FLS §6.12.1: arg 0
    bl      negate_i32               // FLS §6.12.1: call negate_i32
    mov     x10, x0              // FLS §6.12.1: return value → x10
    mov     x11, #3                   // FLS §2.4.4.1: load imm 3
    neg     x12, x11               // FLS §6.5.4: negate x11
    mov     x0, x12                  // FLS §6.12.1: arg 0
    bl      negate_i32               // FLS §6.12.1: call negate_i32
    mov     x13, x0              // FLS §6.12.1: return value → x13
    mov     x14, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x14                  // FLS §6.12.1: arg 0
    bl      negate_bool              // FLS §6.12.1: call negate_bool
    mov     x15, x0              // FLS §6.12.1: return value → x15
    mov     x16, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x16                  // FLS §6.12.1: arg 0
    bl      negate_bool              // FLS §6.12.1: call negate_bool
    mov     x17, x0              // FLS §6.12.1: return value → x17
    mov     x18, #10                  // FLS §2.4.4.1: load imm 10
    mov     x19, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x18                  // FLS §6.12.1: arg 0
    mov     x1, x19                  // FLS §6.12.1: arg 1
    bl      arithmetic               // FLS §6.12.1: call arithmetic
    mov     x20, x0              // FLS §6.12.1: return value → x20
    mov     x21, #10                  // FLS §2.4.4.1: load imm 10
    mov     x22, #12                  // FLS §2.4.4.1: load imm 12
    mov     x0, x21                  // FLS §6.12.1: arg 0
    mov     x1, x22                  // FLS §6.12.1: arg 1
    bl      bitwise                  // FLS §6.12.1: call bitwise
    mov     x23, x0              // FLS §6.12.1: return value → x23
    mov     x24, #4                   // FLS §2.4.4.1: load imm 4
    mov     x25, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x24                  // FLS §6.12.1: arg 0
    mov     x1, x25                  // FLS §6.12.1: arg 1
    bl      shifts                   // FLS §6.12.1: call shifts
    mov     x26, x0              // FLS §6.12.1: return value → x26
    mov     x27, #5                   // FLS §2.4.4.1: load imm 5
    mov     x28, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x27                  // FLS §6.12.1: arg 0
    mov     x1, x28                  // FLS §6.12.1: arg 1
    bl      comparisons              // FLS §6.12.1: call comparisons
    mov     x29, x0              // FLS §6.12.1: return value → x29
    mov     x30, #3                   // FLS §2.4.4.1: load imm 3
    mov     x31, #7                   // FLS §2.4.4.1: load imm 7
    mov     x0, x30                  // FLS §6.12.1: arg 0
    mov     x1, x31                  // FLS §6.12.1: arg 1
    bl      comparisons              // FLS §6.12.1: call comparisons
    mov     x32, x0              // FLS §6.12.1: return value → x32
    mov     x33, #1                   // FLS §2.4.4.1: load imm 1
    mov     x34, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x33                  // FLS §6.12.1: arg 0
    mov     x1, x34                  // FLS §6.12.1: arg 1
    bl      lazy_boolean             // FLS §6.12.1: call lazy_boolean
    mov     x35, x0              // FLS §6.12.1: return value → x35
    mov     x36, #0                   // FLS §2.4.4.1: load imm 0
    mov     x37, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x36                  // FLS §6.12.1: arg 0
    mov     x1, x37                  // FLS §6.12.1: arg 1
    bl      lazy_boolean             // FLS §6.12.1: call lazy_boolean
    mov     x38, x0              // FLS §6.12.1: return value → x38
    mov     x39, #42                  // FLS §2.4.4.1: load imm 42
    mov     x0, x39                  // FLS §6.12.1: arg 0
    bl      type_cast                // FLS §6.12.1: call type_cast
    mov     x40, x0              // FLS §6.12.1: return value → x40
    mov     x41, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x41                  // FLS §6.12.1: arg 0
    bl      bool_as_int              // FLS §6.12.1: call bool_as_int
    mov     x42, x0              // FLS §6.12.1: return value → x42
    mov     x43, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x43                  // FLS §6.12.1: arg 0
    bl      bool_as_int              // FLS §6.12.1: call bool_as_int
    mov     x44, x0              // FLS §6.12.1: return value → x44
    mov     x45, #10                  // FLS §2.4.4.1: load imm 10
    mov     x0, x45                  // FLS §6.12.1: arg 0
    bl      assignment               // FLS §6.12.1: call assignment
    mov     x46, x0              // FLS §6.12.1: return value → x46
    mov     x47, #10                  // FLS §2.4.4.1: load imm 10
    mov     x0, x47                  // FLS §6.12.1: arg 0
    bl      compound_assignment      // FLS §6.12.1: call compound_assignment
    mov     x48, x0              // FLS §6.12.1: return value → x48
    mov     x0, #0              // FLS §4.4: unit return
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)

    // FLS §6.23: runtime panic primitive — exit(101)
    .global _galvanic_panic
_galvanic_panic:
    mov     x0, #101        // panic exit code (galvanic sentinel)
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(101) — FLS §6.23: panic
