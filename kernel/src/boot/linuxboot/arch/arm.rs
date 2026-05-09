use core::arch::global_asm;

global_asm!(
    r#"
    .section .text.start
    .globl _start
    .globl secondary_start

    .equ BOOT_STACK_SIZE, 65536 // 64KB 启动栈
    .equ MAX_CPUS, 8

_start:
    // x0 = dtb_pa
    // Disable interrupts
    msr daifset, #0xf

    // Save early boot args
    ldr x8, =LINUXBOOT_ARGS
    str x0, [x8, #0]
    str x1, [x8, #8]
    str x2, [x8, #16]
    str x3, [x8, #24]

    // Setup stack
    ldr x8, =boot_stack_top
    mov sp, x8

    // Call linuxboot_bootstrap()
    bl linuxboot_bootstrap
    b .

secondary_start:
    // x0 = context_id (cpuid)
    msr daifset, #0xf

    // Setup stack for secondary CPU
    // sp = boot_stack_top - (cpuid * BOOT_STACK_SIZE)
    ldr x8, =boot_stack_top
    mov x9, #65536
    mul x9, x9, x0
    sub sp, x8, x9

    // Call linuxboot_secondary_bootstrap(cpuid)
    bl linuxboot_secondary_bootstrap
    b .

    .section .bss
    .align 16
boot_stack:
    .space 65536 * 8
boot_stack_top:
    "#
);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn linuxboot_bootstrap() -> ! {
    crate::glenda_boot(0);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn linuxboot_secondary_bootstrap(cpuid: usize) -> ! {
    crate::glenda_secondary(cpuid);
}
