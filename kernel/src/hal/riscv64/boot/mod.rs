use super::dtb;
use super::platform::PlatformHandle;
use crate::glenda_main;
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
        tail glenda_boot
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
#[repr(usize)]
pub enum BootLoaderType {
    OpenSBI,
    #[cfg(feature = "multiboot2")]
    Multiboot2,
}

pub static mut BOOT_LOADER_TYPE: BootLoaderType = BootLoaderType::OpenSBI;

#[unsafe(no_mangle)]
pub extern "C" fn glenda_boot(_a0: usize, a1: usize) -> ! {
    let dtb = a1;
    #[cfg(feature = "multiboot2")]
    {
        let mut dtb = dtb;
        use super::cpu;
        // Check for Multiboot2 magic
        if a0 == multiboot2::MULTIBOOT2_MAGIC as usize {
            let info = multiboot2::parse(a0, a1);
            if let Some(new_info) = info.dtb {
                dtb = new_info;
            }
            if let (Some(start), Some(end)) = (info.initrd_start, info.initrd_end) {
                unsafe {
                    multiboot2::MULTIBOOT_INITRD = (start, end);
                }
            }
            // If we are in Multiboot2, we might not know the hartid.
            // Assume 0 for the boot hart if not provided.
            unsafe {
                BOOT_LOADER_TYPE = BootLoaderType::Multiboot2;
            }
        }
    }
    let dtb = PlatformHandle::from(dtb);
    dtb::init(dtb.as_ptr());
    log!("hal: HAL initialized, dtb at {:#x}", dtb.bits());
    glenda_main();
}
