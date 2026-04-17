    .text

    // fn named_block_tail — FLS §9
    .global named_block_tail
named_block_tail:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #3                   // FLS §2.4.4.1: load imm 3
    mov     x1, #4                   // FLS §2.4.4.1: load imm 4
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
.L0:                              // FLS §6.17: branch target
    str     x2, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn named_block_break_with_value — FLS §9
    .global named_block_break_with_value
named_block_break_with_value:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, gt                    // FLS §6.5.3: x2 = (x0 > x1)
    cbz     x2, .L2                     // FLS §6.17: branch if false
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x4, #2                   // FLS §2.4.4.1: load imm 2
    mul     x5, x3, x4          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    str     x5, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L1                        // FLS §6.17: branch to end
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
.L3:                              // FLS §6.17: branch target
    mov     x6, #0                   // FLS §2.4.4.1: load imm 0
    str     x6, [sp, #16             ] // FLS §8.1: store slot 2
.L1:                              // FLS §6.17: branch target
    ldr     x7, [sp, #16             ] // FLS §8.1: load slot 2
    str     x7, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x8              // FLS §6.19: return reg 8 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn named_block_break_arithmetic — FLS §9
    .global named_block_break_arithmetic
named_block_break_arithmetic:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, lt                    // FLS §6.5.3: x2 = (x0 < x1)
    cbz     x2, .L5                     // FLS §6.17: branch if false
    mov     x3, #0                   // FLS §2.4.4.1: load imm 0
    str     x3, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L4                        // FLS §6.17: branch to end
    b       .L6                        // FLS §6.17: branch to end
.L5:                              // FLS §6.17: branch target
.L6:                              // FLS §6.17: branch target
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    add     x6, x4, x5          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x6, [sp, #16             ] // FLS §8.1: store slot 2
.L4:                              // FLS §6.17: branch target
    ldr     x7, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x7              // FLS §6.19: return reg 7 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn named_block_in_loop — FLS §9
    .global named_block_in_loop
named_block_in_loop:
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #1                   // FLS §2.4.4.1: load imm 1
    neg     x1, x0               // FLS §6.5.4: negate x0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
.L7:                              // FLS §6.17: branch target
    ldr     x3, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, lt                    // FLS §6.5.3: x5 = (x3 < x4)
    cbz     x5, .L8                     // FLS §6.17: branch if false
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x7, #3                   // FLS §2.4.4.1: load imm 3
    cmp     x6, x7               // FLS §6.5.3: compare (signed)
    cset    x8, eq                    // FLS §6.5.3: x8 = (x6 == x7)
    cbz     x8, .L10                    // FLS §6.17: branch if false
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    str     x9, [sp, #32             ] // FLS §8.1: store slot 4
    b       .L9                        // FLS §6.17: branch to end
    b       .L11                       // FLS §6.17: branch to end
.L10:                              // FLS §6.17: branch target
.L11:                              // FLS §6.17: branch target
    mov     x10, #1                   // FLS §2.4.4.1: load imm 1
    neg     x11, x10               // FLS §6.5.4: negate x10
    str     x11, [sp, #32             ] // FLS §8.1: store slot 4
.L9:                              // FLS §6.17: branch target
    ldr     x12, [sp, #32             ] // FLS §8.1: load slot 4
    str     x12, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x13, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x14, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x13, x14               // FLS §6.5.3: compare (signed)
    cset    x15, ge                    // FLS §6.5.3: x15 = (x13 >= x14)
    cbz     x15, .L12                    // FLS §6.17: branch if false
    ldr     x16, [sp, #24             ] // FLS §8.1: load slot 3
    str     x16, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L13                       // FLS §6.17: branch to end
.L12:                              // FLS §6.17: branch target
.L13:                              // FLS §6.17: branch target
    ldr     x17, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x18, #1                   // FLS §2.4.4.1: load imm 1
    add     x19, x17, x18          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x19, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L7                        // FLS §6.17: branch to end
.L8:                              // FLS §6.17: branch target
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn nested_named_blocks — FLS §9
    .global nested_named_blocks
nested_named_blocks:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    cbz     x0, .L16                    // FLS §6.17: branch if false
    mov     x1, #1                   // FLS §2.4.4.1: load imm 1
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L15                       // FLS §6.17: branch to end
    b       .L17                       // FLS §6.17: branch to end
.L16:                              // FLS §6.17: branch target
.L17:                              // FLS §6.17: branch target
    mov     x2, #99                  // FLS §2.4.4.1: load imm 99
    str     x2, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L14                       // FLS §6.17: branch to end
.L15:                              // FLS §6.17: branch target
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    str     x3, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x4, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x5, #10                  // FLS §2.4.4.1: load imm 10
    add     x6, x4, x5          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x6, [sp, #8              ] // FLS §8.1: store slot 1
.L14:                              // FLS §6.17: branch target
    ldr     x7, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x7              // FLS §6.19: return reg 7 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn use_value — FLS §9
    .global use_value
use_value:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #1                   // FLS §2.4.4.1: load imm 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn named_block_as_argument — FLS §9
    .global named_block_as_argument
named_block_as_argument:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #5                   // FLS §2.4.4.1: load imm 5
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, gt                    // FLS §6.5.3: x2 = (x0 > x1)
    cbz     x2, .L19                    // FLS §6.17: branch if false
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    str     x3, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L18                       // FLS §6.17: branch to end
    b       .L20                       // FLS §6.17: branch to end
.L19:                              // FLS §6.17: branch target
.L20:                              // FLS §6.17: branch target
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    str     x4, [sp, #8              ] // FLS §8.1: store slot 1
.L18:                              // FLS §6.17: branch target
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x5                   // FLS §6.12.1: arg 0
    bl      use_value                // FLS §6.12.1: call use_value
    mov     x6, x0              // FLS §6.12.1: return value → x6
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn named_block_no_break_needed — FLS §9
    .global named_block_no_break_needed
named_block_no_break_needed:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #7                   // FLS §2.4.4.1: load imm 7
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x2, #6                   // FLS §2.4.4.1: load imm 6
    mul     x3, x1, x2          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
.L21:                              // FLS §6.17: branch target
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #64             // FLS §8.1: frame for 7 slot(s)
    bl      named_block_tail         // FLS §6.12.1: call named_block_tail
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x1                   // FLS §6.12.1: arg 0
    bl      named_block_break_with_value // FLS §6.12.1: call named_block_break_with_value
    mov     x2, x0              // FLS §6.12.1: return value → x2
    str     x2, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x3, #2                   // FLS §2.4.4.1: load imm 2
    mov     x4, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x3                   // FLS §6.12.1: arg 0
    mov     x1, x4                   // FLS §6.12.1: arg 1
    bl      named_block_break_arithmetic // FLS §6.12.1: call named_block_break_arithmetic
    mov     x5, x0              // FLS §6.12.1: return value → x5
    str     x5, [sp, #16             ] // FLS §8.1: store slot 2
    mov     x6, #10                  // FLS §2.4.4.1: load imm 10
    mov     x0, x6                   // FLS §6.12.1: arg 0
    bl      named_block_in_loop      // FLS §6.12.1: call named_block_in_loop
    mov     x7, x0              // FLS §6.12.1: return value → x7
    str     x7, [sp, #24             ] // FLS §8.1: store slot 3
    mov     x8, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x8                   // FLS §6.12.1: arg 0
    bl      nested_named_blocks      // FLS §6.12.1: call nested_named_blocks
    mov     x9, x0              // FLS §6.12.1: return value → x9
    str     x9, [sp, #32             ] // FLS §8.1: store slot 4
    mov     x10, #6                   // FLS §2.4.4.1: load imm 6
    mov     x0, x10                  // FLS §6.12.1: arg 0
    bl      named_block_as_argument  // FLS §6.12.1: call named_block_as_argument
    mov     x11, x0              // FLS §6.12.1: return value → x11
    str     x11, [sp, #40             ] // FLS §8.1: store slot 5
    bl      named_block_no_break_needed // FLS §6.12.1: call named_block_no_break_needed
    mov     x12, x0              // FLS §6.12.1: return value → x12
    str     x12, [sp, #48             ] // FLS §8.1: store slot 6
    ldr     x13, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x14, [sp, #8              ] // FLS §8.1: load slot 1
    add     x15, x13, x14          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x16, [sp, #16             ] // FLS §8.1: load slot 2
    add     x17, x15, x16          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x18, [sp, #24             ] // FLS §8.1: load slot 3
    add     x19, x17, x18          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x20, [sp, #32             ] // FLS §8.1: load slot 4
    add     x21, x19, x20          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x22, [sp, #40             ] // FLS §8.1: load slot 5
    add     x23, x21, x22          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x24, [sp, #48             ] // FLS §8.1: load slot 6
    add     x25, x23, x24          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x25              // FLS §6.19: return reg 25 → x0
    add     sp, sp, #64             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
