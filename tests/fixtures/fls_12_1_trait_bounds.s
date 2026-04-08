    .text

    // fn Foo__scale — FLS §9
    .global Foo__scale
Foo__scale:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mul     x2, x0, x1          // FLS §6.5.5: mul
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #3                   // FLS §2.4.4.1: load imm 3
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x2, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x1                   // FLS §6.12.1: arg 0
    mov     x1, x2                   // FLS §6.12.1: arg 1
    bl      apply_scale__Foo         // FLS §6.12.1: call apply_scale__Foo
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn apply_scale__Foo — FLS §9
    .global apply_scale__Foo
apply_scale__Foo:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    bl      Foo__scale               // FLS §6.12.1: call Foo__scale
    mov     x2, x0              // FLS §6.12.1: return value → x2
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
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
