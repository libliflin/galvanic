    .text

    // fn Box2d__width — FLS §9
    .global Box2d__width
Box2d__width:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Box2d__height — FLS §9
    .global Box2d__height
Box2d__height:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #8              ] // FLS §8.1: load slot 1
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Box2d__describe — FLS §9
    .global Box2d__describe
Box2d__describe:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mul     x2, x0, x1          // FLS §6.5.5: mul
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Box2d__area — FLS §9
    .global Box2d__area
Box2d__area:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mul     x2, x0, x1          // FLS §6.5.5: mul
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Point__describe — FLS §9
    .global Point__describe
Point__describe:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    add     x2, x0, x1          // FLS §6.5.5: add
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Half__value — FLS §9
    .global Half__value
Half__value:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Half__doubled — FLS §9
    .global Half__doubled
Half__doubled:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    bl      Half__value              // FLS §6.12.1: call Half__value
    mov     x1, x0              // FLS §6.12.1: return value → x1
    mov     x2, #2                   // FLS §2.4.4.1: load imm 2
    mul     x3, x1, x2          // FLS §6.5.5: mul
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #64             // FLS §8.1: frame for 8 slot(s)
    mov     x0, #3                   // FLS §2.4.4.1: load imm 3
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #4                   // FLS §2.4.4.1: load imm 4
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    mov     x3, #2                   // FLS §2.4.4.1: load imm 2
    str     x3, [sp, #24             ] // FLS §8.1: store slot 3
    mov     x4, #7                   // FLS §2.4.4.1: load imm 7
    str     x4, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x5, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x5                   // FLS §6.12.1: arg 0
    mov     x1, x6                   // FLS §6.12.1: arg 1
    bl      Box2d__describe          // FLS §6.12.1: call Box2d__describe
    mov     x7, x0              // FLS §6.12.1: return value → x7
    str     x7, [sp, #40             ] // FLS §8.1: store slot 5
    ldr     x8, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x9, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x0, x8                   // FLS §6.12.1: arg 0
    mov     x1, x9                   // FLS §6.12.1: arg 1
    bl      Point__describe          // FLS §6.12.1: call Point__describe
    mov     x10, x0              // FLS §6.12.1: return value → x10
    ldr     x11, [sp, #40             ] // FLS §8.1: load slot 5
    sub     x12, x11, x10          // FLS §6.5.5: sub
    str     x12, [sp, #48             ] // FLS §8.1: store slot 6
    ldr     x13, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x14, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x13                  // FLS §6.12.1: arg 0
    mov     x1, x14                  // FLS §6.12.1: arg 1
    bl      Box2d__area              // FLS §6.12.1: call Box2d__area
    mov     x15, x0              // FLS §6.12.1: return value → x15
    ldr     x16, [sp, #48             ] // FLS §8.1: load slot 6
    sub     x17, x16, x15          // FLS §6.5.5: sub
    mov     x18, #9                   // FLS §2.4.4.1: load imm 9
    add     x19, x17, x18          // FLS §6.5.5: add
    str     x19, [sp, #56             ] // FLS §8.1: store slot 7
    ldr     x20, [sp, #32             ] // FLS §8.1: load slot 4
    mov     x0, x20                  // FLS §6.12.1: arg 0
    bl      Half__doubled            // FLS §6.12.1: call Half__doubled
    mov     x21, x0              // FLS §6.12.1: return value → x21
    ldr     x22, [sp, #56             ] // FLS §8.1: load slot 7
    add     x23, x22, x21          // FLS §6.5.5: add
    mov     x24, #10                  // FLS §2.4.4.1: load imm 10
    sub     x25, x23, x24          // FLS §6.5.5: sub
    mov     x0, x25              // FLS §6.19: return reg 25 → x0
    add     sp, sp, #64             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
