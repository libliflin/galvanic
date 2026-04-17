    .text

    // fn div_by_zero_param — FLS §9
    .global div_by_zero_param
div_by_zero_param:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    cbz     x1, _galvanic_panic         // FLS §6.23: div-by-zero guard
    movz    x9, #0x8000, lsl #16          // FLS §6.23: x9 = 0x0000_0000_8000_0000
    sxtw    x9, w9                        // FLS §6.23: x9 = 0xFFFF_FFFF_8000_0000 (i32::MIN)
    cmp     x0, x9                    // FLS §6.23: is lhs == i32::MIN?
    b.ne    .Lsdiv_ok_div_by_zero_param_0       // FLS §6.23: lhs ≠ MIN → safe
    cmn     x1, #1                    // FLS §6.23: is rhs == -1? (rhs+1==0)
    b.ne    .Lsdiv_ok_div_by_zero_param_0       // FLS §6.23: rhs ≠ -1 → safe
    b       _galvanic_panic                // FLS §6.23: MIN/-1 overflow → panic
.Lsdiv_ok_div_by_zero_param_0:
    sdiv    x2, x0, x1          // FLS §6.5.5: div (signed)
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn min_div_neg_one — FLS §9
    .global min_div_neg_one
min_div_neg_one:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    cbz     x1, _galvanic_panic         // FLS §6.23: div-by-zero guard
    movz    x9, #0x8000, lsl #16          // FLS §6.23: x9 = 0x0000_0000_8000_0000
    sxtw    x9, w9                        // FLS §6.23: x9 = 0xFFFF_FFFF_8000_0000 (i32::MIN)
    cmp     x0, x9                    // FLS §6.23: is lhs == i32::MIN?
    b.ne    .Lsdiv_ok_min_div_neg_one_0       // FLS §6.23: lhs ≠ MIN → safe
    cmn     x1, #1                    // FLS §6.23: is rhs == -1? (rhs+1==0)
    b.ne    .Lsdiv_ok_min_div_neg_one_0       // FLS §6.23: rhs ≠ -1 → safe
    b       _galvanic_panic                // FLS §6.23: MIN/-1 overflow → panic
.Lsdiv_ok_min_div_neg_one_0:
    sdiv    x2, x0, x1          // FLS §6.5.5: div (signed)
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    mov     x0, #10                  // FLS §2.4.4.1: load imm 10
    mov     x1, #2                   // FLS §2.4.4.1: load imm 2
    bl      div_by_zero_param        // FLS §6.12.1: call div_by_zero_param
    mov     x2, x0              // FLS §6.12.1: return value → x2
    str     x2, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x3, #100                 // FLS §2.4.4.1: load imm 100
    neg     x4, x3               // FLS §6.5.4: negate x3
    mov     x5, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x4                   // FLS §6.12.1: arg 0
    mov     x1, x5                   // FLS §6.12.1: arg 1
    bl      min_div_neg_one          // FLS §6.12.1: call min_div_neg_one
    mov     x6, x0              // FLS §6.12.1: return value → x6
    str     x6, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x7, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    add     x9, x7, x8          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x9              // FLS §6.19: return reg 9 → x0
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

    // FLS §6.23: runtime panic primitive — exit(101)
    .global _galvanic_panic
_galvanic_panic:
    mov     x0, #101        // panic exit code (galvanic sentinel)
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(101) — FLS §6.23: panic
