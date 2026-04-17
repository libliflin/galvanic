    .text

    // fn if_else_basic — FLS §9
    .global if_else_basic
if_else_basic:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, gt                    // FLS §6.5.3: x2 = (x0 > x1)
    cbz     x2, .L0                     // FLS §6.17: branch if false
    mov     x3, #1                   // FLS §2.4.4.1: load imm 1
    str     x3, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L1                        // FLS §6.17: branch to end
.L0:                              // FLS §6.17: branch target
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    str     x4, [sp, #8              ] // FLS §8.1: store slot 1
.L1:                              // FLS §6.17: branch target
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x5              // FLS §6.19: return reg 5 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn if_as_value — FLS §9
    .global if_as_value
if_as_value:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, ne                    // FLS §6.5.3: x2 = (x0 != x1)
    cbz     x2, .L2                     // FLS §6.17: branch if false
    mov     x3, #42                  // FLS §2.4.4.1: load imm 42
    str     x3, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    str     x4, [sp, #16             ] // FLS §8.1: store slot 2
.L3:                              // FLS §6.17: branch target
    ldr     x5, [sp, #16             ] // FLS §8.1: load slot 2
    str     x5, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn classify — FLS §9
    .global classify
classify:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, lt                    // FLS §6.5.3: x2 = (x0 < x1)
    cbz     x2, .L4                     // FLS §6.17: branch if false
    mov     x3, #0                   // FLS §2.4.4.1: load imm 0
    str     x3, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L5                        // FLS §6.17: branch to end
.L4:                              // FLS §6.17: branch target
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x5, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x4, x5               // FLS §6.5.3: compare (signed)
    cset    x6, eq                    // FLS §6.5.3: x6 = (x4 == x5)
    cbz     x6, .L6                     // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L7                        // FLS §6.17: branch to end
.L6:                              // FLS §6.17: branch target
    ldr     x8, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x9, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, lt                    // FLS §6.5.3: x10 = (x8 < x9)
    cbz     x10, .L8                     // FLS §6.17: branch if false
    mov     x11, #2                   // FLS §2.4.4.1: load imm 2
    str     x11, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L9                        // FLS §6.17: branch to end
.L8:                              // FLS §6.17: branch target
    mov     x12, #3                   // FLS §2.4.4.1: load imm 3
    str     x12, [sp, #24             ] // FLS §8.1: store slot 3
.L9:                              // FLS §6.17: branch target
    ldr     x13, [sp, #24             ] // FLS §8.1: load slot 3
    str     x13, [sp, #16             ] // FLS §8.1: store slot 2
.L7:                              // FLS §6.17: branch target
    ldr     x14, [sp, #16             ] // FLS §8.1: load slot 2
    str     x14, [sp, #8              ] // FLS §8.1: store slot 1
.L5:                              // FLS §6.17: branch target
    ldr     x15, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x15              // FLS §6.19: return reg 15 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn if_let_some — FLS §9
    .global if_let_some
if_let_some:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, eq                    // FLS §6.5.3: x3 = (x1 == x2)
    cbz     x3, .L10                    // FLS §6.17: branch if false
    mov     x4, #99                  // FLS §2.4.4.1: load imm 99
    str     x4, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L11                       // FLS §6.17: branch to end
.L10:                              // FLS §6.17: branch target
    ldr     x5, [sp, #0              ] // FLS §8.1: load slot 0
    str     x5, [sp, #16             ] // FLS §8.1: store slot 2
.L11:                              // FLS §6.17: branch target
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn if_let_bind — FLS §9
    .global if_let_bind
if_let_bind:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x3, #1                   // FLS §2.4.4.1: load imm 1
    add     x4, x2, x3          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x4, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L13                       // FLS §6.17: branch to end
.L12:                              // FLS §6.17: branch target
    mov     x5, #0                   // FLS §2.4.4.1: load imm 0
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
.L13:                              // FLS §6.17: branch target
    ldr     x6, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn if_no_else — FLS §9
    .global if_no_else
if_no_else:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, gt                    // FLS §6.5.3: x2 = (x0 > x1)
    cbz     x2, .L14                    // FLS §6.17: branch if false
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x4, #1                   // FLS §2.4.4.1: load imm 1
    add     x5, x3, x4          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    b       .L15                       // FLS §6.17: branch to end
.L14:                              // FLS §6.17: branch target
.L15:                              // FLS §6.17: branch target
    mov     x0, #0              // FLS §4.4: unit return
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn nested_if — FLS §9
    .global nested_if
nested_if:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, gt                    // FLS §6.5.3: x2 = (x0 > x1)
    cbz     x2, .L16                    // FLS §6.17: branch if false
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, gt                    // FLS §6.5.3: x5 = (x3 > x4)
    cbz     x5, .L18                    // FLS §6.17: branch if false
    ldr     x6, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x7, [sp, #8              ] // FLS §8.1: load slot 1
    add     x8, x6, x7          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x8, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L19                       // FLS §6.17: branch to end
.L18:                              // FLS §6.17: branch target
    ldr     x9, [sp, #0              ] // FLS §8.1: load slot 0
    str     x9, [sp, #24             ] // FLS §8.1: store slot 3
.L19:                              // FLS §6.17: branch target
    ldr     x10, [sp, #24             ] // FLS §8.1: load slot 3
    str     x10, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L17                       // FLS §6.17: branch to end
.L16:                              // FLS §6.17: branch target
    mov     x11, #0                   // FLS §2.4.4.1: load imm 0
    str     x11, [sp, #16             ] // FLS §8.1: store slot 2
.L17:                              // FLS §6.17: branch target
    ldr     x12, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x12              // FLS §6.19: return reg 12 → x0
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
    mov     x4, #5                   // FLS §2.4.4.1: load imm 5
    cmp     x1, x4               // FLS §6.5.3: compare (signed)
    cset    x5, le                    // FLS §6.5.3: x5 = (x1 <= x4)
    and     x6, x3, x5          // FLS §6.5.6: bitwise and
    cbz     x6, .L20                    // FLS §6.17: branch if false
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L21                       // FLS §6.17: branch to end
.L20:                              // FLS §6.17: branch target
    mov     x8, #0                   // FLS §2.4.4.1: load imm 0
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
.L21:                              // FLS §6.17: branch target
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x9              // FLS §6.19: return reg 9 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    mov     x0, #1                   // FLS §2.4.4.1: load imm 1
    bl      if_else_basic            // FLS §6.12.1: call if_else_basic
    mov     x1, x0              // FLS §6.12.1: return value → x1
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    neg     x3, x2               // FLS §6.5.4: negate x2
    mov     x0, x3                   // FLS §6.12.1: arg 0
    bl      if_else_basic            // FLS §6.12.1: call if_else_basic
    mov     x4, x0              // FLS §6.12.1: return value → x4
    mov     x5, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x5                   // FLS §6.12.1: arg 0
    bl      if_as_value              // FLS §6.12.1: call if_as_value
    mov     x6, x0              // FLS §6.12.1: return value → x6
    mov     x7, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x7                   // FLS §6.12.1: arg 0
    bl      if_as_value              // FLS §6.12.1: call if_as_value
    mov     x8, x0              // FLS §6.12.1: return value → x8
    mov     x9, #3                   // FLS §2.4.4.1: load imm 3
    neg     x10, x9               // FLS §6.5.4: negate x9
    mov     x0, x10                  // FLS §6.12.1: arg 0
    bl      classify                 // FLS §6.12.1: call classify
    mov     x11, x0              // FLS §6.12.1: return value → x11
    mov     x12, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x12                  // FLS §6.12.1: arg 0
    bl      classify                 // FLS §6.12.1: call classify
    mov     x13, x0              // FLS §6.12.1: return value → x13
    mov     x14, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x14                  // FLS §6.12.1: arg 0
    bl      classify                 // FLS §6.12.1: call classify
    mov     x15, x0              // FLS §6.12.1: return value → x15
    mov     x16, #100                 // FLS §2.4.4.1: load imm 100
    mov     x0, x16                  // FLS §6.12.1: arg 0
    bl      classify                 // FLS §6.12.1: call classify
    mov     x17, x0              // FLS §6.12.1: return value → x17
    mov     x18, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x18                  // FLS §6.12.1: arg 0
    bl      if_let_some              // FLS §6.12.1: call if_let_some
    mov     x19, x0              // FLS §6.12.1: return value → x19
    mov     x20, #7                   // FLS §2.4.4.1: load imm 7
    mov     x0, x20                  // FLS §6.12.1: arg 0
    bl      if_let_some              // FLS §6.12.1: call if_let_some
    mov     x21, x0              // FLS §6.12.1: return value → x21
    mov     x22, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x22                  // FLS §6.12.1: arg 0
    bl      if_let_bind              // FLS §6.12.1: call if_let_bind
    mov     x23, x0              // FLS §6.12.1: return value → x23
    mov     x24, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x24                  // FLS §6.12.1: arg 0
    bl      if_no_else               // FLS §6.12.1: call if_no_else
    mov     x25, x0              // FLS §6.12.1: return value → x25
    mov     x26, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x26                  // FLS §6.12.1: arg 0
    bl      if_no_else               // FLS §6.12.1: call if_no_else
    mov     x27, x0              // FLS §6.12.1: return value → x27
    mov     x28, #2                   // FLS §2.4.4.1: load imm 2
    mov     x29, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x28                  // FLS §6.12.1: arg 0
    mov     x1, x29                  // FLS §6.12.1: arg 1
    bl      nested_if                // FLS §6.12.1: call nested_if
    mov     x30, x0              // FLS §6.12.1: return value → x30
    mov     x31, #2                   // FLS §2.4.4.1: load imm 2
    mov     x32, #1                   // FLS §2.4.4.1: load imm 1
    neg     x33, x32               // FLS §6.5.4: negate x32
    mov     x0, x31                  // FLS §6.12.1: arg 0
    mov     x1, x33                  // FLS §6.12.1: arg 1
    bl      nested_if                // FLS §6.12.1: call nested_if
    mov     x34, x0              // FLS §6.12.1: return value → x34
    mov     x35, #1                   // FLS §2.4.4.1: load imm 1
    neg     x36, x35               // FLS §6.5.4: negate x35
    mov     x37, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x36                  // FLS §6.12.1: arg 0
    mov     x1, x37                  // FLS §6.12.1: arg 1
    bl      nested_if                // FLS §6.12.1: call nested_if
    mov     x38, x0              // FLS §6.12.1: return value → x38
    mov     x39, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x39                  // FLS §6.12.1: arg 0
    bl      if_let_range             // FLS §6.12.1: call if_let_range
    mov     x40, x0              // FLS §6.12.1: return value → x40
    mov     x41, #9                   // FLS §2.4.4.1: load imm 9
    mov     x0, x41                  // FLS §6.12.1: arg 0
    bl      if_let_range             // FLS §6.12.1: call if_let_range
    mov     x42, x0              // FLS §6.12.1: return value → x42
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
