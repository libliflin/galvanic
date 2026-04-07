    .text

    // fn use_consts — FLS §9
    .global use_consts
use_consts:
    mov     x0, #10                  // FLS §2.4.4.1: load imm 10
    ret

    // fn pythag_sum — FLS §9
    .global pythag_sum
pythag_sum:
    mov     x0, #3                   // FLS §2.4.4.1: load imm 3
    mov     x1, #4                   // FLS §2.4.4.1: load imm 4
    add     x2, x0, x1          // FLS §6.5.5: add
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    ret

    // fn count_to_max — FLS §9
    .global count_to_max
count_to_max:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
.L0:                              // FLS §6.17: branch target
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x2, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, lt                    // FLS §6.5.3: x3 = (x1 < x2)
    cbz     x3, .L1                     // FLS §6.17: branch if false
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x5, #1                   // FLS §2.4.4.1: load imm 1
    add     x6, x4, x5          // FLS §6.5.5: add
    str     x6, [sp, #0              ] // FLS §8.1: store slot 0
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    bl      use_consts               // FLS §6.12.1: call use_consts
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    bl      pythag_sum               // FLS §6.12.1: call pythag_sum
    mov     x1, x0              // FLS §6.12.1: return value → x1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    add     x3, x2, x1          // FLS §6.5.5: add
    str     x3, [sp, #8              ] // FLS §8.1: store slot 1
    bl      count_to_max             // FLS §6.12.1: call count_to_max
    mov     x4, x0              // FLS §6.12.1: return value → x4
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    sub     x6, x5, x4          // FLS §6.5.5: sub
    mov     x7, #65536               // FLS §2.4.4.1: load imm 65536
    add     x8, x6, x7          // FLS §6.5.5: add
    mov     x9, #65536               // FLS §2.4.4.1: load imm 65536
    sub     x10, x8, x9          // FLS §6.5.5: sub
    mov     x11, #5                   // FLS §2.4.4.1: load imm 5
    add     x12, x10, x11          // FLS §6.5.5: add
    mov     x13, #5                   // FLS §2.4.4.1: load imm 5
    sub     x14, x12, x13          // FLS §6.5.5: sub
    mov     x0, x14              // FLS §6.19: return reg 14 → x0
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
