use core::arch::global_asm;

global_asm!(
    "    .section .text.start
    .global _start
_start:
    # 1. 禁用中断
    csrw sie, zero
    
    # 2. 保存 a0 (hartid), a1 (dtb_pa)
    mv s0, a0
    mv s1, a1

    # 3. 设置初始栈 (物理地址)
    la sp, boot_stack_top

    # 4. 跳转到 uboot_bootstrap
    mv a0, s0
    mv a1, s1
    call uboot_bootstrap

    .global secondary_start
secondary_start:
    # 1. 禁用中断
    csrw sie, zero

    # 2. 设置初始栈
    mv s0, a0
    la sp, boot_stack_top
    li t0, 4096 * 4
    mul t1, s0, t0
    sub sp, sp, t1

    # 3. 跳转到 uboot_secondary_bootstrap
    mv a0, s0
    call uboot_secondary_bootstrap

    # 永不返回
loop:
    j loop

    .section .bss.stack
    .align 12
    .global boot_stack_bottom
boot_stack_bottom:
    .space 4096 * 4
    .global boot_stack_top
boot_stack_top:
"
);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn uboot_bootstrap(hartid: usize, dtb_pa: usize) -> ! {
    unsafe { crate::boot::uboot::bootstrap_kernel(hartid, dtb_pa) }
}
