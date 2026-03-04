use core::arch::global_asm;

#[cfg(not(target_arch = "riscv64"))]
compile_error!("OpenSBI only supports riscv64");

global_asm!(
    r#"
    .option arch, +zmmul // 启用 Zmmul 扩展以支持多核启动
    .section .text.start
    .globl _start
    .globl secondary_start

    .equ BOOT_STACK_SIZE, 65536 // 64KB 启动栈
    .equ MAX_BOOT_HARTS, 8  // 最多 8 个 hart 并发启动
    
    .macro HART_ENTRY entry
        csrw sie, zero
        la   t1, boot_stack_top
        li   t2, BOOT_STACK_SIZE
        li   t3, MAX_BOOT_HARTS
        mv   tp, a0           // 保存 hartid 到 tp 寄存器
        bgeu a0, t3, 1f
        mul  t2, t2, a0
        sub  sp, t1, t2
        li   s0, 0            // 初始化 fp 为 0，方便 backtrace 终止
        j    2f
1:
        mv   sp, t1
        li   s0, 0
2:
        tail \entry
    .endm

_start: // boot hart
    HART_ENTRY sbi_bootstrap

secondary_start: // secondary harts
    HART_ENTRY sbi_secondary_bootstrap

// 启动栈放在 .bss 段，这样不会与代码混在一起
    .section .bss
    .align 16
boot_stack:
    .space BOOT_STACK_SIZE * MAX_BOOT_HARTS
boot_stack_top:
    "#
);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sbi_bootstrap(hartid: usize, dtb_pa: usize) -> ! {
    // 保存早期信息
    super::set_boot_info(dtb_pa);
    crate::glenda_boot(hartid);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sbi_secondary_bootstrap(hartid: usize, _dtb_pa: usize) -> ! {
    crate::glenda_secondary(hartid);
}
