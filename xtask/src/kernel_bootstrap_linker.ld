/* Toolchain-owned linker script for the minimal bootstrap kernel image.
 *
 * The toolchain milestone owns a reproducible link layout so the bootstrap
 * binary boots under QEMU virt and is byte-identical across clean rebuilds.
 * The real kernel linker layout (kernel/src/linker.ld) is owned by the
 * boot-asm milestone and supersedes this script.
 */
OUTPUT_ARCH(riscv)
ENTRY(_start)

BASE = 0x80200000;

SECTIONS
{
  . = BASE;

  .text : {
    KEEP(*(.text._start))
    *(.text .text.*)
  }

  .rodata : {
    *(.rodata .rodata.*)
  }

  .data : {
    *(.data .data.*)
    . = ALIGN(16);
  }

  /* Early boot stack lives at the TOP of BSS, above all string literals in
   * .rodata/.data, so the downward-growing stack can never clobber the
   * banner string during the first few calls. The prologue loads
   * _boot_stack_top into sp. */
  .bss : {
    . = ALIGN(16);
    __bss_start = .;
    *(.bss .bss.*)
    __bss_end = .;
    . = ALIGN(16);
    . += 0x4000;             /* 16 KiB early boot stack */
    . = ALIGN(16);
    _boot_stack_top = .;
  }

  /DISCARD/ : {
    *(.comment)
    *(.note*)
    *(.eh_frame*)
  }
}