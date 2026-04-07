    .text

    // fn Wrapper__get — FLS §9
    .global Wrapper__get
Wrapper__get:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Counter__value — FLS §9
    .global Counter__value
Counter__value:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Widget__version — FLS §9
    .global Widget__version
Widget__version:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #3                   // FLS §2.4.4.1: load imm 3
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    add     x2, x0, x1          // FLS §6.5.5: add
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    mov     x0, #10                  // FLS §2.4.4.1: load imm 10
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #7                   // FLS §2.4.4.1: load imm 7
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x2, #2                   // FLS §2.4.4.1: load imm 2
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x3                   // FLS §6.12.1: arg 0
    bl      Wrapper__get             // FLS §6.12.1: call Wrapper__get
    mov     x4, x0              // FLS §6.12.1: return value → x4
    str     x4, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x5                   // FLS §6.12.1: arg 0
    bl      Counter__value           // FLS §6.12.1: call Counter__value
    mov     x6, x0              // FLS §6.12.1: return value → x6
    ldr     x7, [sp, #24             ] // FLS §8.1: load slot 3
    add     x8, x7, x6          // FLS §6.5.5: add
    str     x8, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x9                   // FLS §6.12.1: arg 0
    bl      Widget__version          // FLS §6.12.1: call Widget__version
    mov     x10, x0              // FLS §6.12.1: return value → x10
    ldr     x11, [sp, #32             ] // FLS §8.1: load slot 4
    add     x12, x11, x10          // FLS §6.5.5: add
    mov     x0, x12              // FLS §6.19: return reg 12 → x0
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
