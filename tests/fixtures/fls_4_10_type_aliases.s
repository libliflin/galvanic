    .text

    // fn double — FLS §9
    .global double
double:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #2                   // FLS §2.4.4.1: load imm 2
    mul     x2, x0, x1          // FLS §6.5.5: mul
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn clamp — FLS §9
    .global clamp
clamp:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, lt                    // FLS §6.5.3: x2 = (x0 < x1)
    cbz     x2, .L0                     // FLS §6.17: branch if false
    mov     x3, #0                   // FLS §2.4.4.1: load imm 0
    str     x3, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L1                        // FLS §6.17: branch to end
.L0:                              // FLS §6.17: branch target
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x5, #100                 // FLS §2.4.4.1: load imm 100
    cmp     x4, x5               // FLS §6.5.3: compare (signed)
    cset    x6, gt                    // FLS §6.5.3: x6 = (x4 > x5)
    cbz     x6, .L2                     // FLS §6.17: branch if false
    mov     x7, #100                 // FLS §2.4.4.1: load imm 100
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    ldr     x8, [sp, #0              ] // FLS §8.1: load slot 0
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
.L3:                              // FLS §6.17: branch target
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    str     x9, [sp, #8              ] // FLS §8.1: store slot 1
.L1:                              // FLS §6.17: branch target
    ldr     x10, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x10              // FLS §6.19: return reg 10 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #21                  // FLS §2.4.4.1: load imm 21
    bl      double                   // FLS §6.12.1: call double
    mov     x1, x0              // FLS §6.12.1: return value → x1
    str     x1, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x3, #42                  // FLS §2.4.4.1: load imm 42
    sub     x4, x2, x3          // FLS §6.5.5: sub
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
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
