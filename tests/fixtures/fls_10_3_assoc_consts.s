    .text

    // fn use_assoc_consts — FLS §9
    .global use_assoc_consts
use_assoc_consts:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    mov     x0, #256                 // FLS §2.4.4.1: load imm 256
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #1                   // FLS §2.4.4.1: load imm 1
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    mov     x3, #2                   // FLS §2.4.4.1: load imm 2
    str     x3, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    sub     x6, x4, x5          // FLS §6.5.5: sub
    ldr     x7, [sp, #16             ] // FLS §8.1: load slot 2
    add     x8, x6, x7          // FLS §6.5.5: add
    ldr     x9, [sp, #24             ] // FLS §8.1: load slot 3
    add     x10, x8, x9          // FLS §6.5.5: add
    mov     x0, x10              // FLS §6.19: return reg 10 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    bl      use_assoc_consts         // FLS §6.12.1: call use_assoc_consts
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
