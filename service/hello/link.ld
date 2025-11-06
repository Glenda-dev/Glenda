ENTRY(_start)

SECTIONS
{
  . = 0x1000;

  .text : ALIGN(4)
  {
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

  /* User stack */
  . = 0x20000;
  .stack (NOLOAD) :
  {
    . = . + 24576; /* 24KB stack */
    _stack_top = .;
  }
}
