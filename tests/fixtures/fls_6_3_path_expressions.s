    .text

    // fn path_simple_ident — FLS §9
    .global path_simple_ident
path_simple_ident:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn path_const_item — FLS §9
    .global path_const_item
path_const_item:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #100                 // FLS §2.4.4.1: load imm 100
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn path_static_item — FLS §9
    .global path_static_item
path_static_item:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    adrp    x1, OFFSET              // FLS §7.2: static addr (page)
    add     x1, x1, :lo12:OFFSET  // FLS §7.2: static addr (offset)
    ldr     x1, [x1]             // FLS §7.2: static load
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn Point__new — FLS §9
    .global Point__new
Point__new:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x0, [sp, #16              ] // FLS §10.1: write-back field 0
    ldr     x1, [sp, #24              ] // FLS §10.1: write-back field 1
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn Point__sum — FLS §9
    .global Point__sum
Point__sum:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn path_assoc_fn — FLS §9
    .global path_assoc_fn
path_assoc_fn:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    bl      Point__new               // FLS §6.12.2: call &mut self Point__new
    str     x0, [sp, #16              ] // FLS §10.1: write-back field 0
    str     x1, [sp, #24              ] // FLS §10.1: write-back field 1
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x0, x2                   // FLS §6.12.1: arg 0
    mov     x1, x3                   // FLS §6.12.1: arg 1
    bl      Point__sum               // FLS §6.12.1: call Point__sum
    mov     x4, x0              // FLS §6.12.1: return value → x4
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn path_enum_variant — FLS §9
    .global path_enum_variant
path_enum_variant:
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, eq                    // FLS §6.5.3: x3 = (x1 == x2)
    cbz     x3, .L1                     // FLS §6.17: branch if false
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    str     x4, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L1:                              // FLS §6.17: branch target
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x6, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x5, x6               // FLS §6.5.3: compare (signed)
    cset    x7, eq                    // FLS §6.5.3: x7 = (x5 == x6)
    cbz     x7, .L2                     // FLS §6.17: branch if false
    mov     x8, #1                   // FLS §2.4.4.1: load imm 1
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L2:                              // FLS §6.17: branch target
    ldr     x9, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x10, #2                   // FLS §2.4.4.1: load imm 2
    cmp     x9, x10               // FLS §6.5.3: compare (signed)
    cset    x11, eq                    // FLS §6.5.3: x11 = (x9 == x10)
    cbz     x11, .L3                     // FLS §6.17: branch if false
    mov     x12, #2                   // FLS §2.4.4.1: load imm 2
    str     x12, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L0                        // FLS §6.17: branch to end
.L3:                              // FLS §6.17: branch target
    mov     x13, #3                   // FLS §2.4.4.1: load imm 3
    str     x13, [sp, #16             ] // FLS §8.1: store slot 2
.L0:                              // FLS §6.17: branch target
    ldr     x14, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x14              // FLS §6.19: return reg 14 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn path_tuple_variant — FLS §9
    .global path_tuple_variant
path_tuple_variant:
    sub     sp, sp, #48             // FLS §8.1: frame for 6 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x2, [sp, #8              ] // FLS §8.1: load slot 1
    str     x2, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, eq                    // FLS §6.5.3: x5 = (x3 == x4)
    cbz     x5, .L5                     // FLS §6.17: branch if false
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    str     x6, [sp, #40             ] // FLS §8.1: store slot 5
    ldr     x7, [sp, #40             ] // FLS §8.1: load slot 5
    str     x7, [sp, #32             ] // FLS §8.1: store slot 4
    b       .L4                        // FLS §6.17: branch to end
.L5:                              // FLS §6.17: branch target
    mov     x8, #0                   // FLS §2.4.4.1: load imm 0
    str     x8, [sp, #32             ] // FLS §8.1: store slot 4
.L4:                              // FLS §6.17: branch target
    ldr     x9, [sp, #32             ] // FLS §8.1: load slot 4
    mov     x0, x9              // FLS §6.19: return reg 9 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn square — FLS §9
    .global square
square:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mul     x2, x0, x1          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn path_fn_item_as_value — FLS §9
    .global path_fn_item_as_value
path_fn_item_as_value:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    adrp    x0, square              // FLS §4.9: fn ptr addr (page)
    add     x0, x0, :lo12:square  // FLS §4.9: fn ptr addr (offset)
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x1                        // FLS §4.9: arg 0
    ldr     x9, [sp, #8                     ] // FLS §4.9: load fn ptr
    blr     x9                       // FLS §4.9: indirect call
    mov     x2, x0               // FLS §4.9: capture return
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn path_multiple_bindings — FLS §9
    .global path_multiple_bindings
path_multiple_bindings:
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    add     x4, x2, x3          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x4, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x5, [sp, #32             ] // FLS §8.1: load slot 4
    mov     x0, x5              // FLS §6.19: return reg 5 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    mov     x0, #42                  // FLS §2.4.4.1: load imm 42
    bl      path_simple_ident        // FLS §6.12.1: call path_simple_ident
    mov     x1, x0              // FLS §6.12.1: return value → x1
    mov     x2, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x2                   // FLS §6.12.1: arg 0
    bl      path_const_item          // FLS §6.12.1: call path_const_item
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x4, #7                   // FLS §2.4.4.1: load imm 7
    mov     x0, x4                   // FLS §6.12.1: arg 0
    bl      path_static_item         // FLS §6.12.1: call path_static_item
    mov     x5, x0              // FLS §6.12.1: return value → x5
    mov     x6, #10                  // FLS §2.4.4.1: load imm 10
    mov     x7, #20                  // FLS §2.4.4.1: load imm 20
    mov     x0, x6                   // FLS §6.12.1: arg 0
    mov     x1, x7                   // FLS §6.12.1: arg 1
    bl      path_assoc_fn            // FLS §6.12.1: call path_assoc_fn
    mov     x8, x0              // FLS §6.12.1: return value → x8
    mov     x9, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x9                   // FLS §6.12.1: arg 0
    bl      path_enum_variant        // FLS §6.12.1: call path_enum_variant
    mov     x10, x0              // FLS §6.12.1: return value → x10
    mov     x11, #99                  // FLS §2.4.4.1: load imm 99
    mov     x0, x11                  // FLS §6.12.1: arg 0
    bl      path_tuple_variant       // FLS §6.12.1: call path_tuple_variant
    mov     x12, x0              // FLS §6.12.1: return value → x12
    mov     x13, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x13                  // FLS §6.12.1: arg 0
    bl      path_fn_item_as_value    // FLS §6.12.1: call path_fn_item_as_value
    mov     x14, x0              // FLS §6.12.1: return value → x14
    mov     x15, #3                   // FLS §2.4.4.1: load imm 3
    mov     x16, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x15                  // FLS §6.12.1: arg 0
    mov     x1, x16                  // FLS §6.12.1: arg 1
    bl      path_multiple_bindings   // FLS §6.12.1: call path_multiple_bindings
    mov     x17, x0              // FLS §6.12.1: return value → x17
    mov     x0, #0              // FLS §4.4: unit return
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
    .global OFFSET
OFFSET:
    .quad 5
