    .text

    // fn get_some — FLS §9
    .global get_some
get_some:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x0, [sp, #8               ] // FLS §10.1: write-back field 0
    ldr     x1, [sp, #16              ] // FLS §10.1: write-back field 1
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn get_none — FLS §9
    .global get_none
get_none:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    mov     x0, #1                   // FLS §2.4.4.1: load imm 1
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0               ] // FLS §10.1: write-back field 0
    ldr     x1, [sp, #8               ] // FLS §10.1: write-back field 1
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn extract — FLS §9
    .global extract
extract:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, eq                    // FLS §6.5.3: x3 = (x1 == x2)
    cbz     x3, .L0                     // FLS §6.17: branch if false
    ldr     x4, [sp, #8              ] // FLS §8.1: load slot 1
    str     x4, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L1                        // FLS §6.17: branch to end
.L0:                              // FLS §6.17: branch target
    mov     x5, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x5              // FLS §6.19: return reg 5 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret
.L1:                              // FLS §6.17: branch target
    ldr     x6, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    mov     x0, #7                   // FLS §2.4.4.1: load imm 7
    bl      get_some                 // FLS §6.12.2: call &mut self get_some
    str     x0, [sp, #0               ] // FLS §10.1: write-back field 0
    str     x1, [sp, #8               ] // FLS §10.1: write-back field 1
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x3, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x2, x3               // FLS §6.5.3: compare (signed)
    cset    x4, eq                    // FLS §6.5.3: x4 = (x2 == x3)
    cbz     x4, .L2                     // FLS §6.17: branch if false
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L3                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    mov     x6, #1                   // FLS §2.4.4.1: load imm 1
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret
.L3:                              // FLS §6.17: branch target
    ldr     x7, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x8, #7                   // FLS §2.4.4.1: load imm 7
    sub     x9, x7, x8          // FLS §6.5.5: sub; §6.23: 64-bit, no i32 wrap
    mov     x0, x9              // FLS §6.19: return reg 9 → x0
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
