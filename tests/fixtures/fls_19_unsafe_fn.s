    .text

    // fn double — FLS §9
    .global double
double:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #2                   // FLS §2.4.4.1: load imm 2
    mul     x2, x0, x1          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn add — FLS §9
    .global add
add:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn classify — FLS §9
    .global classify
classify:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
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
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x5, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x4, x5               // FLS §6.5.3: compare (signed)
    cset    x6, lt                    // FLS §6.5.3: x6 = (x4 < x5)
    cbz     x6, .L2                     // FLS §6.17: branch if false
    mov     x7, #2                   // FLS §2.4.4.1: load imm 2
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    mov     x8, #0                   // FLS §2.4.4.1: load imm 0
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
.L3:                              // FLS §6.17: branch target
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    str     x9, [sp, #8              ] // FLS §8.1: store slot 1
.L1:                              // FLS §6.17: branch target
    ldr     x10, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x10              // FLS §6.19: return reg 10 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn quad — FLS §9
    .global quad
quad:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    bl      double                   // FLS §6.12.1: call double
    mov     x1, x0              // FLS §6.12.1: return value → x1
    mov     x0, x1                   // FLS §6.12.1: arg 0
    bl      double                   // FLS §6.12.1: call double
    mov     x2, x0              // FLS §6.12.1: return value → x2
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn safe_double — FLS §9
    .global safe_double
safe_double:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    bl      double                   // FLS §6.12.1: call double
    mov     x1, x0              // FLS §6.12.1: return value → x1
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn safe_add — FLS §9
    .global safe_add
safe_add:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    bl      add                      // FLS §6.12.1: call add
    mov     x2, x0              // FLS §6.12.1: return value → x2
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #48             // FLS §8.1: frame for 6 slot(s)
    mov     x0, #3                   // FLS §2.4.4.1: load imm 3
    bl      double                   // FLS §6.12.1: call double
    mov     x1, x0              // FLS §6.12.1: return value → x1
    str     x1, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x2, #2                   // FLS §2.4.4.1: load imm 2
    mov     x3, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x2                   // FLS §6.12.1: arg 0
    mov     x1, x3                   // FLS §6.12.1: arg 1
    bl      add                      // FLS §6.12.1: call add
    mov     x4, x0              // FLS §6.12.1: return value → x4
    str     x4, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x5, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x5                   // FLS §6.12.1: arg 0
    bl      classify                 // FLS §6.12.1: call classify
    mov     x6, x0              // FLS §6.12.1: return value → x6
    str     x6, [sp, #16             ] // FLS §8.1: store slot 2
    mov     x7, #2                   // FLS §2.4.4.1: load imm 2
    mov     x0, x7                   // FLS §6.12.1: arg 0
    bl      quad                     // FLS §6.12.1: call quad
    mov     x8, x0              // FLS §6.12.1: return value → x8
    str     x8, [sp, #24             ] // FLS §8.1: store slot 3
    mov     x9, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x9                   // FLS §6.12.1: arg 0
    bl      safe_double              // FLS §6.12.1: call safe_double
    mov     x10, x0              // FLS §6.12.1: return value → x10
    str     x10, [sp, #32             ] // FLS §8.1: store slot 4
    mov     x11, #3                   // FLS §2.4.4.1: load imm 3
    mov     x12, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x11                  // FLS §6.12.1: arg 0
    mov     x1, x12                  // FLS §6.12.1: arg 1
    bl      safe_add                 // FLS §6.12.1: call safe_add
    mov     x13, x0              // FLS §6.12.1: return value → x13
    str     x13, [sp, #40             ] // FLS §8.1: store slot 5
    ldr     x14, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x15, [sp, #8              ] // FLS §8.1: load slot 1
    add     x16, x14, x15          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x17, [sp, #16             ] // FLS §8.1: load slot 2
    add     x18, x16, x17          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x19, [sp, #24             ] // FLS §8.1: load slot 3
    add     x20, x18, x19          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x21, [sp, #32             ] // FLS §8.1: load slot 4
    add     x22, x20, x21          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x23, [sp, #40             ] // FLS §8.1: load slot 5
    add     x24, x22, x23          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x24              // FLS §6.19: return reg 24 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
