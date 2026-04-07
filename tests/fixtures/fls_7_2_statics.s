    .text

    // fn get_capacity — FLS §9
    .global get_capacity
get_capacity:
    adrp    x0, CAPACITY              // FLS §7.2: static addr (page)
    add     x0, x0, :lo12:CAPACITY  // FLS §7.2: static addr (offset)
    ldr     x0, [x0]             // FLS §7.2: static load
    ret

    // fn sum_sides — FLS §9
    .global sum_sides
sum_sides:
    adrp    x0, SIDE_A              // FLS §7.2: static addr (page)
    add     x0, x0, :lo12:SIDE_A  // FLS §7.2: static addr (offset)
    ldr     x0, [x0]             // FLS §7.2: static load
    adrp    x1, SIDE_B              // FLS §7.2: static addr (page)
    add     x1, x1, :lo12:SIDE_B  // FLS §7.2: static addr (offset)
    ldr     x1, [x1]             // FLS §7.2: static load
    add     x2, x0, x1          // FLS §6.5.5: add
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    ret

    // fn count_to_capacity — FLS §9
    .global count_to_capacity
count_to_capacity:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    adrp    x0, INITIAL              // FLS §7.2: static addr (page)
    add     x0, x0, :lo12:INITIAL  // FLS §7.2: static addr (offset)
    ldr     x0, [x0]             // FLS §7.2: static load
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
.L0:                              // FLS §6.17: branch target
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    adrp    x2, CAPACITY              // FLS §7.2: static addr (page)
    add     x2, x2, :lo12:CAPACITY  // FLS §7.2: static addr (offset)
    ldr     x2, [x2]             // FLS §7.2: static load
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

    // fn get_gravity — FLS §9
    .global get_gravity
get_gravity:
    adrp    x17, GRAVITY              // FLS §7.2: f64 static addr (page)
    add     x17, x17, :lo12:GRAVITY  // FLS §7.2: f64 static addr (offset)
    ldr     d0, [x17]             // FLS §7.2, §4.2: load f64 static
    fcvtzs  w1, d0              // FLS §6.5.9: f64→i32 truncate
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    ret

    // fn scale_plus_one — FLS §9
    .global scale_plus_one
scale_plus_one:
    adrp    x17, SCALE_F32              // FLS §7.2: f32 static addr (page)
    add     x17, x17, :lo12:SCALE_F32  // FLS §7.2: f32 static addr (offset)
    ldr     s0, [x17]             // FLS §7.2, §4.2: load f32 static
    adrp    x17, scale_plus_one__f32c0              // FLS §2.4.4.2: f32 const addr (page)
    add     x17, x17, :lo12:scale_plus_one__f32c0  // FLS §2.4.4.2: f32 const addr (offset)
    ldr     s1, [x17]             // FLS §2.4.4.2: load f32 into s1
    fadd    s2, s0, s1           // FLS §6.5.5: f32 fadd
    fcvtzs  w3, s2              // FLS §6.5.9: f32→i32 truncate
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    bl      get_capacity             // FLS §6.12.1: call get_capacity
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    bl      sum_sides                // FLS §6.12.1: call sum_sides
    mov     x1, x0              // FLS §6.12.1: return value → x1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    add     x3, x2, x1          // FLS §6.5.5: add
    str     x3, [sp, #8              ] // FLS §8.1: store slot 1
    bl      count_to_capacity        // FLS §6.12.1: call count_to_capacity
    mov     x4, x0              // FLS §6.12.1: return value → x4
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    sub     x6, x5, x4          // FLS §6.5.5: sub
    str     x6, [sp, #16             ] // FLS §8.1: store slot 2
    bl      get_gravity              // FLS §6.12.1: call get_gravity
    mov     x7, x0              // FLS §6.12.1: return value → x7
    ldr     x8, [sp, #16             ] // FLS §8.1: load slot 2
    add     x9, x8, x7          // FLS §6.5.5: add
    str     x9, [sp, #24             ] // FLS §8.1: store slot 3
    bl      scale_plus_one           // FLS §6.12.1: call scale_plus_one
    mov     x10, x0              // FLS §6.12.1: return value → x10
    ldr     x11, [sp, #24             ] // FLS §8.1: load slot 3
    sub     x12, x11, x10          // FLS §6.5.5: sub
    mov     x0, x12              // FLS §6.19: return reg 12 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)

    .data
    .align 3
    .global CAPACITY
CAPACITY:
    .quad 10
    .align 3
    .global INITIAL
INITIAL:
    .quad 0
    .align 3
    .global SIDE_A
SIDE_A:
    .quad 3
    .align 3
    .global SIDE_B
SIDE_B:
    .quad 4
    .align 3
    .global GRAVITY
GRAVITY:
    .quad 0x4022000000000000          // f64 9 (FLS §7.2, §4.2)
    .align 2
    .global SCALE_F32
SCALE_F32:
    .word 0x00000000          // f32 0 (FLS §7.2, §4.2)

    .section .rodata
    .align 2
scale_plus_one__f32c0:
    .word 0x3f800000            // f32 1 (FLS §2.4.4.2)
