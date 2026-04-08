    .text

    // fn Circle__area — FLS §9
    .global Circle__area
Circle__area:
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    ldr     x0, [sp, #0              ] // FLS §8.1: load slot 0
    ldr     x1, [sp, #0              ] // FLS §8.1: load slot 0
    mul     x2, x0, x1          // FLS §6.5.5: mul
    mov     x0, x2              // FLS §6.19: return reg 2 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ret

    // fn print_area — FLS §9
    .global print_area
print_area:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 2 slot(s)
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    str     x1, [sp, #8              ] // FLS §8.1: store slot 1
    ldr     x9,  [sp, #8             ] // FLS §4.13: load vtable ptr from slot 1
    ldr     x10, [x9,  #0             ] // FLS §4.13: load method[0] fn-ptr from vtable
    ldr     x0,  [sp, #0             ] // FLS §4.13: load data ptr into x0
    blr     x10                          // FLS §4.13: indirect call via vtable
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // fn main — FLS §9
    .global main
main:
    str     x30, [sp, #-16]!      // FLS §6.12.1: save lr (non-leaf)
    sub     sp, sp, #16             // FLS §8.1: frame for 1 slot(s)
    mov     x0, #5                   // FLS §2.4.4.1: load imm 5
    str     x0, [sp, #0              ] // FLS §8.1: store slot 0
    add     x1, sp, #0                   // FLS §6.5.1: address of stack slot 0
    adrp    x2, vtable_Shape_Circle              // FLS §4.9: fn ptr addr (page)
    add     x2, x2, :lo12:vtable_Shape_Circle  // FLS §4.9: fn ptr addr (offset)
    mov     x0, x1                   // FLS §6.12.1: arg 0
    mov     x1, x2                   // FLS §6.12.1: arg 1
    bl      print_area               // FLS §6.12.1: call print_area
    mov     x3, x0              // FLS §6.12.1: return value → x3
    mov     x0, x3              // FLS §6.19: return reg 3 → x0
    add     sp, sp, #16             // FLS §8.1: restore stack frame
    ldr     x30, [sp], #16         // FLS §6.12.1: restore lr
    ret

    // FLS §4.13: vtable dispatch shims

    .global vtable_shim_Shape_Circle_0
    .align 2
vtable_shim_Shape_Circle_0:
    mov     x9, x0                       // FLS §4.13: save data ptr in scratch x9
    ldr     x0, [x9, #0               ] // FLS §4.13: load field 0 from data ptr
    b       Circle__area                 // FLS §4.13: tail-call concrete method

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)

    .section .rodata
    .align 3
    .global vtable_Shape_Circle
vtable_Shape_Circle:
    .quad vtable_shim_Shape_Circle_0       // FLS §4.13: vtable entry
