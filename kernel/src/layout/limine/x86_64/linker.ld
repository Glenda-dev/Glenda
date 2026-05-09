OUTPUT_ARCH(i386:x86-64)
ENTRY(master_trampoline)

PHDRS
{
    text    PT_LOAD;
    rodata  PT_LOAD;
    data    PT_LOAD;
}

SECTIONS
{
    . = 0xffffffff80000000;
    __kernel_vbase = .;
    __kernel_pbase = .;

    .text : ALIGN(16) {
        __text_start = .;
        KEEP(*(.text.entry))
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
    .rodata : ALIGN(16) {
        __rodata_start = .;
        *(.rodata .rodata.*)
        __rodata_end = .;
    } :rodata

    . = ALIGN(0x1000);
    .data : ALIGN(16) {
        __data_start = .;
        *(.data .data.*)

        KEEP(*(.requests_start_marker))
        KEEP(*(.requests))
        KEEP(*(.requests_end_marker))

        *(.sdata .sdata.*)
        __data_end = .;
    } :data

    . = ALIGN(0x1000);
    .bss : ALIGN(16) {
        PROVIDE(__bss_start = .);
        *(.sbss .sbss.* .bss .bss.* COMMON)
        *(.bss.stack)
        PROVIDE(__bss_end = .);
    } :data

    . = ALIGN(0x1000);
    PROVIDE(__alloc_start = .);

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
    }
}
