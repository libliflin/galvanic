    .text

    // fn main — FLS §9
    .global main
main:
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    mov     x0, #42                  // FLS §2.4.4.1: load imm 42
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #3                   // FLS §2.4.4.1: load imm 3
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x2, #7                   // FLS §2.4.4.1: load imm 7
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    mov     x3, #1                   // FLS §2.4.4.1: load imm 1
    str     x3, [sp, #24             ] // FLS §8.1: store slot 3
    mov     x4, #10                  // FLS §2.4.4.1: load imm 10
    str     x4, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x5, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    add     x7, x5, x6          // FLS §6.5.5: add
    ldr     x8, [sp, #16             ] // FLS §8.1: load slot 2
    add     x9, x7, x8          // FLS §6.5.5: add
    ldr     x10, [sp, #24             ] // FLS §8.1: load slot 3
    add     x11, x9, x10          // FLS §6.5.5: add
    ldr     x12, [sp, #32             ] // FLS §8.1: load slot 4
    add     x13, x11, x12          // FLS §6.5.5: add
    mov     x0, x13              // FLS §6.19: return reg 13 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
