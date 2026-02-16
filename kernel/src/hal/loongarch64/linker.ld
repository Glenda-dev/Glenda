OUTPUT_ARCH(loongarch64)
ENTRY(_start)

SECTIONS
{
  /*
     Physical RAM start at 0x0, using 2MB offset to avoid boot_info
  */
  . = 0x200000;

  .text : ALIGN(4) {
    __text_start = .;
    KEEP(*(.text.start))
    *(.text .text.*)
    . = ALIGN(0x1000);

    __trampoline = .;
    KEEP(*(trampsec))
    . = ALIGN(0x1000);
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
    *(.bss.end)
    PROVIDE(__bss_end = .);
  }

  . = ALIGN(0x1000);
  PROVIDE(__alloc_start = .);
}
