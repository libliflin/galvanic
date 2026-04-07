    .text

    // fn Square__scaled_area — FLS §9
    .global Square__scaled_area
Square__scaled_area:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mul     x2, x0, x1          // FLS §6.5.5: mul
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    mul     x4, x2, x3          // FLS §6.5.5: mul
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Rectangle__scaled_area — FLS §9
    .global Rectangle__scaled_area
Rectangle__scaled_area:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mul     x2, x0, x1          // FLS §6.5.5: mul
    ldr     x3, [sp, #16             ] // FLS §8.1: load slot 2
    mul     x4, x2, x3          // FLS §6.5.5: mul
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    mov     x0, #3                   // FLS §2.4.4.1: load imm 3
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #4                   // FLS §2.4.4.1: load imm 4
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x2, #5                   // FLS §2.4.4.1: load imm 5
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x4, #2                   // FLS §2.4.4.1: load imm 2
    mov     x0, x3                   // FLS §6.12.1: arg 0
    mov     x1, x4                   // FLS §6.12.1: arg 1
    bl      Square__scaled_area      // FLS §6.12.1: call Square__scaled_area
    mov     x5, x0              // FLS §6.12.1: return value → x5
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x7, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x8, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x6                   // FLS §6.12.1: arg 0
    mov     x1, x7                   // FLS §6.12.1: arg 1
    mov     x2, x8                   // FLS §6.12.1: arg 2
    bl      Rectangle__scaled_area   // FLS §6.12.1: call Rectangle__scaled_area
    mov     x9, x0              // FLS §6.12.1: return value → x9
    ldr     x10, [sp, #24             ] // FLS §8.1: load slot 3
    add     x11, x10, x9          // FLS §6.5.5: add
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
