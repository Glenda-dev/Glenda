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
    __text_start = .;
    KEEP(*(.text.start))
    *(.text .text.*)
    . = ALIGN(0x1000);

    __trampoline = .;
    *(trampsec)
    . = ALIGN(0x1000);
    ASSERT(. - __trampoline == 0x1000, "error: trampoline larger than one page");
    __text_end = .;
  }

  . = ALIGN(0x1000);
  .rodata : ALIGN(16) {
    __rodata_start = .;
    *(.rodata .rodata.*)
    __rodata_end = .;
  }

  . = ALIGN(0x1000);
  .data : ALIGN(16) {
    __data_start = .;
    *(.sdata .sdata.* .data .data.*)
    __data_end = .;
  }

  . = ALIGN(0x1000);
  .bss : ALIGN(16) {
    PROVIDE(__bss_start = .);
    *(.sbss .sbss.* .bss .bss.* COMMON)
    PROVIDE(__bss_end = .);
  }

  . = ALIGN(0x1000);
  PROVIDE(__alloc_start = .);
}
