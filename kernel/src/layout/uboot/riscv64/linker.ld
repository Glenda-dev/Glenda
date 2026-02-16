OUTPUT_ARCH(elf64-littleriscv)
ENTRY(_start)

PHDRS
{
    text    PT_LOAD;
    rodata  PT_LOAD;
    data    PT_LOAD;
}

SECTIONS
{
   . = 0x80200000;
   __kernel_vbase = .;
   __kernel_pbase = .;

  .text : AT(__kernel_pbase) {
    __text_start = .;
    KEEP(*(.text.start))
    *(.text .text.*)
    . = ALIGN(0x1000);

    __trampoline = .;
    KEEP(*(trampsec))
    . = ALIGN(0x1000);
    ASSERT(. - __trampoline == 0x1000, "error: trampoline larger than one page");
    __text_end = .;
  } :text

  . = ALIGN(0x1000);
  .rodata : AT(ADDR(.rodata) - __kernel_vbase + __kernel_pbase) {
    __rodata_start = .;
    *(.rodata .rodata.*)
    __rodata_end = .;
  } :rodata

  . = ALIGN(0x1000);
  .data : AT(ADDR(.data) - __kernel_vbase + __kernel_pbase) {
    __data_start = .;
    PROVIDE(__global_pointer$ = . + 0x800);
    *(.data .data.*)

    KEEP(*(.requests_start_marker))
    KEEP(*(.requests))
    KEEP(*(.requests_end_marker))

    *(.sdata .sdata.*)
    __data_end = .;
  } :data

  . = ALIGN(0x1000);
  .bss : AT(ADDR(.bss) - __kernel_vbase + __kernel_pbase) {
    __bss_start = .;
    *(.sbss .sbss.* .bss .bss.* COMMON)
    *(.bss.stack)
    __bss_end = .;
  } :data

  . = ALIGN(0x1000);
  __kernel_vmax = .;
  __kernel_pmax = . - __kernel_vbase + __kernel_pbase;

  PROVIDE(__alloc_start = .);

  /* Discard .note.* and .eh_frame* since they may cause issues on some hosts. */
  /DISCARD/ : {
      *(.eh_frame*)
      *(.note .note.*)
  }
}
