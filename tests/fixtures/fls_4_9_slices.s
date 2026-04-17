    .text

    // fn slice_len — FLS §9
    .global slice_len
slice_len:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #8              ] // FLS §8.1: load slot 1
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn slice_sum — FLS §9
    .global slice_sum
slice_sum:
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
.L0:                              // FLS §6.17: branch target
    ldr     x2, [sp, #24             ] // FLS §8.1: load slot 3
    str     x2, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x4, [sp, #32             ] // FLS §8.1: load slot 4
    cmp     x4, x3               // FLS §6.5.3: compare (signed)
    cset    x5, lt                    // FLS §6.5.3: x5 = (x4 < x3)
    cbz     x5, .L1                     // FLS §6.17: branch if false
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x7, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x8, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x9, #8                   // FLS §2.4.4.1: load imm 8
    mul     x10, x8, x9          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    add     x11, x7, x10          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x12, [x11]           // FLS §6.5.2: deref pointer in x11
    add     x13, x6, x12          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x13, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x14, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x15, #1                   // FLS §2.4.4.1: load imm 1
    add     x16, x14, x15          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x16, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn slice_first — FLS §9
    .global slice_first
slice_first:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    mov     x2, #8                   // FLS §2.4.4.1: load imm 8
    mul     x3, x1, x2          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    add     x4, x0, x3          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    ldr     x5, [x4]           // FLS §6.5.2: deref pointer in x4
    mov     x0, x5              // FLS §6.19: return reg 5 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #80             // FLS §8.1: frame for 9 slot(s)
    mov     x0, #10                  // FLS §2.4.4.1: load imm 10
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x1, #20                  // FLS §2.4.4.1: load imm 20
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x2, #30                  // FLS §2.4.4.1: load imm 30
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    add     x3, sp, #0                   // FLS §6.5.1: address of stack slot 0
    mov     x4, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x3                   // FLS §6.12.1: arg 0
    mov     x1, x4                   // FLS §6.12.1: arg 1
    bl      slice_len                // FLS §6.12.1: call slice_len
    mov     x5, x0              // FLS §6.12.1: return value → x5
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
    add     x6, sp, #0                   // FLS §6.5.1: address of stack slot 0
    mov     x7, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x6                   // FLS §6.12.1: arg 0
    mov     x1, x7                   // FLS §6.12.1: arg 1
    bl      slice_sum                // FLS §6.12.1: call slice_sum
    mov     x8, x0              // FLS §6.12.1: return value → x8
    str     x8, [sp, #32             ] // FLS §8.1: store slot 4
    add     x9, sp, #0                   // FLS §6.5.1: address of stack slot 0
    mov     x10, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x9                   // FLS §6.12.1: arg 0
    mov     x1, x10                  // FLS §6.12.1: arg 1
    bl      slice_first              // FLS §6.12.1: call slice_first
    mov     x11, x0              // FLS §6.12.1: return value → x11
    str     x11, [sp, #40             ] // FLS §8.1: store slot 5
    ldr     x12, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x13, #3                   // FLS §2.4.4.1: load imm 3
    cmp     x12, x13               // FLS §6.5.3: compare (signed)
    cset    x14, eq                    // FLS §6.5.3: x14 = (x12 == x13)
    cbz     x14, .L2                     // FLS §6.17: branch if false
    ldr     x15, [sp, #32             ] // FLS §8.1: load slot 4
    mov     x16, #60                  // FLS §2.4.4.1: load imm 60
    cmp     x15, x16               // FLS §6.5.3: compare (signed)
    cset    x17, eq                    // FLS §6.5.3: x17 = (x15 == x16)
    cbz     x17, .L4                     // FLS §6.17: branch if false
    ldr     x18, [sp, #40             ] // FLS §8.1: load slot 5
    mov     x19, #10                  // FLS §2.4.4.1: load imm 10
    cmp     x18, x19               // FLS §6.5.3: compare (signed)
    cset    x20, eq                    // FLS §6.5.3: x20 = (x18 == x19)
    cbz     x20, .L6                     // FLS §6.17: branch if false
    mov     x21, #0                   // FLS §2.4.4.1: load imm 0
    str     x21, [sp, #64             ] // FLS §8.1: store slot 8
    b       .L7                        // FLS §6.17: branch to end
.L6:                              // FLS §6.17: branch target
    mov     x22, #3                   // FLS §2.4.4.1: load imm 3
    str     x22, [sp, #64             ] // FLS §8.1: store slot 8
.L7:                              // FLS §6.17: branch target
    ldr     x23, [sp, #64             ] // FLS §8.1: load slot 8
    str     x23, [sp, #56             ] // FLS §8.1: store slot 7
    b       .L5                        // FLS §6.17: branch to end
.L4:                              // FLS §6.17: branch target
    mov     x24, #2                   // FLS §2.4.4.1: load imm 2
    str     x24, [sp, #56             ] // FLS §8.1: store slot 7
.L5:                              // FLS §6.17: branch target
    ldr     x25, [sp, #56             ] // FLS §8.1: load slot 7
    str     x25, [sp, #48             ] // FLS §8.1: store slot 6
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    mov     x26, #1                   // FLS §2.4.4.1: load imm 1
    str     x26, [sp, #48             ] // FLS §8.1: store slot 6
.L3:                              // FLS §6.17: branch target
    ldr     x27, [sp, #48             ] // FLS §8.1: load slot 6
    mov     x0, x27              // FLS §6.19: return reg 27 → x0
    add     sp, sp, #80             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
