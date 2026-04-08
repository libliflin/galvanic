    .text

    // fn Wrapper__get_val — FLS §9
    .global Wrapper__get_val
Wrapper__get_val:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Doubler__get_val — FLS §9
    .global Doubler__get_val
Doubler__get_val:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #2                   // FLS §2.4.4.1: load imm 2
    mul     x2, x0, x1          // FLS §6.5.5: mul
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    mov     x0, #7                   // FLS §2.4.4.1: load imm 7
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #5                   // FLS §2.4.4.1: load imm 5
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x2                   // FLS §6.12.1: arg 0
    bl      extract__Wrapper         // FLS §6.12.1: call extract__Wrapper
    mov     x3, x0              // FLS §6.12.1: return value → x3
    str     x3, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x4, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x4                   // FLS §6.12.1: arg 0
    bl      extract__Doubler         // FLS §6.12.1: call extract__Doubler
    mov     x5, x0              // FLS §6.12.1: return value → x5
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    add     x7, x6, x5          // FLS §6.5.5: add
    mov     x0, x7              // FLS §6.19: return reg 7 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn extract__Wrapper — FLS §9
    .global extract__Wrapper
extract__Wrapper:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    bl      Wrapper__get_val         // FLS §6.12.1: call Wrapper__get_val
    mov     x1, x0              // FLS §6.12.1: return value → x1
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn extract__Doubler — FLS §9
    .global extract__Doubler
extract__Doubler:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    bl      Doubler__get_val         // FLS §6.12.1: call Doubler__get_val
    mov     x1, x0              // FLS §6.12.1: return value → x1
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
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
