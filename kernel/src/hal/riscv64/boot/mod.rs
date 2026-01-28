use super::asm;
use super::platform::PlatformInfo;
use core::arch::global_asm;

global_asm!(
    r#"
    .option arch, +zmmul // 启用 Zmmul 扩展以支持多核启动
    .section .text.start
    .globl _start
    .globl secondary_start

    .equ BOOT_STACK_SIZE, 65536 // 64KB 启动栈
    .equ MAX_BOOT_HARTS, 8  // 最多 8 个 hart 并发启动
    
    .macro HART_ENTRY
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
        tail glenda_main
    .endm

_start: // boot hart
    HART_ENTRY

secondary_start: // secondary harts
    HART_ENTRY

// 启动栈放在 .bss 段，这样不会与代码混在一起
    .section .bss
    .align 16
boot_stack:
    .space BOOT_STACK_SIZE * MAX_BOOT_HARTS
boot_stack_top:
    "#
);

#[cfg(feature = "multiboot2")]
pub mod multiboot2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootLoaderType {
    OpenSBI,
    #[cfg(feature = "multiboot2")]
    Multiboot2,
}

pub static mut BOOT_LOADER_TYPE: BootLoaderType = BootLoaderType::OpenSBI;

pub fn detect() -> (usize, PlatformInfo) {
    let a0 = unsafe { asm::read_a0() };
    let a1 = unsafe { asm::read_a1() };
    let hartid = a0;
    let info = PlatformInfo::from(a1);

    #[cfg(feature = "multiboot2")]
    {
        use super::cpu;
        let mut info = info;
        let mut hartid = hartid;
        // Check for Multiboot2 magic
        if a0 == multiboot2::MULTIBOOT2_MAGIC as usize {
            let info = multiboot2::parse(a0, a1);
            if let Some(new_info) = info.dtb {
                info = PlatformInfo::from(new_info as usize);
            }
            if let (Some(start), Some(end)) = (info.initrd_start, info.initrd_end) {
                unsafe {
                    multiboot2::MULTIBOOT_INITRD = (start, end);
                }
            }
            // If we are in Multiboot2, we might not know the hartid.
            // Assume 0 for the boot hart if not provided.
            hartid = cpu::cpu_id();
            unsafe {
                BOOT_LOADER_TYPE = BootLoaderType::Multiboot2;
            }
        }
    }
    (hartid, info)
}
