OUTPUT_ARCH(riscv)
ENTRY(efi_main)

SECTIONS
{
    /* 
     * UEFI apps are relocatable. We link at 0, 
     * but we provide a .reloc section if needed.
     */
    . = 0;
    __kernel_pbase = .;
    __kernel_vbase = .;

    .text : ALIGN(4096) {
        __text_start = .;
        *(.text .text.*)
        
        . = ALIGN(4096);
        __trampoline = .;
        KEEP(*(trampsec))
        . = ALIGN(4096);
        ASSERT(. - __trampoline == 4096, "error: trampoline larger than one page");
        
        __text_end = .;
    }

    .rodata : ALIGN(4096) {
        __rodata_start = .;
        *(.rodata .rodata.*)
        __rodata_end = .;
    }

    .data : ALIGN(4096) {
        __data_start = .;
        *(.data .data.* .sdata .sdata.*)
        __data_end = .;
    }

    .bss : ALIGN(4096) {
        __bss_start = .;
        *(.bss .bss.* .sbss .sbss.*)
        __bss_end = .;
    }

    . = ALIGN(4096);
    __alloc_start = .;

    /DISCARD/ : {
        *(.eh_frame)
        *(.reltab .rel.* )
    }
}
