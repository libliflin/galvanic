    .text

    // fn main — FLS §9
    .global main
main:
    mov     x0, #0                   // FLS §2.4.4.1: load imm 0
    ret

    // ELF entry point — FLS §18.1
    .global _start
_start:
    bl      main            // call fn main()
    // x0 = main()'s return value
    mov     x8, #93         // __NR_exit (ARM64 Linux)
    svc     #0              // exit(x0)
