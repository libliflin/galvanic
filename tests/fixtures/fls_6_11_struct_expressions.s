    .text

    // fn named_fields — FLS §9
    .global named_fields
named_fields:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    add     x4, x2, x3          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn shorthand_init — FLS §9
    .global shorthand_init
shorthand_init:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    add     x4, x2, x3          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn update_syntax — FLS §9
    .global update_syntax
update_syntax:
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x0, [sp, #16             ] // FLS §8.1: load slot 2
    str     x0, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x2, [sp, #24             ] // FLS §8.1: load slot 3
    ldr     x3, [sp, #32             ] // FLS §8.1: load slot 4
    add     x4, x2, x3          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn update_no_overrides — FLS §9
    .global update_no_overrides
update_no_overrides:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    add     x4, x2, x3          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn unit_struct_expr — FLS §9
    .global unit_struct_expr
unit_struct_expr:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn tuple_struct_expr — FLS §9
    .global tuple_struct_expr
tuple_struct_expr:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    add     x4, x2, x3          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn nested_struct_expr — FLS §9
    .global nested_struct_expr
nested_struct_expr:
    sub     sp, sp, #64             // FLS §8.1: frame for 8 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    str     x3, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    str     x1, [sp, #40             ] // FLS §8.1: store slot 5
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    str     x2, [sp, #48             ] // FLS §8.1: store slot 6
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    str     x3, [sp, #56             ] // FLS §8.1: store slot 7
    ldr     x4, [sp, #48             ] // FLS §8.1: load slot 6
    ldr     x5, [sp, #32             ] // FLS §8.1: load slot 4
    sub     x6, x4, x5          // FLS §6.5.5: sub; §6.23: 64-bit, no i32 wrap
    ldr     x7, [sp, #56             ] // FLS §8.1: load slot 7
    ldr     x8, [sp, #40             ] // FLS §8.1: load slot 5
    sub     x9, x7, x8          // FLS §6.5.5: sub; §6.23: 64-bit, no i32 wrap
    add     x10, x6, x9          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x10              // FLS §6.19: return reg 10 → x0
    add     sp, sp, #64             // FLS §8.1: restore stack frame
    ret

    // fn complex_field_values — FLS §9
    .global complex_field_values
complex_field_values:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #2                   // FLS §2.4.4.1: load imm 2
    mul     x2, x0, x1          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x4, #1                   // FLS §2.4.4.1: load imm 1
    add     x5, x3, x4          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x5, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x7, [sp, #24             ] // FLS §8.1: load slot 3
    add     x8, x6, x7          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x8              // FLS §6.19: return reg 8 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn mixed_shorthand — FLS §9
    .global mixed_shorthand
mixed_shorthand:
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x2, #1                   // FLS §2.4.4.1: load imm 1
    sub     x3, x1, x2          // FLS §6.5.5: sub; §6.23: 64-bit, no i32 wrap
    str     x3, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x4, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x5, [sp, #24             ] // FLS §8.1: load slot 3
    add     x6, x4, x5          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    mov     x0, #3                   // FLS §2.4.4.1: load imm 3
    mov     x1, #4                   // FLS §2.4.4.1: load imm 4
    bl      named_fields             // FLS §6.12.1: call named_fields
    mov     x2, x0              // FLS §6.12.1: return value → x2
    mov     x3, #3                   // FLS §2.4.4.1: load imm 3
    mov     x4, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x3                   // FLS §6.12.1: arg 0
    mov     x1, x4                   // FLS §6.12.1: arg 1
    bl      shorthand_init           // FLS §6.12.1: call shorthand_init
    mov     x5, x0              // FLS §6.12.1: return value → x5
    mov     x6, #10                  // FLS §2.4.4.1: load imm 10
    mov     x7, #20                  // FLS §2.4.4.1: load imm 20
    mov     x8, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x6                   // FLS §6.12.1: arg 0
    mov     x1, x7                   // FLS §6.12.1: arg 1
    mov     x2, x8                   // FLS §6.12.1: arg 2
    bl      update_syntax            // FLS §6.12.1: call update_syntax
    mov     x9, x0              // FLS §6.12.1: return value → x9
    mov     x10, #7                   // FLS §2.4.4.1: load imm 7
    mov     x11, #8                   // FLS §2.4.4.1: load imm 8
    mov     x0, x10                  // FLS §6.12.1: arg 0
    mov     x1, x11                  // FLS §6.12.1: arg 1
    bl      update_no_overrides      // FLS §6.12.1: call update_no_overrides
    mov     x12, x0              // FLS §6.12.1: return value → x12
    bl      unit_struct_expr         // FLS §6.12.1: call unit_struct_expr
    mov     x13, x0              // FLS §6.12.1: return value → x13
    mov     x14, #1                   // FLS §2.4.4.1: load imm 1
    mov     x15, #2                   // FLS §2.4.4.1: load imm 2
    mov     x0, x14                  // FLS §6.12.1: arg 0
    mov     x1, x15                  // FLS §6.12.1: arg 1
    bl      tuple_struct_expr        // FLS §6.12.1: call tuple_struct_expr
    mov     x16, x0              // FLS §6.12.1: return value → x16
    mov     x17, #0                   // FLS §2.4.4.1: load imm 0
    mov     x18, #0                   // FLS §2.4.4.1: load imm 0
    mov     x19, #3                   // FLS §2.4.4.1: load imm 3
    mov     x20, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x17                  // FLS §6.12.1: arg 0
    mov     x1, x18                  // FLS §6.12.1: arg 1
    mov     x2, x19                  // FLS §6.12.1: arg 2
    mov     x3, x20                  // FLS §6.12.1: arg 3
    bl      nested_struct_expr       // FLS §6.12.1: call nested_struct_expr
    mov     x21, x0              // FLS §6.12.1: return value → x21
    mov     x22, #5                   // FLS §2.4.4.1: load imm 5
    mov     x23, #6                   // FLS §2.4.4.1: load imm 6
    mov     x0, x22                  // FLS §6.12.1: arg 0
    mov     x1, x23                  // FLS §6.12.1: arg 1
    bl      complex_field_values     // FLS §6.12.1: call complex_field_values
    mov     x24, x0              // FLS §6.12.1: return value → x24
    mov     x25, #3                   // FLS §2.4.4.1: load imm 3
    mov     x26, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x25                  // FLS §6.12.1: arg 0
    mov     x1, x26                  // FLS §6.12.1: arg 1
    bl      mixed_shorthand          // FLS §6.12.1: call mixed_shorthand
    mov     x27, x0              // FLS §6.12.1: return value → x27
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
