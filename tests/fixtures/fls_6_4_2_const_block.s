    .text

    // fn demonstrate_const_block — FLS §9
    .global demonstrate_const_block
demonstrate_const_block:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    mov     x0, #5                   // FLS §2.4.4.1: load imm 5
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #42                  // FLS §2.4.4.1: load imm 42
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x2, #40                  // FLS §2.4.4.1: load imm 40
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x4, [sp, #8              ] // FLS §8.1: load slot 1
    add     x5, x3, x4          // FLS §6.5.5: add
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    add     x7, x5, x6          // FLS §6.5.5: add
    mov     x0, x7              // FLS §6.19: return reg 7 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    bl      demonstrate_const_block  // FLS §6.12.1: call demonstrate_const_block
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
