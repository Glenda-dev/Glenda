use core::arch::global_asm;

global_asm!(
    r#"
    .equ CSR_EENTRY, 0xc
    .equ CSR_DMW0,   0x180
    .equ CSR_DMW1,   0x181

    .section .text.start
    .globl _start
_start:
    # DMW0: Cached, PLV0, MAT=1
    # See Chapter 7.5.18
    li.d $t0, 0x9000000000000011
    csrwr $t0, CSR_DMW0
    # DMW1: Uncached, PLV0, MAT=0
    li.d $t0, 0xa000000000000001
    csrwr $t0, CSR_DMW1

    la.abs $t0, __exception_vector
    csrwr $t0, CSR_EENTRY
    la.local $sp, boot_stack_top

    li.w $a0, 1
    la.abs $t0, glenda_main
    jirl $r0, $t0, 0

    .section .bss.end
    .globl __kernel_image_end
__kernel_image_end:
    .space 8

    .section .bss.stack
    .align 4 # 2^4 = 16 bytes
    .globl boot_stack
boot_stack:
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top:
    "#
);
