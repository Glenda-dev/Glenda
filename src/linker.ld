/*
 The Linker Script for Glenda
 */

OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
  /*
    QEMU Virt 的 RAM 从 0x80000000 开始,
    OpenSBI 会把内核放到 0x80200000
   */
  . = 0x80200000;

  .text : ALIGN(4) {
    KEEP(*(.text.start))
    *(.text .text.*)
  }

  .rodata : ALIGN(16) {
    *(.rodata .rodata.*)
  }

  .data : ALIGN(16) {
    *(.sdata .sdata.* .data .data.*)
  }

  .bss : ALIGN(16) {
    __bss_start = .;
    *(.sbss .sbss.* .bss .bss.* COMMON)
    __bss_end = .;
  }

  . = ALIGN(16);
}
