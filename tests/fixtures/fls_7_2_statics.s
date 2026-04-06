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

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
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
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
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
