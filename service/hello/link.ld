ENTRY(_start)

SECTIONS
{
  . = 0;

  .text : ALIGN(4)
  {
    /* 把 _start 放在最前面 */
    KEEP(*(.text.entry))
    *(.text*)
  }

  .rodata : ALIGN(4)
  {
    *(.rodata*)
  }

  .data : ALIGN(4)
  {
    *(.data*)
  }

  .bss : ALIGN(4)
  {
    *(.bss*)
    *(COMMON)
  }
}

