    .text

    // fn early_return_taken — FLS §9
    .global early_return_taken
early_return_taken:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, gt                    // FLS §6.5.3: x2 = (x0 > x1)
    cbz     x2, .L0                     // FLS §6.17: branch if false
    mov     x3, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret
    b       .L1                        // FLS §6.17: branch to end
.L0:                              // FLS §6.17: branch target
.L1:                              // FLS §6.17: branch target
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn early_return_not_taken — FLS §9
    .global early_return_not_taken
early_return_not_taken:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, lt                    // FLS §6.5.3: x2 = (x0 < x1)
    cbz     x2, .L2                     // FLS §6.17: branch if false
    mov     x3, #1                   // FLS §2.4.4.1: load imm 1
    neg     x4, x3               // FLS §6.5.4: negate x3
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
.L3:                              // FLS §6.17: branch target
    ldr     x5, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x5              // FLS §6.19: return reg 5 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn return_unit — FLS §9
    .global return_unit
return_unit:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, eq                    // FLS §6.5.3: x2 = (x0 == x1)
    cbz     x2, .L4                     // FLS §6.17: branch if false
    mov     x0, #0              // FLS §4.4: unit return
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret
    b       .L5                        // FLS §6.17: branch to end
.L4:                              // FLS §6.17: branch target
.L5:                              // FLS §6.17: branch target
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x4, #1                   // FLS §2.4.4.1: load imm 1
    add     x5, x3, x4          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, #0              // FLS §4.4: unit return
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn tail_expression_return — FLS §9
    .global tail_expression_return
tail_expression_return:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #2                   // FLS §2.4.4.1: load imm 2
    mul     x2, x0, x1          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    str     x2, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn return_from_loop — FLS §9
    .global return_from_loop
return_from_loop:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
.L6:                              // FLS §6.17: branch target
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, ge                    // FLS §6.5.3: x3 = (x1 >= x2)
    cbz     x3, .L8                     // FLS §6.17: branch if false
    ldr     x4, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret
    b       .L9                        // FLS §6.17: branch to end
.L8:                              // FLS §6.17: branch target
.L9:                              // FLS §6.17: branch target
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x6, #1                   // FLS §2.4.4.1: load imm 1
    add     x7, x5, x6          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x7, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L6                        // FLS §6.17: branch to end
.L7:                              // FLS §6.17: branch target
    mov     x0, #0              // FLS §4.4: unit return
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn classify — FLS §9
    .global classify
classify:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, lt                    // FLS §6.5.3: x2 = (x0 < x1)
    cbz     x2, .L10                    // FLS §6.17: branch if false
    mov     x3, #1                   // FLS §2.4.4.1: load imm 1
    neg     x4, x3               // FLS §6.5.4: negate x3
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret
    b       .L11                       // FLS §6.17: branch to end
.L10:                              // FLS §6.17: branch target
.L11:                              // FLS §6.17: branch target
    ldr     x5, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x6, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x5, x6               // FLS §6.5.3: compare (signed)
    cset    x7, eq                    // FLS §6.5.3: x7 = (x5 == x6)
    cbz     x7, .L12                    // FLS §6.17: branch if false
    mov     x8, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x8              // FLS §6.19: return reg 8 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret
    b       .L13                       // FLS §6.17: branch to end
.L12:                              // FLS §6.17: branch target
.L13:                              // FLS §6.17: branch target
    mov     x9, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x9              // FLS §6.19: return reg 9 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn return_from_nested_block — FLS §9
    .global return_from_nested_block
return_from_nested_block:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, gt                    // FLS §6.5.3: x2 = (x0 > x1)
    cbz     x2, .L14                    // FLS §6.17: branch if false
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret
    b       .L15                       // FLS §6.17: branch to end
.L14:                              // FLS §6.17: branch target
.L15:                              // FLS §6.17: branch target
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x5, #1                   // FLS §2.4.4.1: load imm 1
    add     x6, x4, x5          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x7, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x7              // FLS §6.19: return reg 7 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn explicit_return_only — FLS §9
    .global explicit_return_only
explicit_return_only:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #3                   // FLS §2.4.4.1: load imm 3
    mul     x2, x0, x1          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret
    mov     x0, #0              // FLS §4.4: unit return
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #64             // FLS §8.1: frame for 7 slot(s)
    mov     x0, #1                   // FLS §2.4.4.1: load imm 1
    bl      early_return_taken       // FLS §6.12.1: call early_return_taken
    mov     x1, x0              // FLS §6.12.1: return value → x1
    str     x1, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x2, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x2                   // FLS §6.12.1: arg 0
    bl      early_return_not_taken   // FLS §6.12.1: call early_return_not_taken
    mov     x3, x0              // FLS §6.12.1: return value → x3
    str     x3, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x4                   // FLS §6.12.1: arg 0
    bl      return_unit              // FLS §6.12.1: call return_unit
    mov     x5, x0              // FLS §6.12.1: return value → x5
    mov     x6, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x6                   // FLS §6.12.1: arg 0
    bl      return_unit              // FLS §6.12.1: call return_unit
    mov     x7, x0              // FLS §6.12.1: return value → x7
    mov     x8, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x8                   // FLS §6.12.1: arg 0
    bl      tail_expression_return   // FLS §6.12.1: call tail_expression_return
    mov     x9, x0              // FLS §6.12.1: return value → x9
    str     x9, [sp, #16             ] // FLS §8.1: store slot 2
    mov     x10, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x10                  // FLS §6.12.1: arg 0
    bl      return_from_loop         // FLS §6.12.1: call return_from_loop
    mov     x11, x0              // FLS §6.12.1: return value → x11
    str     x11, [sp, #24             ] // FLS §8.1: store slot 3
    mov     x12, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x12                  // FLS §6.12.1: arg 0
    bl      classify                 // FLS §6.12.1: call classify
    mov     x13, x0              // FLS §6.12.1: return value → x13
    str     x13, [sp, #32             ] // FLS §8.1: store slot 4
    mov     x14, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x14                  // FLS §6.12.1: arg 0
    bl      return_from_nested_block // FLS §6.12.1: call return_from_nested_block
    mov     x15, x0              // FLS §6.12.1: return value → x15
    str     x15, [sp, #40             ] // FLS §8.1: store slot 5
    mov     x16, #2                   // FLS §2.4.4.1: load imm 2
    mov     x0, x16                  // FLS §6.12.1: arg 0
    bl      explicit_return_only     // FLS §6.12.1: call explicit_return_only
    mov     x17, x0              // FLS §6.12.1: return value → x17
    str     x17, [sp, #48             ] // FLS §8.1: store slot 6
    ldr     x18, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x19, [sp, #8              ] // FLS §8.1: load slot 1
    add     x20, x18, x19          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x21, [sp, #16             ] // FLS §8.1: load slot 2
    add     x22, x20, x21          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x23, [sp, #24             ] // FLS §8.1: load slot 3
    add     x24, x22, x23          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x25, [sp, #32             ] // FLS §8.1: load slot 4
    add     x26, x24, x25          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x27, [sp, #40             ] // FLS §8.1: load slot 5
    add     x28, x26, x27          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x29, [sp, #48             ] // FLS §8.1: load slot 6
    add     x30, x28, x29          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x30              // FLS §6.19: return reg 30 → x0
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
