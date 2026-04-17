    .text

    // fn Safe__value — FLS §9
    .global Safe__value
Safe__value:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Pair__first — FLS §9
    .global Pair__first
Pair__first:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Pair__second — FLS §9
    .global Pair__second
Pair__second:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #8              ] // FLS §8.1: load slot 1
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn DoubleValue__value — FLS §9
    .global DoubleValue__value
DoubleValue__value:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #2                   // FLS §2.4.4.1: load imm 2
    mul     x2, x0, x1          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #64             // FLS §8.1: frame for 7 slot(s)
    mov     x0, #7                   // FLS §2.4.4.1: load imm 7
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #3                   // FLS §2.4.4.1: load imm 3
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x2, #4                   // FLS §2.4.4.1: load imm 4
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    mov     x3, #5                   // FLS §2.4.4.1: load imm 5
    str     x3, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x4                   // FLS §6.12.1: arg 0
    bl      Safe__value              // FLS §6.12.1: call Safe__value
    mov     x5, x0              // FLS §6.12.1: return value → x5
    str     x5, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x7, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x6                   // FLS §6.12.1: arg 0
    mov     x1, x7                   // FLS §6.12.1: arg 1
    bl      Pair__first              // FLS §6.12.1: call Pair__first
    mov     x8, x0              // FLS §6.12.1: return value → x8
    ldr     x9, [sp, #32             ] // FLS §8.1: load slot 4
    add     x10, x9, x8          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x10, [sp, #40             ] // FLS §8.1: store slot 5
    ldr     x11, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x12, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x11                  // FLS §6.12.1: arg 0
    mov     x1, x12                  // FLS §6.12.1: arg 1
    bl      Pair__second             // FLS §6.12.1: call Pair__second
    mov     x13, x0              // FLS §6.12.1: return value → x13
    ldr     x14, [sp, #40             ] // FLS §8.1: load slot 5
    add     x15, x14, x13          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x15, [sp, #48             ] // FLS §8.1: store slot 6
    ldr     x16, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x0, x16                  // FLS §6.12.1: arg 0
    bl      DoubleValue__value       // FLS §6.12.1: call DoubleValue__value
    mov     x17, x0              // FLS §6.12.1: return value → x17
    ldr     x18, [sp, #48             ] // FLS §8.1: load slot 6
    add     x19, x18, x17          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x19              // FLS §6.19: return reg 19 → x0
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
