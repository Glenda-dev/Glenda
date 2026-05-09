OUTPUT_ARCH(aarch64)
ENTRY(_start)

SECTIONS
{
  /*
     QEMU Virt 的 RAM 从 0x40000000 开始,
     TFA/Direct boot 通常把内核加载到 0x40080000
   */
  . = 0x40080000;

  __kernel_pbase = .;
  __kernel_vbase = .;

  .text : ALIGN(16) {
    __text_start = .;
    KEEP(*(.text.start))
    *(.text .text.*)
    . = ALIGN(0x1000);

    __trampoline = .;
    KEEP(*(trampsec))
    . = ALIGN(0x1000);
    ASSERT(. - __trampoline == 0x1000, "error: trampoline larger than one page");
    __text_end = .;
  }

  . = ALIGN(16);
  .rodata : ALIGN(16) {
    __rodata_start = .;
    *(.rodata .rodata.*)
    __rodata_end = .;
  }

  . = ALIGN(16);
  .data : ALIGN(16) {
    __data_start = .;
    *(.sdata .sdata.* .data .data.*)
    __data_end = .;
  }

  .bss : ALIGN(16) {
    __bss_start = .;
    *(.sbss .sbss.* .bss .bss.*)
    *(COMMON)
    __bss_end = .;
  }

  . = ALIGN(0x1000);
  __alloc_start = .;
  __kernel_end = .;
}
