    .text

    // fn range_inclusive — FLS §9
    .global range_inclusive
range_inclusive:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, ge                    // FLS §6.5.3: x3 = (x1 >= x2)
    mov     x4, #3                   // FLS §2.4.4.1: load imm 3
    cmp     x1, x4               // FLS §6.5.3: compare (signed)
    cset    x5, le                    // FLS §6.5.3: x5 = (x1 <= x4)
    and     x6, x3, x5          // FLS §6.5.6: bitwise and
    cbz     x6, .L1                     // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x9, #4                   // FLS §2.4.4.1: load imm 4
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, ge                    // FLS §6.5.3: x10 = (x8 >= x9)
    mov     x11, #6                   // FLS §2.4.4.1: load imm 6
    cmp     x8, x11               // FLS §6.5.3: compare (signed)
    cset    x12, le                    // FLS §6.5.3: x12 = (x8 <= x11)
    and     x13, x10, x12          // FLS §6.5.6: bitwise and
    cbz     x13, .L2                     // FLS §6.17: branch if false
    mov     x14, #2                   // FLS §2.4.4.1: load imm 2
    str     x14, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    mov     x15, #0                   // FLS §2.4.4.1: load imm 0
    str     x15, [sp, #16             ] // FLS §8.1: store slot 2
.L0:                              // FLS §6.17: branch target
    ldr     x16, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x16              // FLS §6.19: return reg 16 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn range_exclusive — FLS §9
    .global range_exclusive
range_exclusive:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, ge                    // FLS §6.5.3: x3 = (x1 >= x2)
    mov     x4, #4                   // FLS §2.4.4.1: load imm 4
    cmp     x1, x4               // FLS §6.5.3: compare (signed)
    cset    x5, lt                    // FLS §6.5.3: x5 = (x1 < x4)
    and     x6, x3, x5          // FLS §6.5.6: bitwise and
    cbz     x6, .L4                     // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L3                        // FLS §6.17: branch to end
.L4:                              // FLS §6.17: branch target
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x9, #4                   // FLS §2.4.4.1: load imm 4
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, ge                    // FLS §6.5.3: x10 = (x8 >= x9)
    mov     x11, #7                   // FLS §2.4.4.1: load imm 7
    cmp     x8, x11               // FLS §6.5.3: compare (signed)
    cset    x12, lt                    // FLS §6.5.3: x12 = (x8 < x11)
    and     x13, x10, x12          // FLS §6.5.6: bitwise and
    cbz     x13, .L5                     // FLS §6.17: branch if false
    mov     x14, #2                   // FLS §2.4.4.1: load imm 2
    str     x14, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L3                        // FLS §6.17: branch to end
.L5:                              // FLS §6.17: branch target
    mov     x15, #0                   // FLS §2.4.4.1: load imm 0
    str     x15, [sp, #16             ] // FLS §8.1: store slot 2
.L3:                              // FLS §6.17: branch target
    ldr     x16, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x16              // FLS §6.19: return reg 16 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn range_negative — FLS §9
    .global range_negative
range_negative:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #-5                  // FLS §2.4.4.1: load imm -5
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, ge                    // FLS §6.5.3: x3 = (x1 >= x2)
    mov     x4, #-1                  // FLS §2.4.4.1: load imm -1
    cmp     x1, x4               // FLS §6.5.3: compare (signed)
    cset    x5, le                    // FLS §6.5.3: x5 = (x1 <= x4)
    and     x6, x3, x5          // FLS §6.5.6: bitwise and
    cbz     x6, .L7                     // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L6                        // FLS §6.17: branch to end
.L7:                              // FLS §6.17: branch target
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x9, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, eq                    // FLS §6.5.3: x10 = (x8 == x9)
    cbz     x10, .L8                     // FLS §6.17: branch if false
    mov     x11, #2                   // FLS §2.4.4.1: load imm 2
    str     x11, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L6                        // FLS §6.17: branch to end
.L8:                              // FLS §6.17: branch target
    mov     x12, #3                   // FLS §2.4.4.1: load imm 3
    str     x12, [sp, #16             ] // FLS §8.1: store slot 2
.L6:                              // FLS §6.17: branch target
    ldr     x13, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x13              // FLS §6.19: return reg 13 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn classify_with_guard — FLS §9
    .global classify_with_guard
classify_with_guard:
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x2, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x3, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x2, x3               // FLS §6.5.3: compare (signed)
    cset    x4, gt                    // FLS §6.5.3: x4 = (x2 > x3)
    cbz     x4, .L10                    // FLS §6.17: branch if false
    mov     x5, #1                   // FLS §2.4.4.1: load imm 1
    str     x5, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L9                        // FLS §6.17: branch to end
.L10:                              // FLS §6.17: branch target
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    str     x6, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x7, [sp, #32             ] // FLS §8.1: load slot 4
    mov     x8, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x7, x8               // FLS §6.5.3: compare (signed)
    cset    x9, lt                    // FLS §6.5.3: x9 = (x7 < x8)
    cbz     x9, .L11                    // FLS §6.17: branch if false
    mov     x10, #2                   // FLS §2.4.4.1: load imm 2
    str     x10, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L9                        // FLS §6.17: branch to end
.L11:                              // FLS §6.17: branch target
    mov     x11, #0                   // FLS §2.4.4.1: load imm 0
    str     x11, [sp, #16             ] // FLS §8.1: store slot 2
.L9:                              // FLS §6.17: branch target
    ldr     x12, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x12              // FLS §6.19: return reg 12 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn check_exact — FLS §9
    .global check_exact
check_exact:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #42                  // FLS §2.4.4.1: load imm 42
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, eq                    // FLS §6.5.3: x3 = (x1 == x2)
    cbz     x3, .L12                    // FLS §6.17: branch if false
    mov     x4, #1                   // FLS §2.4.4.1: load imm 1
    str     x4, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L13                       // FLS §6.17: branch to end
.L12:                              // FLS §6.17: branch target
    mov     x5, #0                   // FLS §2.4.4.1: load imm 0
    str     x5, [sp, #16             ] // FLS §8.1: store slot 2
.L13:                              // FLS §6.17: branch target
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn check_range — FLS §9
    .global check_range
check_range:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, ge                    // FLS §6.5.3: x3 = (x1 >= x2)
    mov     x4, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x1, x4               // FLS §6.5.3: compare (signed)
    cset    x5, le                    // FLS §6.5.3: x5 = (x1 <= x4)
    and     x6, x3, x5          // FLS §6.5.6: bitwise and
    cbz     x6, .L14                    // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L15                       // FLS §6.17: branch to end
.L14:                              // FLS §6.17: branch target
    mov     x8, #0                   // FLS §2.4.4.1: load imm 0
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
.L15:                              // FLS §6.17: branch target
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x9              // FLS §6.19: return reg 9 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn bind_and_use — FLS §9
    .global bind_and_use
bind_and_use:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x3, #1                   // FLS §2.4.4.1: load imm 1
    add     x4, x2, x3          // FLS §6.5.5: add
    str     x4, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L17                       // FLS §6.17: branch to end
.L16:                              // FLS §6.17: branch target
    mov     x5, #0                   // FLS §2.4.4.1: load imm 0
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
.L17:                              // FLS §6.17: branch target
    ldr     x6, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn describe_direction — FLS §9
    .global describe_direction
describe_direction:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, eq                    // FLS §6.5.3: x3 = (x1 == x2)
    cbz     x3, .L19                    // FLS §6.17: branch if false
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    str     x4, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L18                       // FLS §6.17: branch to end
.L19:                              // FLS §6.17: branch target
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x6, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x5, x6               // FLS §6.5.3: compare (signed)
    cset    x7, eq                    // FLS §6.5.3: x7 = (x5 == x6)
    cbz     x7, .L20                    // FLS §6.17: branch if false
    mov     x8, #1                   // FLS §2.4.4.1: load imm 1
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L18                       // FLS §6.17: branch to end
.L20:                              // FLS §6.17: branch target
    ldr     x9, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x10, #2                   // FLS §2.4.4.1: load imm 2
    cmp     x9, x10               // FLS §6.5.3: compare (signed)
    cset    x11, eq                    // FLS §6.5.3: x11 = (x9 == x10)
    cbz     x11, .L21                    // FLS §6.17: branch if false
    mov     x12, #2                   // FLS §2.4.4.1: load imm 2
    str     x12, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L18                       // FLS §6.17: branch to end
.L21:                              // FLS §6.17: branch target
    mov     x13, #3                   // FLS §2.4.4.1: load imm 3
    str     x13, [sp, #16             ] // FLS §8.1: store slot 2
.L18:                              // FLS §6.17: branch target
    ldr     x14, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x14              // FLS §6.19: return reg 14 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn unwrap_or_zero — FLS §9
    .global unwrap_or_zero
unwrap_or_zero:
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, eq                    // FLS §6.5.3: x3 = (x1 == x2)
    cbz     x3, .L23                    // FLS §6.17: branch if false
    ldr     x4, [sp, #8              ] // FLS §8.1: load slot 1
    str     x4, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x5, [sp, #32             ] // FLS §8.1: load slot 4
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L22                       // FLS §6.17: branch to end
.L23:                              // FLS §6.17: branch target
    mov     x6, #0                   // FLS §2.4.4.1: load imm 0
    str     x6, [sp, #24             ] // FLS §8.1: store slot 3
.L22:                              // FLS §6.17: branch target
    ldr     x7, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x0, x7              // FLS §6.19: return reg 7 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn magnitude_sq — FLS §9
    .global magnitude_sq
magnitude_sq:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mul     x2, x0, x1          // FLS §6.5.5: mul
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x4, [sp, #8              ] // FLS §8.1: load slot 1
    mul     x5, x3, x4          // FLS §6.5.5: mul
    add     x6, x2, x5          // FLS §6.5.5: add
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn swap — FLS §9
    .global swap
swap:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #8              ] // FLS §8.1: load slot 1
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    sub     x4, x2, x3          // FLS §6.5.5: sub
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn sum_pair — FLS §9
    .global sum_pair
sum_pair:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    add     x4, x2, x3          // FLS §6.5.5: add
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn nested_sum — FLS §9
    .global nested_sum
nested_sum:
    sub     sp, sp, #48             // FLS §8.1: frame for 6 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    str     x2, [sp, #40             ] // FLS §8.1: store slot 5
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    ldr     x4, [sp, #32             ] // FLS §8.1: load slot 4
    add     x5, x3, x4          // FLS §6.5.5: add
    ldr     x6, [sp, #40             ] // FLS §8.1: load slot 5
    add     x7, x5, x6          // FLS §6.5.5: add
    mov     x0, x7              // FLS §6.19: return reg 7 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn dist_sq — FLS §9
    .global dist_sq
dist_sq:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mul     x2, x0, x1          // FLS §6.5.5: mul
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x4, [sp, #8              ] // FLS §8.1: load slot 1
    mul     x5, x3, x4          // FLS §6.5.5: mul
    add     x6, x2, x5          // FLS §6.5.5: add
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #192            // FLS §8.1: frame for 23 slot(s)
    mov     x0, #2                   // FLS §2.4.4.1: load imm 2
    bl      range_inclusive          // FLS §6.12.1: call range_inclusive
    mov     x1, x0              // FLS §6.12.1: return value → x1
    str     x1, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x2, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x2                   // FLS §6.12.1: arg 0
    bl      range_exclusive          // FLS §6.12.1: call range_exclusive
    mov     x3, x0              // FLS §6.12.1: return value → x3
    str     x3, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x4, #3                   // FLS §2.4.4.1: load imm 3
    neg     x5, x4               // FLS §6.5.4: negate x4
    mov     x0, x5                   // FLS §6.12.1: arg 0
    bl      range_negative           // FLS §6.12.1: call range_negative
    mov     x6, x0              // FLS §6.12.1: return value → x6
    str     x6, [sp, #16             ] // FLS §8.1: store slot 2
    mov     x7, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x7                   // FLS §6.12.1: arg 0
    bl      classify_with_guard      // FLS §6.12.1: call classify_with_guard
    mov     x8, x0              // FLS §6.12.1: return value → x8
    str     x8, [sp, #24             ] // FLS §8.1: store slot 3
    mov     x9, #42                  // FLS §2.4.4.1: load imm 42
    mov     x0, x9                   // FLS §6.12.1: arg 0
    bl      check_exact              // FLS §6.12.1: call check_exact
    mov     x10, x0              // FLS §6.12.1: return value → x10
    str     x10, [sp, #32             ] // FLS §8.1: store slot 4
    mov     x11, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x11                  // FLS §6.12.1: arg 0
    bl      check_range              // FLS §6.12.1: call check_range
    mov     x12, x0              // FLS §6.12.1: return value → x12
    str     x12, [sp, #40             ] // FLS §8.1: store slot 5
    mov     x13, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x13                  // FLS §6.12.1: arg 0
    bl      bind_and_use             // FLS §6.12.1: call bind_and_use
    mov     x14, x0              // FLS §6.12.1: return value → x14
    str     x14, [sp, #48             ] // FLS §8.1: store slot 6
    mov     x15, #2                   // FLS §2.4.4.1: load imm 2
    mov     x0, x15                  // FLS §6.12.1: arg 0
    bl      describe_direction       // FLS §6.12.1: call describe_direction
    mov     x16, x0              // FLS §6.12.1: return value → x16
    str     x16, [sp, #56             ] // FLS §8.1: store slot 7
    mov     x17, #1                   // FLS §2.4.4.1: load imm 1
    str     x17, [sp, #64             ] // FLS §8.1: store slot 8
    mov     x18, #3                   // FLS §2.4.4.1: load imm 3
    str     x18, [sp, #72             ] // FLS §8.1: store slot 9
    ldr     x19, [sp, #64             ] // FLS §8.1: load slot 8
    ldr     x20, [sp, #72             ] // FLS §8.1: load slot 9
    mov     x0, x19                  // FLS §6.12.1: arg 0
    mov     x1, x20                  // FLS §6.12.1: arg 1
    bl      unwrap_or_zero           // FLS §6.12.1: call unwrap_or_zero
    mov     x21, x0              // FLS §6.12.1: return value → x21
    str     x21, [sp, #80             ] // FLS §8.1: store slot 10
    mov     x22, #0                   // FLS §2.4.4.1: load imm 0
    str     x22, [sp, #88             ] // FLS §8.1: store slot 11
    ldr     x23, [sp, #88             ] // FLS §8.1: load slot 11
    ldr     x24, [sp, #96             ] // FLS §8.1: load slot 12
    mov     x0, x23                  // FLS §6.12.1: arg 0
    mov     x1, x24                  // FLS §6.12.1: arg 1
    bl      unwrap_or_zero           // FLS §6.12.1: call unwrap_or_zero
    mov     x25, x0              // FLS §6.12.1: return value → x25
    str     x25, [sp, #104            ] // FLS §8.1: store slot 13
    mov     x26, #3                   // FLS §2.4.4.1: load imm 3
    mov     x27, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x26                  // FLS §6.12.1: arg 0
    mov     x1, x27                  // FLS §6.12.1: arg 1
    bl      swap                     // FLS §6.12.1: call swap
    mov     x28, x0              // FLS §6.12.1: return value → x28
    str     x28, [sp, #112            ] // FLS §8.1: store slot 14
    mov     x29, #1                   // FLS §2.4.4.1: load imm 1
    mov     x30, #2                   // FLS §2.4.4.1: load imm 2
    mov     x0, x29                  // FLS §6.12.1: arg 0
    mov     x1, x30                  // FLS §6.12.1: arg 1
    bl      sum_pair                 // FLS §6.12.1: call sum_pair
    mov     x31, x0              // FLS §6.12.1: return value → x31
    str     x31, [sp, #120            ] // FLS §8.1: store slot 15
    mov     x32, #1                   // FLS §2.4.4.1: load imm 1
    str     x32, [sp, #128            ] // FLS §8.1: store slot 16
    mov     x33, #0                   // FLS §2.4.4.1: load imm 0
    str     x33, [sp, #136            ] // FLS §8.1: store slot 17
    ldr     x34, [sp, #128            ] // FLS §8.1: load slot 16
    ldr     x35, [sp, #136            ] // FLS §8.1: load slot 17
    mov     x0, x34                  // FLS §6.12.1: arg 0
    mov     x1, x35                  // FLS §6.12.1: arg 1
    bl      magnitude_sq             // FLS §6.12.1: call magnitude_sq
    mov     x36, x0              // FLS §6.12.1: return value → x36
    str     x36, [sp, #144            ] // FLS §8.1: store slot 18
    mov     x37, #1                   // FLS §2.4.4.1: load imm 1
    str     x37, [sp, #152            ] // FLS §8.1: store slot 19
    mov     x38, #0                   // FLS §2.4.4.1: load imm 0
    str     x38, [sp, #160            ] // FLS §8.1: store slot 20
    ldr     x39, [sp, #152            ] // FLS §8.1: load slot 19
    ldr     x40, [sp, #160            ] // FLS §8.1: load slot 20
    mov     x0, x39                  // FLS §6.12.1: arg 0
    mov     x1, x40                  // FLS §6.12.1: arg 1
    bl      dist_sq                  // FLS §6.12.1: call dist_sq
    mov     x41, x0              // FLS §6.12.1: return value → x41
    str     x41, [sp, #168            ] // FLS §8.1: store slot 21
    mov     x42, #1                   // FLS §2.4.4.1: load imm 1
    mov     x43, #2                   // FLS §2.4.4.1: load imm 2
    mov     x44, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x42                  // FLS §6.12.1: arg 0
    mov     x1, x43                  // FLS §6.12.1: arg 1
    mov     x2, x44                  // FLS §6.12.1: arg 2
    bl      nested_sum               // FLS §6.12.1: call nested_sum
    mov     x45, x0              // FLS §6.12.1: return value → x45
    str     x45, [sp, #176            ] // FLS §8.1: store slot 22
    ldr     x46, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x47, [sp, #8              ] // FLS §8.1: load slot 1
    add     x48, x46, x47          // FLS §6.5.5: add
    ldr     x49, [sp, #16             ] // FLS §8.1: load slot 2
    add     x50, x48, x49          // FLS §6.5.5: add
    ldr     x51, [sp, #24             ] // FLS §8.1: load slot 3
    add     x52, x50, x51          // FLS §6.5.5: add
    ldr     x53, [sp, #32             ] // FLS §8.1: load slot 4
    add     x54, x52, x53          // FLS §6.5.5: add
    ldr     x55, [sp, #40             ] // FLS §8.1: load slot 5
    add     x56, x54, x55          // FLS §6.5.5: add
    ldr     x57, [sp, #48             ] // FLS §8.1: load slot 6
    add     x58, x56, x57          // FLS §6.5.5: add
    ldr     x59, [sp, #56             ] // FLS §8.1: load slot 7
    add     x60, x58, x59          // FLS §6.5.5: add
    ldr     x61, [sp, #80             ] // FLS §8.1: load slot 10
    add     x62, x60, x61          // FLS §6.5.5: add
    ldr     x63, [sp, #104            ] // FLS §8.1: load slot 13
    add     x64, x62, x63          // FLS §6.5.5: add
    ldr     x65, [sp, #112            ] // FLS §8.1: load slot 14
    add     x66, x64, x65          // FLS §6.5.5: add
    ldr     x67, [sp, #120            ] // FLS §8.1: load slot 15
    add     x68, x66, x67          // FLS §6.5.5: add
    ldr     x69, [sp, #144            ] // FLS §8.1: load slot 18
    add     x70, x68, x69          // FLS §6.5.5: add
    ldr     x71, [sp, #168            ] // FLS §8.1: load slot 21
    add     x72, x70, x71          // FLS §6.5.5: add
    ldr     x73, [sp, #176            ] // FLS §8.1: load slot 22
    add     x74, x72, x73          // FLS §6.5.5: add
    mov     x0, x74              // FLS §6.19: return reg 74 → x0
    add     sp, sp, #192            // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
