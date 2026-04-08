    .text

    // fn main — FLS §9
    .global main
main:
    sub     sp, sp, #48             // FLS §8.1: frame for 6 slot(s)
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #7                   // FLS §2.4.4.1: load imm 7
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    str     x2, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, eq                    // FLS §6.5.3: x5 = (x3 == x4)
    cbz     x5, .L1                     // FLS §6.17: branch if false
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    str     x6, [sp, #40             ] // FLS §8.1: store slot 5
    ldr     x7, [sp, #40             ] // FLS §8.1: load slot 5
    str     x7, [sp, #32             ] // FLS §8.1: store slot 4
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    mov     x8, #0                   // FLS §2.4.4.1: load imm 0
    str     x8, [sp, #32             ] // FLS §8.1: store slot 4
.L0:                              // FLS §6.17: branch target
    ldr     x9, [sp, #32             ] // FLS §8.1: load slot 4
    str     x9, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x10, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x10              // FLS §6.19: return reg 10 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
