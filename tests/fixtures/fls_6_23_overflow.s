    .text

    // fn add_large — FLS §9
    .global add_large
add_large:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn sub_from_large — FLS §9
    .global sub_from_large
sub_from_large:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    sub     x2, x0, x1          // FLS §6.5.5: sub; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn mul_large — FLS §9
    .global mul_large
mul_large:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mul     x2, x0, x1          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    movz    x0, #0x4240           // FLS §2.4.4.1: load imm 1000000 (lo16)
    movk    x0, #0x000f, lsl #16  // FLS §2.4.4.1: load imm 1000000 (hi16)
    sxtw    x0, w0               // sign-extend i32 to 64-bit (FLS §2.4.4.1)
    movz    x1, #0x4240           // FLS §2.4.4.1: load imm 1000000 (lo16)
    movk    x1, #0x000f, lsl #16  // FLS §2.4.4.1: load imm 1000000 (hi16)
    sxtw    x1, w1               // sign-extend i32 to 64-bit (FLS §2.4.4.1)
    bl      add_large                // FLS §6.12.1: call add_large
    mov     x2, x0              // FLS §6.12.1: return value → x2
    str     x2, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    movz    x4, #0x41dc           // FLS §2.4.4.1: load imm 999900 (lo16)
    movk    x4, #0x000f, lsl #16  // FLS §2.4.4.1: load imm 999900 (hi16)
    sxtw    x4, w4               // sign-extend i32 to 64-bit (FLS §2.4.4.1)
    mov     x0, x3                   // FLS §6.12.1: arg 0
    mov     x1, x4                   // FLS §6.12.1: arg 1
    bl      sub_from_large           // FLS §6.12.1: call sub_from_large
    mov     x5, x0              // FLS §6.12.1: return value → x5
    str     x5, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x6                   // FLS §6.12.1: arg 0
    mov     x1, x7                   // FLS §6.12.1: arg 1
    bl      mul_large                // FLS §6.12.1: call mul_large
    mov     x8, x0              // FLS §6.12.1: return value → x8
    mov     x0, x8              // FLS §6.19: return reg 8 → x0
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
