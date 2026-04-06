    .text

    // fn main — FLS §9
    .global main
main:
    sub     sp, sp, #224            // FLS §8.1: frame for 28 slot(s)
    mov     x0, #5                   // FLS §2.4.4.1: load imm 5
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #1                   // FLS §2.4.4.1: load imm 1
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x2, #42                  // FLS §2.4.4.1: load imm 42
    neg     x3, x2               // FLS §6.5.4: negate x2
    str     x3, [sp, #16             ] // FLS §8.1: store slot 2
    mov     x4, #42                  // FLS §2.4.4.1: load imm 42
    mvn     x5, x4               // FLS §6.5.4: bitwise NOT x4
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
    mov     x6, #0                   // FLS §2.4.4.1: load imm 0
    mvn     x7, x6               // FLS §6.5.4: bitwise NOT x6
    str     x7, [sp, #32             ] // FLS §8.1: store slot 4
    mov     x8, #1                   // FLS §2.4.4.1: load imm 1
    mov     x9, #2                   // FLS §2.4.4.1: load imm 2
    add     x10, x8, x9          // FLS §6.5.5: add
    str     x10, [sp, #40             ] // FLS §8.1: store slot 5
    mov     x11, #10                  // FLS §2.4.4.1: load imm 10
    mov     x12, #4                   // FLS §2.4.4.1: load imm 4
    sdiv    x13, x11, x12          // FLS §6.5.5: rem step 1: quotient
    msub    x13, x13, x12, x11  // FLS §6.5.5: rem step 2: lhs - q*rhs
    str     x13, [sp, #48             ] // FLS §8.1: store slot 6
    mov     x14, #10                  // FLS §2.4.4.1: load imm 10
    mov     x15, #2                   // FLS §2.4.4.1: load imm 2
    sdiv    x16, x14, x15          // FLS §6.5.5: div (signed)
    str     x16, [sp, #56             ] // FLS §8.1: store slot 7
    mov     x17, #3                   // FLS §2.4.4.1: load imm 3
    mov     x18, #2                   // FLS §2.4.4.1: load imm 2
    sub     x19, x17, x18          // FLS §6.5.5: sub
    str     x19, [sp, #64             ] // FLS §8.1: store slot 8
    mov     x20, #10                  // FLS §2.4.4.1: load imm 10
    mov     x21, #12                  // FLS §2.4.4.1: load imm 12
    and     x22, x20, x21          // FLS §6.5.6: bitwise and
    str     x22, [sp, #72             ] // FLS §8.1: store slot 9
    mov     x23, #10                  // FLS §2.4.4.1: load imm 10
    mov     x24, #3                   // FLS §2.4.4.1: load imm 3
    orr     x25, x23, x24          // FLS §6.5.6: bitwise or
    str     x25, [sp, #80             ] // FLS §8.1: store slot 10
    mov     x26, #10                  // FLS §2.4.4.1: load imm 10
    mov     x27, #9                   // FLS §2.4.4.1: load imm 9
    eor     x28, x26, x27          // FLS §6.5.6: bitwise xor
    str     x28, [sp, #88             ] // FLS §8.1: store slot 11
    mov     x29, #13                  // FLS §2.4.4.1: load imm 13
    mov     x30, #3                   // FLS §2.4.4.1: load imm 3
    lsl     x31, x29, x30          // FLS §6.5.7: shift left
    str     x31, [sp, #96             ] // FLS §8.1: store slot 12
    mov     x32, #10                  // FLS §2.4.4.1: load imm 10
    neg     x33, x32               // FLS §6.5.4: negate x32
    mov     x34, #2                   // FLS §2.4.4.1: load imm 2
    asr     x35, x33, x34          // FLS §6.5.7: arithmetic shift right (signed)
    str     x35, [sp, #104            ] // FLS §8.1: store slot 13
    mov     x36, #12                  // FLS §2.4.4.1: load imm 12
    mov     x37, #12                  // FLS §2.4.4.1: load imm 12
    cmp     x36, x37               // FLS §6.5.3: compare (signed)
    cset    x38, eq                    // FLS §6.5.3: x38 = (x36 == x37)
    str     x38, [sp, #112            ] // FLS §8.1: store slot 14
    mov     x39, #42                  // FLS §2.4.4.1: load imm 42
    mov     x40, #12                  // FLS §2.4.4.1: load imm 12
    cmp     x39, x40               // FLS §6.5.3: compare (signed)
    cset    x41, gt                    // FLS §6.5.3: x41 = (x39 > x40)
    str     x41, [sp, #120            ] // FLS §8.1: store slot 15
    mov     x42, #42                  // FLS §2.4.4.1: load imm 42
    mov     x43, #35                  // FLS §2.4.4.1: load imm 35
    cmp     x42, x43               // FLS §6.5.3: compare (signed)
    cset    x44, ge                    // FLS §6.5.3: x44 = (x42 >= x43)
    str     x44, [sp, #128            ] // FLS §8.1: store slot 16
    mov     x45, #42                  // FLS §2.4.4.1: load imm 42
    mov     x46, #109                 // FLS §2.4.4.1: load imm 109
    cmp     x45, x46               // FLS §6.5.3: compare (signed)
    cset    x47, lt                    // FLS §6.5.3: x47 = (x45 < x46)
    str     x47, [sp, #136            ] // FLS §8.1: store slot 17
    mov     x48, #42                  // FLS §2.4.4.1: load imm 42
    mov     x49, #42                  // FLS §2.4.4.1: load imm 42
    cmp     x48, x49               // FLS §6.5.3: compare (signed)
    cset    x50, le                    // FLS §6.5.3: x50 = (x48 <= x49)
    str     x50, [sp, #144            ] // FLS §8.1: store slot 18
    mov     x51, #12                  // FLS §2.4.4.1: load imm 12
    mov     x52, #42                  // FLS §2.4.4.1: load imm 42
    cmp     x51, x52               // FLS §6.5.3: compare (signed)
    cset    x53, ne                    // FLS §6.5.3: x53 = (x51 != x52)
    str     x53, [sp, #152            ] // FLS §8.1: store slot 19
    mov     x54, #1                   // FLS §2.4.4.1: load imm 1
    cbz     x54, .L0                     // FLS §6.17: branch if false
    str     x54, [sp, #168            ] // FLS §8.1: store slot 21
    b       .L1                        // FLS §6.17: branch to end
.L0:                              // FLS §6.17: branch target
    mov     x55, #0                   // FLS §2.4.4.1: load imm 0
    str     x55, [sp, #168            ] // FLS §8.1: store slot 21
.L1:                              // FLS §6.17: branch target
    ldr     x56, [sp, #168            ] // FLS §8.1: load slot 21
    str     x56, [sp, #160            ] // FLS §8.1: store slot 20
    mov     x57, #1                   // FLS §2.4.4.1: load imm 1
    cbz     x57, .L2                     // FLS §6.17: branch if false
    mov     x58, #0                   // FLS §2.4.4.1: load imm 0
    str     x58, [sp, #184            ] // FLS §8.1: store slot 23
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    mov     x59, #0                   // FLS §2.4.4.1: load imm 0
    str     x59, [sp, #184            ] // FLS §8.1: store slot 23
.L3:                              // FLS §6.17: branch target
    ldr     x60, [sp, #184            ] // FLS §8.1: load slot 23
    str     x60, [sp, #176            ] // FLS §8.1: store slot 22
    mov     x61, #5                   // FLS §2.4.4.1: load imm 5
    str     x61, [sp, #192            ] // FLS §8.1: store slot 24
    mov     x62, #1                   // FLS §2.4.4.1: load imm 1
    str     x62, [sp, #200            ] // FLS §8.1: store slot 25
    mov     x63, #0                   // FLS §2.4.4.1: load imm 0
    str     x63, [sp, #208            ] // FLS §8.1: store slot 26
    mov     x64, #42                  // FLS §2.4.4.1: load imm 42
    str     x64, [sp, #216            ] // FLS §8.1: store slot 27
    mov     x0, #0              // FLS §4.4: unit return
    add     sp, sp, #224            // FLS §8.1: restore stack frame
    ret

    // fn classify_age — FLS §9
    .global classify_age
classify_age:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #14                  // FLS §2.4.4.1: load imm 14
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, le                    // FLS §6.5.3: x2 = (x0 <= x1)
    cbz     x2, .L0                     // FLS §6.17: branch if false
    mov     x3, #0                   // FLS §2.4.4.1: load imm 0
    str     x3, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L1                        // FLS §6.17: branch to end
.L0:                              // FLS §6.17: branch target
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x5, #24                  // FLS §2.4.4.1: load imm 24
    cmp     x4, x5               // FLS §6.5.3: compare (signed)
    cset    x6, le                    // FLS §6.5.3: x6 = (x4 <= x5)
    cbz     x6, .L2                     // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    ldr     x8, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x9, #64                  // FLS §2.4.4.1: load imm 64
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, le                    // FLS §6.5.3: x10 = (x8 <= x9)
    cbz     x10, .L4                     // FLS §6.17: branch if false
    mov     x11, #2                   // FLS §2.4.4.1: load imm 2
    str     x11, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L5                        // FLS §6.17: branch to end
.L4:                              // FLS §6.17: branch target
    mov     x12, #3                   // FLS §2.4.4.1: load imm 3
    str     x12, [sp, #24             ] // FLS §8.1: store slot 3
.L5:                              // FLS §6.17: branch target
    ldr     x13, [sp, #24             ] // FLS §8.1: load slot 3
    str     x13, [sp, #16             ] // FLS §8.1: store slot 2
.L3:                              // FLS §6.17: branch target
    ldr     x14, [sp, #16             ] // FLS §8.1: load slot 2
    str     x14, [sp, #8              ] // FLS §8.1: store slot 1
.L1:                              // FLS §6.17: branch target
    ldr     x15, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x15              // FLS §6.19: return reg 15 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn count_to_ten — FLS §9
    .global count_to_ten
count_to_ten:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
.L0:                              // FLS §6.17: branch target
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x2, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, lt                    // FLS §6.5.3: x3 = (x1 < x2)
    cbz     x3, .L1                     // FLS §6.17: branch if false
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x5, #1                   // FLS §2.4.4.1: load imm 1
    add     x6, x4, x5          // FLS §6.5.5: add
    str     x6, [sp, #0              ] // FLS §8.1: store slot 0
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x7, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x7              // FLS §6.19: return reg 7 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn find_first_over_threshold — FLS §9
    .global find_first_over_threshold
find_first_over_threshold:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #1                   // FLS §2.4.4.1: load imm 1
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
.L0:                              // FLS §6.17: branch target
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x2, #100                 // FLS §2.4.4.1: load imm 100
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, gt                    // FLS §6.5.3: x3 = (x1 > x2)
    cbz     x3, .L2                     // FLS §6.17: branch if false
    b       .L1                        // FLS §6.17: branch to end
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
.L3:                              // FLS §6.17: branch target
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x5, #2                   // FLS §2.4.4.1: load imm 2
    mul     x6, x4, x5          // FLS §6.5.5: mul
    str     x6, [sp, #0              ] // FLS §8.1: store slot 0
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x7, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x7              // FLS §6.19: return reg 7 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn loop_returning_value — FLS §9
    .global loop_returning_value
loop_returning_value:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
.L0:                              // FLS §6.17: branch target
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    add     x3, x1, x2          // FLS §6.5.5: add
    str     x3, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x5, #7                   // FLS §2.4.4.1: load imm 7
    cmp     x4, x5               // FLS §6.5.3: compare (signed)
    cset    x6, ge                    // FLS §6.5.3: x6 = (x4 >= x5)
    cbz     x6, .L2                     // FLS §6.17: branch if false
    ldr     x7, [sp, #0              ] // FLS §8.1: load slot 0
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L1                        // FLS §6.17: branch to end
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
.L3:                              // FLS §6.17: branch target
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x8, [sp, #16             ] // FLS §8.1: load slot 2
    str     x8, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x9, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x9              // FLS §6.19: return reg 9 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn sum_skipping_three — FLS §9
    .global sum_skipping_three
sum_skipping_three:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
.L0:                              // FLS §6.17: branch target
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x3, #5                   // FLS §2.4.4.1: load imm 5
    cmp     x2, x3               // FLS §6.5.3: compare (signed)
    cset    x4, lt                    // FLS §6.5.3: x4 = (x2 < x3)
    cbz     x4, .L1                     // FLS §6.17: branch if false
    ldr     x5, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x6, #1                   // FLS §2.4.4.1: load imm 1
    add     x7, x5, x6          // FLS §6.5.5: add
    str     x7, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x8, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x9, #3                   // FLS §2.4.4.1: load imm 3
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, eq                    // FLS §6.5.3: x10 = (x8 == x9)
    cbz     x10, .L2                     // FLS §6.17: branch if false
    b       .L0                        // FLS §6.17: branch to end
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
.L3:                              // FLS §6.17: branch target
    ldr     x11, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x12, [sp, #0              ] // FLS §8.1: load slot 0
    add     x13, x11, x12          // FLS §6.5.5: add
    str     x13, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x14, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x14              // FLS §6.19: return reg 14 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn use_call — FLS §9
    .global use_call
use_call:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #1                   // FLS §2.4.4.1: load imm 1
    mov     x1, #2                   // FLS §2.4.4.1: load imm 2
    bl      add_two                  // FLS §6.12.1: call add_two
    mov     x2, x0              // FLS §6.12.1: return value → x2
    str     x2, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn add_two — FLS §9
    .global add_two
add_two:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    add     x2, x0, x1          // FLS §6.5.5: add
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn bool_param_example — FLS §9
    .global bool_param_example
bool_param_example:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    cbz     x0, .L0                     // FLS §6.17: branch if false
    mov     x1, #1                   // FLS §2.4.4.1: load imm 1
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L1                        // FLS §6.17: branch to end
.L0:                              // FLS §6.17: branch target
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    str     x2, [sp, #8              ] // FLS §8.1: store slot 1
.L1:                              // FLS §6.17: branch target
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn bool_return_example — FLS §9
    .global bool_return_example
bool_return_example:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, gt                    // FLS §6.5.3: x2 = (x0 > x1)
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn bool_not_example — FLS §9
    .global bool_not_example
bool_not_example:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    eor     x1, x0, #1             // FLS §6.5.4: logical NOT x0 (bool)
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn compound_assign_example — FLS §9
    .global compound_assign_example
compound_assign_example:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #5                   // FLS §2.4.4.1: load imm 5
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x2, #3                   // FLS §2.4.4.1: load imm 3
    add     x3, x1, x2          // FLS §6.5.5: add
    str     x3, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x5, #1                   // FLS §2.4.4.1: load imm 1
    sub     x6, x4, x5          // FLS §6.5.5: sub
    str     x6, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x7, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x8, #2                   // FLS §2.4.4.1: load imm 2
    mul     x9, x7, x8          // FLS §6.5.5: mul
    str     x9, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x10, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x11, #2                   // FLS §2.4.4.1: load imm 2
    sdiv    x12, x10, x11          // FLS §6.5.5: div (signed)
    str     x12, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x13, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x14, #3                   // FLS §2.4.4.1: load imm 3
    sdiv    x15, x13, x14          // FLS §6.5.5: rem step 1: quotient
    msub    x15, x15, x14, x13  // FLS §6.5.5: rem step 2: lhs - q*rhs
    str     x15, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x16, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x17, #3                   // FLS §2.4.4.1: load imm 3
    and     x18, x16, x17          // FLS §6.5.6: bitwise and
    str     x18, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x19, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x20, #4                   // FLS §2.4.4.1: load imm 4
    orr     x21, x19, x20          // FLS §6.5.6: bitwise or
    str     x21, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x22, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x23, #2                   // FLS §2.4.4.1: load imm 2
    eor     x24, x22, x23          // FLS §6.5.6: bitwise xor
    str     x24, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x25, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x26, #1                   // FLS §2.4.4.1: load imm 1
    lsl     x27, x25, x26          // FLS §6.5.7: shift left
    str     x27, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x28, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x29, #1                   // FLS §2.4.4.1: load imm 1
    asr     x30, x28, x29          // FLS §6.5.7: arithmetic shift right (signed)
    str     x30, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x31, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x31              // FLS §6.19: return reg 31 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn for_loop_sum_example — FLS §9
    .global for_loop_sum_example
for_loop_sum_example:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x2, #5                   // FLS §2.4.4.1: load imm 5
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
.L0:                              // FLS §6.17: branch target
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x4, [sp, #16             ] // FLS §8.1: load slot 2
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, lt                    // FLS §6.5.3: x5 = (x3 < x4)
    cbz     x5, .L2                     // FLS §6.17: branch if false
    ldr     x6, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x7, [sp, #8              ] // FLS §8.1: load slot 1
    add     x8, x6, x7          // FLS §6.5.5: add
    str     x8, [sp, #0              ] // FLS §8.1: store slot 0
.L1:                              // FLS §6.17: branch target
    ldr     x9, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x10, #1                   // FLS §2.4.4.1: load imm 1
    add     x11, x9, x10          // FLS §6.5.5: add
    str     x11, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L0                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    ldr     x12, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x12              // FLS §6.19: return reg 12 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn for_loop_inclusive_example — FLS §9
    .global for_loop_inclusive_example
for_loop_inclusive_example:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    mov     x0, #1                   // FLS §2.4.4.1: load imm 1
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #1                   // FLS §2.4.4.1: load imm 1
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x2, #4                   // FLS §2.4.4.1: load imm 4
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
.L0:                              // FLS §6.17: branch target
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x4, [sp, #16             ] // FLS §8.1: load slot 2
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, le                    // FLS §6.5.3: x5 = (x3 <= x4)
    cbz     x5, .L2                     // FLS §6.17: branch if false
    ldr     x6, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x7, [sp, #8              ] // FLS §8.1: load slot 1
    mul     x8, x6, x7          // FLS §6.5.5: mul
    str     x8, [sp, #0              ] // FLS §8.1: store slot 0
.L1:                              // FLS §6.17: branch target
    ldr     x9, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x10, #1                   // FLS §2.4.4.1: load imm 1
    add     x11, x9, x10          // FLS §6.5.5: add
    str     x11, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L0                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    ldr     x12, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x12              // FLS §6.19: return reg 12 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn uninit_let_example — FLS §9
    .global uninit_let_example
uninit_let_example:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    mov     x0, #7                   // FLS §2.4.4.1: load imm 7
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    add     x3, x1, x2          // FLS §6.5.5: add
    str     x3, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x4, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn conditional_init_example — FLS §9
    .global conditional_init_example
conditional_init_example:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    cbz     x0, .L0                     // FLS §6.17: branch if false
    mov     x1, #1                   // FLS §2.4.4.1: load imm 1
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L1                        // FLS §6.17: branch to end
.L0:                              // FLS §6.17: branch target
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    str     x2, [sp, #8              ] // FLS §8.1: store slot 1
.L1:                              // FLS §6.17: branch target
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn match_example — FLS §9
    .global match_example
match_example:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, eq                    // FLS §6.5.3: x3 = (x1 == x2)
    cbz     x3, .L1                     // FLS §6.17: branch if false
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    str     x4, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x6, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x5, x6               // FLS §6.5.3: compare (signed)
    cset    x7, eq                    // FLS §6.5.3: x7 = (x5 == x6)
    cbz     x7, .L2                     // FLS §6.17: branch if false
    mov     x8, #1                   // FLS §2.4.4.1: load imm 1
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    mov     x9, #2                   // FLS §2.4.4.1: load imm 2
    str     x9, [sp, #16             ] // FLS §8.1: store slot 2
.L0:                              // FLS §6.17: branch target
    ldr     x10, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x10              // FLS §6.19: return reg 10 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn match_bool_example — FLS §9
    .global match_bool_example
match_bool_example:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, eq                    // FLS §6.5.3: x3 = (x1 == x2)
    cbz     x3, .L1                     // FLS §6.17: branch if false
    mov     x4, #1                   // FLS §2.4.4.1: load imm 1
    str     x4, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    mov     x5, #0                   // FLS §2.4.4.1: load imm 0
    str     x5, [sp, #16             ] // FLS §8.1: store slot 2
.L0:                              // FLS §6.17: branch target
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn match_negative_pattern — FLS §9
    .global match_negative_pattern
match_negative_pattern:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #-2                  // FLS §2.4.4.1: load imm -2
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, eq                    // FLS §6.5.3: x3 = (x1 == x2)
    cbz     x3, .L1                     // FLS §6.17: branch if false
    mov     x4, #10                  // FLS §2.4.4.1: load imm 10
    str     x4, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x6, #-1                  // FLS §2.4.4.1: load imm -1
    cmp     x5, x6               // FLS §6.5.3: compare (signed)
    cset    x7, eq                    // FLS §6.5.3: x7 = (x5 == x6)
    cbz     x7, .L2                     // FLS §6.17: branch if false
    mov     x8, #20                  // FLS §2.4.4.1: load imm 20
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    ldr     x9, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x10, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x9, x10               // FLS §6.5.3: compare (signed)
    cset    x11, eq                    // FLS §6.5.3: x11 = (x9 == x10)
    cbz     x11, .L3                     // FLS §6.17: branch if false
    mov     x12, #30                  // FLS §2.4.4.1: load imm 30
    str     x12, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L3:                              // FLS §6.17: branch target
    mov     x13, #40                  // FLS §2.4.4.1: load imm 40
    str     x13, [sp, #16             ] // FLS §8.1: store slot 2
.L0:                              // FLS §6.17: branch target
    ldr     x14, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x14              // FLS §6.19: return reg 14 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn match_ident_pattern — FLS §9
    .global match_ident_pattern
match_ident_pattern:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, eq                    // FLS §6.5.3: x3 = (x1 == x2)
    cbz     x3, .L1                     // FLS §6.17: branch if false
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    str     x4, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x6, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x7, #2                   // FLS §2.4.4.1: load imm 2
    mul     x8, x6, x7          // FLS §6.5.5: mul
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
.L0:                              // FLS §6.17: branch target
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x9              // FLS §6.19: return reg 9 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn struct_expr_example — FLS §9
    .global struct_expr_example
struct_expr_example:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    mov     x0, #10                  // FLS §2.4.4.1: load imm 10
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #20                  // FLS §2.4.4.1: load imm 20
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    add     x4, x2, x3          // FLS §6.5.5: add
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn field_access_example — FLS §9
    .global field_access_example
field_access_example:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    mov     x0, #6                   // FLS §2.4.4.1: load imm 6
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #7                   // FLS §2.4.4.1: load imm 7
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    mul     x4, x2, x3          // FLS §6.5.5: mul
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
