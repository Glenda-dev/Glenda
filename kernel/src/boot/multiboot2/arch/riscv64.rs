use core::arch::global_asm;

global_asm!(
    "
    .section .text.header, \"ax\"
    .balign 8
multiboot2_header:
    .long 0xe85250d6                /* magic */
    .long 0                         /* architecture (i386 is 0, but works for generic) */
    .long multiboot2_header_end - multiboot2_header /* header_length */
    .long -(0xe85250d6 + 0 + (multiboot2_header_end - multiboot2_header)) /* checksum */

    /* Tags */
    .short 0, 0                     /* type, flags */
    .long 8                         /* size */
multiboot2_header_end:

    .section .text.start, \"ax\"
    .global _start
_start:
    /* Disable interrupts */
    csrw sie, zero

    /* Save magic (a0) and pointer (a1) */
    mv s0, a0
    mv s1, a1

    /* Set up stack */
    la sp, boot_stack_top

    /* Jump to multiboot2_bootstrap */
    mv a0, s0
    mv a1, s1
    call multiboot2_bootstrap

loop:
    j loop

    .section .bss.stack
    .align 12
    .global boot_stack_bottom
boot_stack_bottom:
    .space 4096 * 4
    .global boot_stack_top
boot_stack_top:
    "
);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn multiboot2_bootstrap(magic: usize, info_pa: usize) -> ! {
    unsafe { crate::boot::multiboot2::bootstrap_kernel(magic, info_pa) }
}
