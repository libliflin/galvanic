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

    // fn Bar__get — FLS §9
    .global Bar__get
Bar__get:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    mov     x0, #3                   // FLS §2.4.4.1: load imm 3
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #7                   // FLS §2.4.4.1: load imm 7
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x3, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x2                   // FLS §6.12.1: arg 0
    mov     x1, x3                   // FLS §6.12.1: arg 1
    bl      apply_scale__Foo         // FLS §6.12.1: call apply_scale__Foo
    mov     x4, x0              // FLS §6.12.1: return value → x4
    str     x4, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x5                   // FLS §6.12.1: arg 0
    bl      get_and_scale__Bar       // FLS §6.12.1: call get_and_scale__Bar
    mov     x6, x0              // FLS §6.12.1: return value → x6
    ldr     x7, [sp, #16             ] // FLS §8.1: load slot 2
    add     x8, x7, x6          // FLS §6.5.5: add
    mov     x0, x8              // FLS §6.19: return reg 8 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
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

    // fn get_and_scale__Bar — FLS §9
    .global get_and_scale__Bar
get_and_scale__Bar:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    bl      Bar__get                 // FLS §6.12.1: call Bar__get
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
