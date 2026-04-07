    .text

    // fn use_wrapper — FLS §9
    .global use_wrapper
use_wrapper:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #10                  // FLS §2.4.4.1: load imm 10
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x1                   // FLS §6.12.1: arg 0
    mov     x1, x2                   // FLS §6.12.1: arg 1
    bl      Wrapper__apply__i32      // FLS §6.12.1: call Wrapper__apply__i32
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    mov     x0, #3                   // FLS §2.4.4.1: load imm 3
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x2, #7                   // FLS §2.4.4.1: load imm 7
    mov     x0, x1                   // FLS §6.12.1: arg 0
    mov     x1, x2                   // FLS §6.12.1: arg 1
    bl      Wrapper__apply__i32      // FLS §6.12.1: call Wrapper__apply__i32
    mov     x3, x0              // FLS §6.12.1: return value → x3
    str     x3, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x5, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x4                   // FLS §6.12.1: arg 0
    mov     x1, x5                   // FLS §6.12.1: arg 1
    bl      Wrapper__add_val__i32    // FLS §6.12.1: call Wrapper__add_val__i32
    mov     x6, x0              // FLS §6.12.1: return value → x6
    str     x6, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x7, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x8, #1                   // FLS §2.4.4.1: load imm 1
    mov     x9, #99                  // FLS §2.4.4.1: load imm 99
    mov     x0, x7                   // FLS §6.12.1: arg 0
    mov     x1, x8                   // FLS §6.12.1: arg 1
    mov     x2, x9                   // FLS §6.12.1: arg 2
    bl      Wrapper__pick_first__i32_i32 // FLS §6.12.1: call Wrapper__pick_first__i32_i32
    mov     x10, x0              // FLS §6.12.1: return value → x10
    str     x10, [sp, #24             ] // FLS §8.1: store slot 3
    mov     x11, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x11                  // FLS §6.12.1: arg 0
    bl      use_wrapper              // FLS §6.12.1: call use_wrapper
    mov     x12, x0              // FLS §6.12.1: return value → x12
    str     x12, [sp, #32             ] // FLS §8.1: store slot 4
    mov     x0, #0              // FLS §4.4: unit return
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn Wrapper__apply__i32 — FLS §9
    .global Wrapper__apply__i32
Wrapper__apply__i32:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #8              ] // FLS §8.1: load slot 1
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Wrapper__add_val__i32 — FLS §9
    .global Wrapper__add_val__i32
Wrapper__add_val__i32:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    add     x2, x0, x1          // FLS §6.5.5: add
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Wrapper__pick_first__i32_i32 — FLS §9
    .global Wrapper__pick_first__i32_i32
Wrapper__pick_first__i32_i32:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x0, [sp, #8              ] // FLS §8.1: load slot 1
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
