    .text

    // fn while_loop — FLS §9
    .global while_loop
while_loop:
    // cache-line: prologue = 1 instr(s) × 4 bytes = 4 bytes — 1 of 16 slots in first cache line
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
.L0:                              // FLS §6.15.3: loop boundary — cache-line: label has zero footprint
    ldr     x2, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    cmp     x2, x3               // FLS §6.5.3: compare (signed)
    cset    x4, lt                    // FLS §6.5.3: x4 = (x2 < x3)
    cbz     x4, .L1                     // FLS §6.15.3: branch if false
    ldr     x5, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    add     x7, x5, x6          // FLS §6.5.5: add
    cmp     x7, w7, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    str     x7, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x9, #1                   // FLS §2.4.4.1: load imm 1
    add     x10, x8, x9          // FLS §6.5.5: add
    cmp     x10, w10, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    str     x10, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L0                        // FLS §6.15.3: back-edge — cache: loop body = 18 instr × 4 B = 72 B, spans 2 cache line(s)
.L1:                              // FLS §6.15.3: loop boundary — cache-line: label has zero footprint
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn loop_with_break_value — FLS §9
    .global loop_with_break_value
loop_with_break_value:
    // cache-line: prologue = 1 instr(s) × 4 bytes = 4 bytes — 1 of 16 slots in first cache line
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
.L2:                              // FLS §6.15.2: loop boundary — cache-line: label has zero footprint
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    cmp     x1, x2               // FLS §6.5.3: compare (signed)
    cset    x3, ge                    // FLS §6.5.3: x3 = (x1 >= x2)
    cbz     x3, .L4                     // FLS §6.17: branch if false
    ldr     x4, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x5, #2                   // FLS §2.4.4.1: load imm 2
    mul     x6, x4, x5          // FLS §6.5.5: mul
    cmp     x6, w6, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    str     x6, [sp, #24             ] // FLS §8.1: store slot 3
    b       .L3                        // FLS §6.15.6: branch
    b       .L5                        // FLS §6.17: branch
.L4:                              // FLS §6.17: branch target
.L5:                              // FLS §6.17: branch target
    ldr     x7, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x8, #1                   // FLS §2.4.4.1: load imm 1
    add     x9, x7, x8          // FLS §6.5.5: add
    cmp     x9, w9, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    str     x9, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L2                        // FLS §6.15.2: back-edge — cache: loop body = 20 instr × 4 B = 80 B, spans 2 cache line(s)
.L3:                              // FLS §6.15.2: loop boundary — cache-line: label has zero footprint
    ldr     x1, [sp, #24             ] // FLS §8.1: load slot 3
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn while_with_continue — FLS §9
    .global while_with_continue
while_with_continue:
    // cache-line: prologue = 1 instr(s) × 4 bytes = 4 bytes — 1 of 16 slots in first cache line
    sub     sp, sp, #32             // FLS §8.1: frame for 3 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
.L6:                              // FLS §6.15.3: loop boundary — cache-line: label has zero footprint
    ldr     x2, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x3, [sp, #0              ] // FLS §8.1: load slot 0
    cmp     x2, x3               // FLS §6.5.3: compare (signed)
    cset    x4, lt                    // FLS §6.5.3: x4 = (x2 < x3)
    cbz     x4, .L7                     // FLS §6.15.3: branch if false
    ldr     x5, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x6, #1                   // FLS §2.4.4.1: load imm 1
    add     x7, x5, x6          // FLS §6.5.5: add
    cmp     x7, w7, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    str     x7, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x8, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x9, #3                   // FLS §2.4.4.1: load imm 3
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, eq                    // FLS §6.5.3: x10 = (x8 == x9)
    cbz     x10, .L8                     // FLS §6.17: branch if false
    b       .L6                        // FLS §6.15.7: back-edge — cache: loop body = 17 instr × 4 B = 68 B, spans 2 cache line(s)
    b       .L9                        // FLS §6.17: branch
.L8:                              // FLS §6.17: branch target
.L9:                              // FLS §6.17: branch target
    ldr     x11, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x12, [sp, #8              ] // FLS §8.1: load slot 1
    add     x13, x11, x12          // FLS §6.5.5: add
    cmp     x13, w13, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    str     x13, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L6                        // FLS §6.15.3: back-edge — cache: loop body = 25 instr × 4 B = 100 B, spans 2 cache line(s)
.L7:                              // FLS §6.15.3: loop boundary — cache-line: label has zero footprint
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn for_range_sum — FLS §9
    .global for_range_sum
for_range_sum:
    // cache-line: prologue = 1 instr(s) × 4 bytes = 4 bytes — 1 of 16 slots in first cache line
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    str     x2, [sp, #24             ] // FLS §8.1: store slot 3
.L10:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x3, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x4, [sp, #24             ] // FLS §8.1: load slot 3
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, lt                    // FLS §6.5.3: x5 = (x3 < x4)
    cbz     x5, .L12                    // FLS §6.15.1: branch if false
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x7, [sp, #16             ] // FLS §8.1: load slot 2
    add     x8, x6, x7          // FLS §6.5.5: add
    cmp     x8, w8, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    str     x8, [sp, #8              ] // FLS §8.1: store slot 1
.L11:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x10, #1                   // FLS §2.4.4.1: load imm 1
    add     x11, x9, x10          // FLS §6.5.5: add
    str     x11, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L10                       // FLS §6.15.1: back-edge — cache: loop body = 16 instr × 4 B = 64 B, spans 1 cache line(s)
.L12:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn for_inclusive_range — FLS §9
    .global for_inclusive_range
for_inclusive_range:
    // cache-line: prologue = 1 instr(s) × 4 bytes = 4 bytes — 1 of 16 slots in first cache line
    sub     sp, sp, #32             // FLS §8.1: frame for 4 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    str     x2, [sp, #24             ] // FLS §8.1: store slot 3
.L13:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x3, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x4, [sp, #24             ] // FLS §8.1: load slot 3
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, le                    // FLS §6.5.3: x5 = (x3 <= x4)
    cbz     x5, .L15                    // FLS §6.15.1: branch if false
    ldr     x6, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x7, [sp, #16             ] // FLS §8.1: load slot 2
    add     x8, x6, x7          // FLS §6.5.5: add
    cmp     x8, w8, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    str     x8, [sp, #8              ] // FLS §8.1: store slot 1
.L14:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x9, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x10, #1                   // FLS §2.4.4.1: load imm 1
    add     x11, x9, x10          // FLS §6.5.5: add
    str     x11, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L13                       // FLS §6.15.1: back-edge — cache: loop body = 16 instr × 4 B = 64 B, spans 1 cache line(s)
.L15:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #32             // FLS §8.1: restore stack frame
    ret

    // fn while_let_countdown — FLS §9
    .global while_let_countdown
while_let_countdown:
    // cache-line: prologue = 1 instr(s) × 4 bytes = 4 bytes — 1 of 16 slots in first cache line
    sub     sp, sp, #48             // FLS §8.1: frame for 5 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
.L16:                              // FLS §6.15.4: loop boundary — cache-line: label has zero footprint
    ldr     x2, [sp, #8              ] // FLS §8.1: load slot 1
    str     x2, [sp, #24             ] // FLS §8.1: store slot 3
    ldr     x3, [sp, #24             ] // FLS §8.1: load slot 3
    str     x3, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x4, [sp, #32             ] // FLS §8.1: load slot 4
    mov     x5, #0                   // FLS §2.4.4.1: load imm 0
    cmp     x4, x5               // FLS §6.5.3: compare (signed)
    cset    x6, le                    // FLS §6.5.3: x6 = (x4 <= x5)
    cbz     x6, .L18                    // FLS §6.17: branch if false
    b       .L17                       // FLS §6.15.6: branch
    b       .L19                       // FLS §6.17: branch
.L18:                              // FLS §6.17: branch target
.L19:                              // FLS §6.17: branch target
    ldr     x7, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x8, #1                   // FLS §2.4.4.1: load imm 1
    add     x9, x7, x8          // FLS §6.5.5: add
    cmp     x9, w9, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    str     x9, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x10, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x11, #1                   // FLS §2.4.4.1: load imm 1
    sub     x12, x10, x11          // FLS §6.5.5: sub
    cmp     x12, w12, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    str     x12, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L16                       // FLS §6.15.4: back-edge — cache: loop body = 24 instr × 4 B = 96 B, spans 2 cache line(s)
.L17:                              // FLS §6.15.4: loop boundary — cache-line: label has zero footprint
    ldr     x2, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn labeled_break_outer — FLS §9
    .global labeled_break_outer
labeled_break_outer:
    // cache-line: prologue = 1 instr(s) × 4 bytes = 4 bytes — 1 of 16 slots in first cache line
    sub     sp, sp, #48             // FLS §8.1: frame for 6 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    str     x2, [sp, #24             ] // FLS §8.1: store slot 3
.L20:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x3, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x4, [sp, #24             ] // FLS §8.1: load slot 3
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, lt                    // FLS §6.5.3: x5 = (x3 < x4)
    cbz     x5, .L22                    // FLS §6.15.1: branch if false
    mov     x6, #0                   // FLS §2.4.4.1: load imm 0
    str     x6, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x7, [sp, #0              ] // FLS §8.1: load slot 0
    str     x7, [sp, #40             ] // FLS §8.1: store slot 5
.L23:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x8, [sp, #32             ] // FLS §8.1: load slot 4
    ldr     x9, [sp, #40             ] // FLS §8.1: load slot 5
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, lt                    // FLS §6.5.3: x10 = (x8 < x9)
    cbz     x10, .L25                    // FLS §6.15.1: branch if false
    ldr     x11, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x12, [sp, #32             ] // FLS §8.1: load slot 4
    add     x13, x11, x12          // FLS §6.5.5: add
    cmp     x13, w13, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    ldr     x14, [sp, #0              ] // FLS §8.1: load slot 0
    mov     x15, #1                   // FLS §2.4.4.1: load imm 1
    sub     x16, x14, x15          // FLS §6.5.5: sub
    cmp     x16, w16, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    cmp     x13, x16               // FLS §6.5.3: compare (signed)
    cset    x17, eq                    // FLS §6.5.3: x17 = (x13 == x16)
    cbz     x17, .L26                    // FLS §6.17: branch if false
    ldr     x18, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x19, #10                  // FLS §2.4.4.1: load imm 10
    mul     x20, x18, x19          // FLS §6.5.5: mul
    cmp     x20, w20, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    ldr     x21, [sp, #32             ] // FLS §8.1: load slot 4
    add     x22, x20, x21          // FLS §6.5.5: add
    cmp     x22, w22, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    str     x22, [sp, #8              ] // FLS §8.1: store slot 1
    b       .L22                       // FLS §6.15.6: branch
    b       .L27                       // FLS §6.17: branch
.L26:                              // FLS §6.17: branch target
.L27:                              // FLS §6.17: branch target
.L24:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x23, [sp, #32             ] // FLS §8.1: load slot 4
    mov     x24, #1                   // FLS §2.4.4.1: load imm 1
    add     x25, x23, x24          // FLS §6.5.5: add
    str     x25, [sp, #32             ] // FLS §8.1: store slot 4
    b       .L23                       // FLS §6.15.1: back-edge — cache: loop body = 35 instr × 4 B = 140 B, spans 3 cache line(s)
.L25:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
.L21:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    add     x8, x6, x7          // FLS §6.5.5: add
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L20                       // FLS §6.15.1: back-edge — cache: loop body = 49 instr × 4 B = 196 B, spans 4 cache line(s)
.L22:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn labeled_continue — FLS §9
    .global labeled_continue
labeled_continue:
    // cache-line: prologue = 1 instr(s) × 4 bytes = 4 bytes — 1 of 16 slots in first cache line
    sub     sp, sp, #48             // FLS §8.1: frame for 6 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    str     x0, [sp, #8              ] // FLS §8.1: store slot 1
    mov     x1, #0                   // FLS §2.4.4.1: load imm 0
    str     x1, [sp, #16             ] // FLS §8.1: store slot 2
    ldr     x2, [sp, #0              ] // FLS §8.1: load slot 0
    str     x2, [sp, #24             ] // FLS §8.1: store slot 3
.L28:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x3, [sp, #16             ] // FLS §8.1: load slot 2
    ldr     x4, [sp, #24             ] // FLS §8.1: load slot 3
    cmp     x3, x4               // FLS §6.5.3: compare (signed)
    cset    x5, lt                    // FLS §6.5.3: x5 = (x3 < x4)
    cbz     x5, .L30                    // FLS §6.15.1: branch if false
    mov     x6, #0                   // FLS §2.4.4.1: load imm 0
    str     x6, [sp, #32             ] // FLS §8.1: store slot 4
    ldr     x7, [sp, #0              ] // FLS §8.1: load slot 0
    str     x7, [sp, #40             ] // FLS §8.1: store slot 5
.L31:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x8, [sp, #32             ] // FLS §8.1: load slot 4
    ldr     x9, [sp, #40             ] // FLS §8.1: load slot 5
    cmp     x8, x9               // FLS §6.5.3: compare (signed)
    cset    x10, lt                    // FLS §6.5.3: x10 = (x8 < x9)
    cbz     x10, .L33                    // FLS §6.15.1: branch if false
    ldr     x11, [sp, #32             ] // FLS §8.1: load slot 4
    mov     x12, #1                   // FLS §2.4.4.1: load imm 1
    cmp     x11, x12               // FLS §6.5.3: compare (signed)
    cset    x13, eq                    // FLS §6.5.3: x13 = (x11 == x12)
    cbz     x13, .L34                    // FLS §6.17: branch if false
    b       .L29                       // FLS §6.15.7: branch
    b       .L35                       // FLS §6.17: branch
.L34:                              // FLS §6.17: branch target
.L35:                              // FLS §6.17: branch target
    ldr     x14, [sp, #8              ] // FLS §8.1: load slot 1
    ldr     x15, [sp, #16             ] // FLS §8.1: load slot 2
    add     x16, x14, x15          // FLS §6.5.5: add
    cmp     x16, w16, sxtw            // FLS §6.23: i32 overflow check
    b.ne    _galvanic_panic                  // FLS §6.23: i32 overflow → panic
    str     x16, [sp, #8              ] // FLS §8.1: store slot 1
.L32:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x17, [sp, #32             ] // FLS §8.1: load slot 4
    mov     x18, #1                   // FLS §2.4.4.1: load imm 1
    add     x19, x17, x18          // FLS §6.5.5: add
    str     x19, [sp, #32             ] // FLS §8.1: store slot 4
    b       .L31                       // FLS §6.15.1: back-edge — cache: loop body = 23 instr × 4 B = 92 B, spans 2 cache line(s)
.L33:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
.L29:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x6, [sp, #16             ] // FLS §8.1: load slot 2
    mov     x7, #1                   // FLS §2.4.4.1: load imm 1
    add     x8, x6, x7          // FLS §6.5.5: add
    str     x8, [sp, #16             ] // FLS §8.1: store slot 2
    b       .L28                       // FLS §6.15.1: back-edge — cache: loop body = 37 instr × 4 B = 148 B, spans 3 cache line(s)
.L30:                              // FLS §6.15.1: loop boundary — cache-line: label has zero footprint
    ldr     x1, [sp, #8              ] // FLS §8.1: load slot 1
    mov     x0, x1              // FLS §6.19: return reg 1 → x0
    add     sp, sp, #48             // FLS §8.1: restore stack frame
    ret

    // fn main — FLS §9
    .global main
main:
    // cache-line: prologue = 1 instr(s) × 4 bytes = 4 bytes — 1 of 16 slots in first cache line
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    mov     x0, #5                   // FLS §2.4.4.1: load imm 5
    bl      while_loop               // FLS §6.12.1: call while_loop
    mov     x1, x0              // FLS §6.12.1: return value → x1
    mov     x2, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x2                   // FLS §6.12.1: arg 0
    bl      loop_with_break_value    // FLS §6.12.1: call loop_with_break_value
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x4, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x4                   // FLS §6.12.1: arg 0
    bl      while_with_continue      // FLS §6.12.1: call while_with_continue
    mov     x5, x0              // FLS §6.12.1: return value → x5
    mov     x6, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x6                   // FLS §6.12.1: arg 0
    bl      for_range_sum            // FLS §6.12.1: call for_range_sum
    mov     x7, x0              // FLS §6.12.1: return value → x7
    mov     x8, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x8                   // FLS §6.12.1: arg 0
    bl      for_inclusive_range      // FLS §6.12.1: call for_inclusive_range
    mov     x9, x0              // FLS §6.12.1: return value → x9
    mov     x10, #3                   // FLS §2.4.4.1: load imm 3
    mov     x0, x10                  // FLS §6.12.1: arg 0
    bl      while_let_countdown      // FLS §6.12.1: call while_let_countdown
    mov     x11, x0              // FLS §6.12.1: return value → x11
    mov     x12, #5                   // FLS §2.4.4.1: load imm 5
    mov     x0, x12                  // FLS §6.12.1: arg 0
    bl      labeled_break_outer      // FLS §6.12.1: call labeled_break_outer
    mov     x13, x0              // FLS §6.12.1: return value → x13
    mov     x14, #4                   // FLS §2.4.4.1: load imm 4
    mov     x0, x14                  // FLS §6.12.1: arg 0
    bl      labeled_continue         // FLS §6.12.1: call labeled_continue
    mov     x15, x0              // FLS §6.12.1: return value → x15
    mov     x0, #0              // FLS §4.4: unit return
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // ELF entry point — FLS §18.1
    // cache-line: _start = 3 instructions × 4 bytes = 12 bytes — fits in one 64-byte cache line
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)

    // FLS §6.23: runtime panic primitive — exit(101)
    // cache-line: _galvanic_panic = 3 instructions × 4 bytes = 12 bytes — fits in one 64-byte cache line
    .global _galvanic_panic
_galvanic_panic:
    movz    x0, #101        // panic exit code (galvanic sentinel)
    movz    x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(101) — FLS §6.23: panic
