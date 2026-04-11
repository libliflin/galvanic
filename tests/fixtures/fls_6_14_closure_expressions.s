    .text

    // fn apply_no_param — FLS §9
    .global apply_no_param
apply_no_param:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x9, [sp, #0                     ] // FLS §4.9: load fn ptr
    blr     x9                       // FLS §4.9: indirect call
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn closure_zero_params — FLS §9
    .global closure_zero_params
closure_zero_params:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #1                   // FLS §2.4.4.1: load imm 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x2, [sp, #8              ] // FLS §8.1: store slot 1
    adrp    x3, __closure_closure_zero_params_0              // FLS §4.9: fn ptr addr (page)
    add     x3, x3, :lo12:__closure_closure_zero_params_0  // FLS §4.9: fn ptr addr (offset)
    str     x3, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x4, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x4                   // FLS §6.12.1: arg 0
    bl      apply_no_param           // FLS §6.12.1: call apply_no_param
    mov     x5, x0              // FLS §6.12.1: return value → x5
    mov     x0, x5              // FLS §6.19: return reg 5 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn __closure_closure_zero_params_0 — FLS §9
    .global __closure_closure_zero_params_0
__closure_closure_zero_params_0:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn apply_one_param — FLS §9
    .global apply_one_param
apply_one_param:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x9, [sp, #0                     ] // FLS §4.9: load fn ptr
    blr     x9                       // FLS §4.9: indirect call
    mov     x1, x0               // FLS §4.9: capture return
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn closure_one_param — FLS §9
    .global closure_one_param
closure_one_param:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    adrp    x0, __closure_closure_one_param_0              // FLS §4.9: fn ptr addr (page)
    add     x0, x0, :lo12:__closure_closure_one_param_0  // FLS §4.9: fn ptr addr (offset)
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x1                   // FLS §6.12.1: arg 0
    mov     x1, x2                   // FLS §6.12.1: arg 1
    bl      apply_one_param          // FLS §6.12.1: call apply_one_param
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn __closure_closure_one_param_0 — FLS §9
    .global __closure_closure_one_param_0
__closure_closure_one_param_0:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #1                   // FLS §2.4.4.1: load imm 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn apply_two_params — FLS §9
    .global apply_two_params
apply_two_params:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x0, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x1, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x9, [sp, #0                     ] // FLS §4.9: load fn ptr
    blr     x9                       // FLS §4.9: indirect call
    mov     x2, x0               // FLS §4.9: capture return
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn closure_two_params — FLS §9
    .global closure_two_params
closure_two_params:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    adrp    x0, __closure_closure_two_params_0              // FLS §4.9: fn ptr addr (page)
    add     x0, x0, :lo12:__closure_closure_two_params_0  // FLS §4.9: fn ptr addr (offset)
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x1                   // FLS §6.12.1: arg 0
    mov     x1, x2                   // FLS §6.12.1: arg 1
    mov     x2, x3                   // FLS §6.12.1: arg 2
    bl      apply_two_params         // FLS §6.12.1: call apply_two_params
    mov     x4, x0              // FLS §6.12.1: return value → x4
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn __closure_closure_two_params_0 — FLS §9
    .global __closure_closure_two_params_0
__closure_closure_two_params_0:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn closure_typed_params — FLS §9
    .global closure_typed_params
closure_typed_params:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    adrp    x0, __closure_closure_typed_params_0              // FLS §4.9: fn ptr addr (page)
    add     x0, x0, :lo12:__closure_closure_typed_params_0  // FLS §4.9: fn ptr addr (offset)
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x1                   // FLS §6.12.1: arg 0
    mov     x1, x2                   // FLS §6.12.1: arg 1
    bl      apply_one_param          // FLS §6.12.1: call apply_one_param
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn __closure_closure_typed_params_0 — FLS §9
    .global __closure_closure_typed_params_0
__closure_closure_typed_params_0:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #2                   // FLS §2.4.4.1: load imm 2
    mul     x2, x0, x1          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn closure_block_body — FLS §9
    .global closure_block_body
closure_block_body:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    adrp    x0, __closure_closure_block_body_0              // FLS §4.9: fn ptr addr (page)
    add     x0, x0, :lo12:__closure_closure_block_body_0  // FLS §4.9: fn ptr addr (offset)
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x1                   // FLS §6.12.1: arg 0
    mov     x1, x2                   // FLS §6.12.1: arg 1
    bl      apply_one_param          // FLS §6.12.1: call apply_one_param
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn __closure_closure_block_body_0 — FLS §9
    .global __closure_closure_block_body_0
__closure_closure_block_body_0:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #1                   // FLS §2.4.4.1: load imm 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    str     x2, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x4, #2                   // FLS §2.4.4.1: load imm 2
    mul     x5, x3, x4          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    mov     x0, x5              // FLS §6.19: return reg 5 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn closure_explicit_return_type — FLS §9
    .global closure_explicit_return_type
closure_explicit_return_type:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    adrp    x0, __closure_closure_explicit_return_type_0              // FLS §4.9: fn ptr addr (page)
    add     x0, x0, :lo12:__closure_closure_explicit_return_type_0  // FLS §4.9: fn ptr addr (offset)
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x1                   // FLS §6.12.1: arg 0
    mov     x1, x2                   // FLS §6.12.1: arg 1
    bl      apply_one_param          // FLS §6.12.1: call apply_one_param
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn __closure_closure_explicit_return_type_0 — FLS §9
    .global __closure_closure_explicit_return_type_0
__closure_closure_explicit_return_type_0:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #10                  // FLS §2.4.4.1: load imm 10
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn closure_captures_local — FLS §9
    .global closure_captures_local
closure_captures_local:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #2                   // FLS §2.4.4.1: load imm 2
    mul     x2, x0, x1          // FLS §6.5.5: mul; §6.23: 64-bit, no i32 wrap
    str     x2, [sp, #16             ] // FLS §8.1: store slot 2
    adrp    x3, __closure_closure_captures_local_0              // FLS §4.9: fn ptr addr (page)
    add     x3, x3, :lo12:__closure_closure_captures_local_0  // FLS §4.9: fn ptr addr (offset)
    str     x3, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x4, [sp, #24             ] // FLS §8.1: load slot 3
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x4                   // FLS §6.12.1: arg 0
    mov     x1, x5                   // FLS §6.12.1: arg 1
    bl      apply_one_param          // FLS §6.12.1: call apply_one_param
    mov     x6, x0              // FLS §6.12.1: return value → x6
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn __closure_closure_captures_local_0 — FLS §9
    .global __closure_closure_captures_local_0
__closure_closure_captures_local_0:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn closure_captures_parameter — FLS §9
    .global closure_captures_parameter
closure_captures_parameter:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    adrp    x0, __closure_closure_captures_parameter_0              // FLS §4.9: fn ptr addr (page)
    add     x0, x0, :lo12:__closure_closure_captures_parameter_0  // FLS §4.9: fn ptr addr (offset)
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x1, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x2, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x1                   // FLS §6.12.1: arg 0
    mov     x1, x2                   // FLS §6.12.1: arg 1
    bl      apply_one_param          // FLS §6.12.1: call apply_one_param
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn __closure_closure_captures_parameter_0 — FLS §9
    .global __closure_closure_captures_parameter_0
__closure_closure_captures_parameter_0:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn apply_move_closure — FLS §9
    .global apply_move_closure
apply_move_closure:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x9, [sp, #0                     ] // FLS §4.9: load fn ptr
    blr     x9                       // FLS §4.9: indirect call
    mov     x1, x0               // FLS §4.9: capture return
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn closure_move — FLS §9
    .global closure_move
closure_move:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    adrp    x1, __closure_closure_move_0              // FLS §4.9: fn ptr addr (page)
    add     x1, x1, :lo12:__closure_closure_move_0  // FLS §4.9: fn ptr addr (offset)
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x2, [sp, #24             ] // FLS §8.1: load slot 3
    ldr     x3, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x2                   // FLS §6.12.1: arg 0
    mov     x1, x3                   // FLS §6.12.1: arg 1
    bl      apply_move_closure       // FLS §6.12.1: call apply_move_closure
    mov     x4, x0              // FLS §6.12.1: return value → x4
    mov     x0, x4              // FLS §6.19: return reg 4 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn __closure_closure_move_0 — FLS §9
    .global __closure_closure_move_0
__closure_closure_move_0:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn closure_inline_argument — FLS §9
    .global closure_inline_argument
closure_inline_argument:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    adrp    x0, __closure_closure_inline_argument_0              // FLS §4.9: fn ptr addr (page)
    add     x0, x0, :lo12:__closure_closure_inline_argument_0  // FLS §4.9: fn ptr addr (offset)
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    bl      apply_one_param          // FLS §6.12.1: call apply_one_param
    mov     x2, x0              // FLS §6.12.1: return value → x2
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn __closure_closure_inline_argument_0 — FLS §9
    .global __closure_closure_inline_argument_0
__closure_closure_inline_argument_0:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #5                   // FLS §2.4.4.1: load imm 5
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn closure_conditional_body — FLS §9
    .global closure_conditional_body
closure_conditional_body:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    adrp    x0, __closure_closure_conditional_body_0              // FLS §4.9: fn ptr addr (page)
    add     x0, x0, :lo12:__closure_closure_conditional_body_0  // FLS §4.9: fn ptr addr (offset)
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x0, x1                   // FLS §6.12.1: arg 0
    mov     x1, x2                   // FLS §6.12.1: arg 1
    bl      apply_one_param          // FLS §6.12.1: call apply_one_param
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn __closure_closure_conditional_body_0 — FLS §9
    .global __closure_closure_conditional_body_0
__closure_closure_conditional_body_0:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x0, x1               // FLS §6.5.3: compare (signed)
    cset    x2, gt                    // FLS §6.5.3: x2 = (x0 > x1)
    cbz     x2, .L0                     // FLS §6.17: branch if false
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    str     x3, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L1                        // FLS §6.17: branch to end
.L0:                              // FLS §6.17: branch target
    ldr     x4, [sp, #0              ] // FLS §8.1: load slot 0
    neg     x5, x4               // FLS §6.5.4: negate x4
    str     x5, [sp, #8              ] // FLS §8.1: store slot 1
.L1:                              // FLS §6.17: branch target
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn two_closures — FLS §9
    .global two_closures
two_closures:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    adrp    x0, __closure_two_closures_0              // FLS §4.9: fn ptr addr (page)
    add     x0, x0, :lo12:__closure_two_closures_0  // FLS §4.9: fn ptr addr (offset)
    str     x0, [sp, #16             ] // FLS §8.1: store slot 2
    adrp    x1, __closure_two_closures_1              // FLS §4.9: fn ptr addr (page)
    add     x1, x1, :lo12:__closure_two_closures_1  // FLS §4.9: fn ptr addr (offset)
    str     x1, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    mov     x4, #0                   // FLS §2.4.4.1: load imm 0
    mov     x0, x3                   // FLS §6.12.1: arg 0
    mov     x1, x4                   // FLS §6.12.1: arg 1
    bl      apply_one_param          // FLS §6.12.1: call apply_one_param
    mov     x5, x0              // FLS §6.12.1: return value → x5
    mov     x0, x2                   // FLS §6.12.1: arg 0
    mov     x1, x5                   // FLS §6.12.1: arg 1
    bl      apply_one_param          // FLS §6.12.1: call apply_one_param
    mov     x6, x0              // FLS §6.12.1: return value → x6
    mov     x0, x6              // FLS §6.19: return reg 6 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn __closure_two_closures_0 — FLS §9
    .global __closure_two_closures_0
__closure_two_closures_0:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn __closure_two_closures_1 — FLS §9
    .global __closure_two_closures_1
__closure_two_closures_1:
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x0, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    add     x2, x0, x1          // FLS §6.5.5: add; §6.23: 64-bit, no i32 wrap
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    mov     x0, #10                  // FLS §2.4.4.1: load imm 10
    bl      closure_zero_params      // FLS §6.12.1: call closure_zero_params
    mov     x1, x0              // FLS §6.12.1: return value → x1
    mov     x2, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x2                   // FLS §6.12.1: arg 0
    bl      closure_one_param        // FLS §6.12.1: call closure_one_param
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x4, #3                   // FLS §2.4.4.1: load imm 3
    mov     x5, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x4                   // FLS §6.12.1: arg 0
    mov     x1, x5                   // FLS §6.12.1: arg 1
    bl      closure_two_params       // FLS §6.12.1: call closure_two_params
    mov     x6, x0              // FLS §6.12.1: return value → x6
    mov     x7, #7                   // FLS §2.4.4.1: load imm 7
    mov     x0, x7                   // FLS §6.12.1: arg 0
    bl      closure_typed_params     // FLS §6.12.1: call closure_typed_params
    mov     x8, x0              // FLS §6.12.1: return value → x8
    mov     x9, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x9                   // FLS §6.12.1: arg 0
    bl      closure_block_body       // FLS §6.12.1: call closure_block_body
    mov     x10, x0              // FLS §6.12.1: return value → x10
    mov     x11, #2                   // FLS §2.4.4.1: load imm 2
    mov     x0, x11                  // FLS §6.12.1: arg 0
    bl      closure_explicit_return_type // FLS §6.12.1: call closure_explicit_return_type
    mov     x12, x0              // FLS §6.12.1: return value → x12
    mov     x13, #5                   // FLS §2.4.4.1: load imm 5
    mov     x14, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x13                  // FLS §6.12.1: arg 0
    mov     x1, x14                  // FLS §6.12.1: arg 1
    bl      closure_captures_local   // FLS §6.12.1: call closure_captures_local
    mov     x15, x0              // FLS §6.12.1: return value → x15
    mov     x16, #10                  // FLS §2.4.4.1: load imm 10
    mov     x17, #20                  // FLS §2.4.4.1: load imm 20
    mov     x0, x16                  // FLS §6.12.1: arg 0
    mov     x1, x17                  // FLS §6.12.1: arg 1
    bl      closure_captures_parameter // FLS §6.12.1: call closure_captures_parameter
    mov     x18, x0              // FLS §6.12.1: return value → x18
    mov     x19, #8                   // FLS §2.4.4.1: load imm 8
    mov     x20, #2                   // FLS §2.4.4.1: load imm 2
    mov     x0, x19                  // FLS §6.12.1: arg 0
    mov     x1, x20                  // FLS §6.12.1: arg 1
    bl      closure_move             // FLS §6.12.1: call closure_move
    mov     x21, x0              // FLS §6.12.1: return value → x21
    mov     x22, #6                   // FLS §2.4.4.1: load imm 6
    mov     x0, x22                  // FLS §6.12.1: arg 0
    bl      closure_inline_argument  // FLS §6.12.1: call closure_inline_argument
    mov     x23, x0              // FLS §6.12.1: return value → x23
    mov     x24, #4                   // FLS §2.4.4.1: load imm 4
    neg     x25, x24               // FLS §6.5.4: negate x24
    mov     x0, x25                  // FLS §6.12.1: arg 0
    bl      closure_conditional_body // FLS §6.12.1: call closure_conditional_body
    mov     x26, x0              // FLS §6.12.1: return value → x26
    mov     x27, #1                   // FLS §2.4.4.1: load imm 1
    mov     x28, #2                   // FLS §2.4.4.1: load imm 2
    mov     x0, x27                  // FLS §6.12.1: arg 0
    mov     x1, x28                  // FLS §6.12.1: arg 1
    bl      two_closures             // FLS §6.12.1: call two_closures
    mov     x29, x0              // FLS §6.12.1: return value → x29
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
