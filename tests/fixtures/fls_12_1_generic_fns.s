    .text

    // fn add_one — FLS §9
    .global add_one
add_one:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    bl      identity__i32            // FLS §6.12.1: call identity__i32
    mov     x1, x0              // FLS §6.12.1: return value → x1
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    add     x3, x1, x2          // FLS §6.5.5: add
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    mov     x0, #42                  // FLS §2.4.4.1: load imm 42
    bl      identity__i32            // FLS §6.12.1: call identity__i32
    mov     x1, x0              // FLS §6.12.1: return value → x1
    str     x1, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x2, #10                  // FLS §2.4.4.1: load imm 10
    mov     x3, #20                  // FLS §2.4.4.1: load imm 20
    mov     x0, x2                   // FLS §6.12.1: arg 0
    mov     x1, x3                   // FLS §6.12.1: arg 1
    bl      first__i32               // FLS §6.12.1: call first__i32
    mov     x4, x0              // FLS §6.12.1: return value → x4
    str     x4, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x5, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x5                   // FLS §6.12.1: arg 0
    bl      add_one                  // FLS §6.12.1: call add_one
    mov     x6, x0              // FLS §6.12.1: return value → x6
    str     x6, [sp, #16             ] // FLS §8.1: store slot 2
    mov     x0, #0              // FLS §4.4: unit return
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn identity__i32 — FLS §9
    .global identity__i32
identity__i32:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn first__i32 — FLS §9
    .global first__i32
first__i32:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
