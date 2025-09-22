    .section .text.start
    .globl _start
_start:
    csrw sie, zero
    la   t0, trap
    csrw stvec, t0
    la   sp, boot_stack_top
    tail glenda_main

    .align 2
trap:
    wfi
    j trap

    .section .bss
    .align 16
boot_stack:
    .space 4096
boot_stack_top:
