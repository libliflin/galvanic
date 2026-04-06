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
    cbz     x6, .L1                     // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x9, #4                   // FLS §2.4.4.1: load imm 4
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, ge                    // FLS §6.5.3: x10 = (x8 >= x9)
    mov     x11, #7                   // FLS §2.4.4.1: load imm 7
    cmp     x8, x11               // FLS §6.5.3: compare (signed)
    cset    x12, lt                    // FLS §6.5.3: x12 = (x8 < x11)
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
    cbz     x6, .L1                     // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x9, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, eq                    // FLS §6.5.3: x10 = (x8 == x9)
    cbz     x10, .L2                     // FLS §6.17: branch if false
    mov     x11, #2                   // FLS §2.4.4.1: load imm 2
    str     x11, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    mov     x12, #3                   // FLS §2.4.4.1: load imm 3
    str     x12, [sp, #16             ] // FLS §8.1: store slot 2
.L0:                              // FLS §6.17: branch target
    ldr     x13, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x13              // FLS §6.19: return reg 13 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
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
    ldr     x7, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    add     x9, x7, x8          // FLS §6.5.5: add
    ldr     x10, [sp, #16             ] // FLS §8.1: load slot 2
    add     x11, x9, x10          // FLS §6.5.5: add
    mov     x0, x11              // FLS §6.19: return reg 11 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
