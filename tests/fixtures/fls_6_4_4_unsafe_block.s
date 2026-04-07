    .text

    // fn add — FLS §9
    .global add
add:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    add     x2, x0, x1          // FLS §6.5.5: add
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #3                   // FLS §2.4.4.1: load imm 3
    mov     x1, #4                   // FLS §2.4.4.1: load imm 4
    add     x2, x0, x1          // FLS §6.5.5: add
    str     x2, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x4, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x3                   // FLS §6.12.1: arg 0
    mov     x1, x4                   // FLS §6.12.1: arg 1
    bl      add                      // FLS §6.12.1: call add
    mov     x5, x0              // FLS §6.12.1: return value → x5
    mov     x0, x5              // FLS §6.19: return reg 5 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
