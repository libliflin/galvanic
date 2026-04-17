    .text

    // fn for_exclusive_range — FLS §9
    .global for_exclusive_range
for_exclusive_range:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    str     x2, [sp, #24             ] // FLS §8.1: store slot 3
.L0:                              // FLS §6.17: branch target
    ldr     x3, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x4, [sp, #24             ] // FLS §8.1: load slot 3
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, lt                    // FLS §6.5.3: x5 = (x3 < x4)
    cbz     x5, .L2                     // FLS §6.17: branch if false
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x7, [sp, #16             ] // FLS §8.1: load slot 2
    add     x8, x6, x7          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x8, [sp, #8              ] // FLS §8.1: store slot 1
.L1:                              // FLS §6.17: branch target
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x10, #1                   // FLS §2.4.4.1: load imm 1
    add     x11, x9, x10          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x11, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn for_inclusive_range — FLS §9
    .global for_inclusive_range
for_inclusive_range:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    str     x2, [sp, #24             ] // FLS §8.1: store slot 3
.L3:                              // FLS §6.17: branch target
    ldr     x3, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x4, [sp, #24             ] // FLS §8.1: load slot 3
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, le                    // FLS §6.5.3: x5 = (x3 <= x4)
    cbz     x5, .L5                     // FLS §6.17: branch if false
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x7, [sp, #16             ] // FLS §8.1: load slot 2
    add     x8, x6, x7          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x8, [sp, #8              ] // FLS §8.1: store slot 1
.L4:                              // FLS §6.17: branch target
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x10, #1                   // FLS §2.4.4.1: load imm 1
    add     x11, x9, x10          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x11, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L3                        // FLS §6.17: branch to end
.L5:                              // FLS §6.17: branch target
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn classify_exclusive — FLS §9
    .global classify_exclusive
classify_exclusive:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, ge                    // FLS §6.5.3: x3 = (x1 >= x2)
    mov     x4, #5                   // FLS §2.4.4.1: load imm 5
    cmp     x1, x4               // FLS §6.5.3: compare (signed)
    cset    x5, lt                    // FLS §6.5.3: x5 = (x1 < x4)
    and     x6, x3, x5          // FLS §6.5.6: bitwise and
    cbz     x6, .L7                     // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L6                        // FLS §6.17: branch to end
.L7:                              // FLS §6.17: branch target
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x9, #5                   // FLS §2.4.4.1: load imm 5
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, ge                    // FLS §6.5.3: x10 = (x8 >= x9)
    mov     x11, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x8, x11               // FLS §6.5.3: compare (signed)
    cset    x12, lt                    // FLS §6.5.3: x12 = (x8 < x11)
    and     x13, x10, x12          // FLS §6.5.6: bitwise and
    cbz     x13, .L8                     // FLS §6.17: branch if false
    mov     x14, #2                   // FLS §2.4.4.1: load imm 2
    str     x14, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L6                        // FLS §6.17: branch to end
.L8:                              // FLS §6.17: branch target
    mov     x15, #0                   // FLS §2.4.4.1: load imm 0
    str     x15, [sp, #16             ] // FLS §8.1: store slot 2
.L6:                              // FLS §6.17: branch target
    ldr     x16, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x16              // FLS §6.19: return reg 16 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn classify_inclusive — FLS §9
    .global classify_inclusive
classify_inclusive:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, ge                    // FLS §6.5.3: x3 = (x1 >= x2)
    mov     x4, #5                   // FLS §2.4.4.1: load imm 5
    cmp     x1, x4               // FLS §6.5.3: compare (signed)
    cset    x5, le                    // FLS §6.5.3: x5 = (x1 <= x4)
    and     x6, x3, x5          // FLS §6.5.6: bitwise and
    cbz     x6, .L10                    // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L9                        // FLS §6.17: branch to end
.L10:                              // FLS §6.17: branch target
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x9, #6                   // FLS §2.4.4.1: load imm 6
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, ge                    // FLS §6.5.3: x10 = (x8 >= x9)
    mov     x11, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x8, x11               // FLS §6.5.3: compare (signed)
    cset    x12, le                    // FLS §6.5.3: x12 = (x8 <= x11)
    and     x13, x10, x12          // FLS §6.5.6: bitwise and
    cbz     x13, .L11                    // FLS §6.17: branch if false
    mov     x14, #2                   // FLS §2.4.4.1: load imm 2
    str     x14, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L9                        // FLS §6.17: branch to end
.L11:                              // FLS §6.17: branch target
    mov     x15, #0                   // FLS §2.4.4.1: load imm 0
    str     x15, [sp, #16             ] // FLS §8.1: store slot 2
.L9:                              // FLS §6.17: branch target
    ldr     x16, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x16              // FLS §6.19: return reg 16 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn negative_range_pattern — FLS §9
    .global negative_range_pattern
negative_range_pattern:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #-10                 // FLS §2.4.4.1: load imm -10
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, ge                    // FLS §6.5.3: x3 = (x1 >= x2)
    mov     x4, #-1                  // FLS §2.4.4.1: load imm -1
    cmp     x1, x4               // FLS §6.5.3: compare (signed)
    cset    x5, le                    // FLS §6.5.3: x5 = (x1 <= x4)
    and     x6, x3, x5          // FLS §6.5.6: bitwise and
    cbz     x6, .L13                    // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L12                       // FLS §6.17: branch to end
.L13:                              // FLS §6.17: branch target
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x9, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, eq                    // FLS §6.5.3: x10 = (x8 == x9)
    cbz     x10, .L14                    // FLS §6.17: branch if false
    mov     x11, #2                   // FLS §2.4.4.1: load imm 2
    str     x11, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L12                       // FLS §6.17: branch to end
.L14:                              // FLS §6.17: branch target
    ldr     x12, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x13, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x12, x13               // FLS §6.5.3: compare (signed)
    cset    x14, ge                    // FLS §6.5.3: x14 = (x12 >= x13)
    mov     x15, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x12, x15               // FLS §6.5.3: compare (signed)
    cset    x16, le                    // FLS §6.5.3: x16 = (x12 <= x15)
    and     x17, x14, x16          // FLS §6.5.6: bitwise and
    cbz     x17, .L15                    // FLS §6.17: branch if false
    mov     x18, #3                   // FLS §2.4.4.1: load imm 3
    str     x18, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L12                       // FLS §6.17: branch to end
.L15:                              // FLS §6.17: branch target
    mov     x19, #0                   // FLS §2.4.4.1: load imm 0
    str     x19, [sp, #16             ] // FLS §8.1: store slot 2
.L12:                              // FLS §6.17: branch target
    ldr     x20, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x20              // FLS §6.19: return reg 20 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn if_let_range — FLS §9
    .global if_let_range
if_let_range:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, ge                    // FLS §6.5.3: x3 = (x1 >= x2)
    mov     x4, #100                 // FLS §2.4.4.1: load imm 100
    cmp     x1, x4               // FLS §6.5.3: compare (signed)
    cset    x5, le                    // FLS §6.5.3: x5 = (x1 <= x4)
    and     x6, x3, x5          // FLS §6.5.6: bitwise and
    cbz     x6, .L16                    // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L17                       // FLS §6.17: branch to end
.L16:                              // FLS §6.17: branch target
    mov     x8, #0                   // FLS §2.4.4.1: load imm 0
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
.L17:                              // FLS §6.17: branch target
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x9              // FLS §6.19: return reg 9 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn if_let_exclusive_range — FLS §9
    .global if_let_exclusive_range
if_let_exclusive_range:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, ge                    // FLS §6.5.3: x3 = (x1 >= x2)
    mov     x4, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x1, x4               // FLS §6.5.3: compare (signed)
    cset    x5, lt                    // FLS §6.5.3: x5 = (x1 < x4)
    and     x6, x3, x5          // FLS §6.5.6: bitwise and
    cbz     x6, .L18                    // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L19                       // FLS §6.17: branch to end
.L18:                              // FLS §6.17: branch target
    mov     x8, #0                   // FLS §2.4.4.1: load imm 0
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
.L19:                              // FLS §6.17: branch target
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x9              // FLS §6.19: return reg 9 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn for_range_nonzero_start — FLS §9
    .global for_range_nonzero_start
for_range_nonzero_start:
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x2, [sp, #8              ] // FLS §8.1: load slot 1
    str     x2, [sp, #32             ] // FLS §8.1: store slot 4
.L20:                              // FLS §6.17: branch target
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    ldr     x4, [sp, #32             ] // FLS §8.1: load slot 4
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, lt                    // FLS §6.5.3: x5 = (x3 < x4)
    cbz     x5, .L22                    // FLS §6.17: branch if false
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x7, [sp, #24             ] // FLS §8.1: load slot 3
    add     x8, x6, x7          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
.L21:                              // FLS §6.17: branch target
    ldr     x9, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x10, #1                   // FLS §2.4.4.1: load imm 1
    add     x11, x9, x10          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x11, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L20                       // FLS §6.17: branch to end
.L22:                              // FLS §6.17: branch target
    ldr     x1, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn for_range_nonzero_start_inclusive — FLS §9
    .global for_range_nonzero_start_inclusive
for_range_nonzero_start_inclusive:
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x2, [sp, #8              ] // FLS §8.1: load slot 1
    str     x2, [sp, #32             ] // FLS §8.1: store slot 4
.L23:                              // FLS §6.17: branch target
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    ldr     x4, [sp, #32             ] // FLS §8.1: load slot 4
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, le                    // FLS §6.5.3: x5 = (x3 <= x4)
    cbz     x5, .L25                    // FLS §6.17: branch if false
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x7, [sp, #24             ] // FLS §8.1: load slot 3
    add     x8, x6, x7          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
.L24:                              // FLS §6.17: branch target
    ldr     x9, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x10, #1                   // FLS §2.4.4.1: load imm 1
    add     x11, x9, x10          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x11, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L23                       // FLS §6.17: branch to end
.L25:                              // FLS §6.17: branch target
    ldr     x1, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn while_let_range_counts — FLS §9
    .global while_let_range_counts
while_let_range_counts:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
.L26:                              // FLS §6.17: branch target
    ldr     x2, [sp, #8              ] // FLS §8.1: load slot 1
    str     x2, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x4, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, ge                    // FLS §6.5.3: x5 = (x3 >= x4)
    mov     x6, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x3, x6               // FLS §6.5.3: compare (signed)
    cset    x7, le                    // FLS §6.5.3: x7 = (x3 <= x6)
    and     x8, x5, x7          // FLS §6.5.6: bitwise and
    cbz     x8, .L27                    // FLS §6.17: branch if false
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x10, #1                   // FLS §2.4.4.1: load imm 1
    add     x11, x9, x10          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x11, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x12, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x13, #1                   // FLS §2.4.4.1: load imm 1
    sub     x14, x12, x13          // FLS §6.5.5: sub; §6.23: 64-bit, no i32 wrap
    str     x14, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L26                       // FLS §6.17: branch to end
.L27:                              // FLS §6.17: branch target
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn mixed_range_patterns — FLS §9
    .global mixed_range_patterns
mixed_range_patterns:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, ge                    // FLS §6.5.3: x3 = (x1 >= x2)
    mov     x4, #5                   // FLS §2.4.4.1: load imm 5
    cmp     x1, x4               // FLS §6.5.3: compare (signed)
    cset    x5, lt                    // FLS §6.5.3: x5 = (x1 < x4)
    and     x6, x3, x5          // FLS §6.5.6: bitwise and
    cbz     x6, .L29                    // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L28                       // FLS §6.17: branch to end
.L29:                              // FLS §6.17: branch target
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x9, #5                   // FLS §2.4.4.1: load imm 5
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, ge                    // FLS §6.5.3: x10 = (x8 >= x9)
    mov     x11, #9                   // FLS §2.4.4.1: load imm 9
    cmp     x8, x11               // FLS §6.5.3: compare (signed)
    cset    x12, le                    // FLS §6.5.3: x12 = (x8 <= x11)
    and     x13, x10, x12          // FLS §6.5.6: bitwise and
    cbz     x13, .L30                    // FLS §6.17: branch if false
    mov     x14, #2                   // FLS §2.4.4.1: load imm 2
    str     x14, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L28                       // FLS §6.17: branch to end
.L30:                              // FLS §6.17: branch target
    ldr     x15, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x16, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x15, x16               // FLS §6.5.3: compare (signed)
    cset    x17, ge                    // FLS §6.5.3: x17 = (x15 >= x16)
    mov     x18, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x15, x18               // FLS §6.5.3: compare (signed)
    cset    x19, le                    // FLS §6.5.3: x19 = (x15 <= x18)
    and     x20, x17, x19          // FLS §6.5.6: bitwise and
    cbz     x20, .L31                    // FLS §6.17: branch if false
    mov     x21, #3                   // FLS §2.4.4.1: load imm 3
    str     x21, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L28                       // FLS §6.17: branch to end
.L31:                              // FLS §6.17: branch target
    mov     x22, #0                   // FLS §2.4.4.1: load imm 0
    str     x22, [sp, #16             ] // FLS §8.1: store slot 2
.L28:                              // FLS §6.17: branch target
    ldr     x23, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x23              // FLS §6.19: return reg 23 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    mov     x0, #5                   // FLS §2.4.4.1: load imm 5
    bl      for_exclusive_range      // FLS §6.12.1: call for_exclusive_range
    mov     x1, x0              // FLS §6.12.1: return value → x1
    mov     x2, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x2                   // FLS §6.12.1: arg 0
    bl      for_inclusive_range      // FLS §6.12.1: call for_inclusive_range
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x4, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x4                   // FLS §6.12.1: arg 0
    bl      classify_exclusive       // FLS §6.12.1: call classify_exclusive
    mov     x5, x0              // FLS §6.12.1: return value → x5
    mov     x6, #7                   // FLS §2.4.4.1: load imm 7
    mov     x0, x6                   // FLS §6.12.1: arg 0
    bl      classify_inclusive       // FLS §6.12.1: call classify_inclusive
    mov     x7, x0              // FLS §6.12.1: return value → x7
    mov     x8, #5                   // FLS §2.4.4.1: load imm 5
    neg     x9, x8               // FLS §6.5.4: negate x8
    mov     x0, x9                   // FLS §6.12.1: arg 0
    bl      negative_range_pattern   // FLS §6.12.1: call negative_range_pattern
    mov     x10, x0              // FLS §6.12.1: return value → x10
    mov     x11, #50                  // FLS §2.4.4.1: load imm 50
    mov     x0, x11                  // FLS §6.12.1: arg 0
    bl      if_let_range             // FLS §6.12.1: call if_let_range
    mov     x12, x0              // FLS §6.12.1: return value → x12
    mov     x13, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x13                  // FLS §6.12.1: arg 0
    bl      if_let_exclusive_range   // FLS §6.12.1: call if_let_exclusive_range
    mov     x14, x0              // FLS §6.12.1: return value → x14
    mov     x15, #2                   // FLS §2.4.4.1: load imm 2
    mov     x16, #6                   // FLS §2.4.4.1: load imm 6
    mov     x0, x15                  // FLS §6.12.1: arg 0
    mov     x1, x16                  // FLS §6.12.1: arg 1
    bl      for_range_nonzero_start  // FLS §6.12.1: call for_range_nonzero_start
    mov     x17, x0              // FLS §6.12.1: return value → x17
    mov     x18, #2                   // FLS §2.4.4.1: load imm 2
    mov     x19, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x18                  // FLS §6.12.1: arg 0
    mov     x1, x19                  // FLS §6.12.1: arg 1
    bl      for_range_nonzero_start_inclusive // FLS §6.12.1: call for_range_nonzero_start_inclusive
    mov     x20, x0              // FLS §6.12.1: return value → x20
    mov     x21, #8                   // FLS §2.4.4.1: load imm 8
    mov     x0, x21                  // FLS §6.12.1: arg 0
    bl      while_let_range_counts   // FLS §6.12.1: call while_let_range_counts
    mov     x22, x0              // FLS §6.12.1: return value → x22
    mov     x23, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x23                  // FLS §6.12.1: arg 0
    bl      mixed_range_patterns     // FLS §6.12.1: call mixed_range_patterns
    mov     x24, x0              // FLS §6.12.1: return value → x24
    mov     x0, #0              // FLS §4.4: unit return
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
