use crate::hart;
use core::arch::global_asm;

pub mod info;
pub mod initrd;
#[cfg(feature = "multiboot2")]
pub mod multiboot2;

pub use info::BootInfo;
pub use info::UntypedDesc;

global_asm!(
    r#"
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

/// Magic number to verify BootInfo validity: 'GLENDA_B'
pub const BOOTINFO_MAGIC: u32 = 0x99999999;

/// Fixed size of the BootInfo page (usually 4KB)
pub const BOOTINFO_SIZE: usize = 4096;

/// Maximum number of untyped memory regions we can describe
pub const MAX_UNTYPED_REGIONS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootLoaderType {
    OpenSBI,
    #[cfg(feature = "multiboot2")]
    Multiboot2,
}

pub static mut BOOT_LOADER_TYPE: BootLoaderType = BootLoaderType::OpenSBI;

pub fn detect(a0: usize, a1: usize) -> (usize, *const u8) {
    let mut hartid = a0;
    let mut dtb = a1 as *const u8;

    #[cfg(feature = "multiboot2")]
    {
        // Check for Multiboot2 magic
        if a0 == multiboot2::MULTIBOOT2_MAGIC as usize {
            let info = multiboot2::parse(a0, a1);
            if let Some(new_dtb) = info.dtb {
                dtb = new_dtb;
            }
            if let (Some(start), Some(end)) = (info.initrd_start, info.initrd_end) {
                unsafe {
                    multiboot2::MULTIBOOT_INITRD = (start, end);
                }
            }
            // If we are in Multiboot2, we might not know the hartid.
            // Assume 0 for the boot hart if not provided.
            hartid = hart::getid();
            unsafe {
                BOOT_LOADER_TYPE = BootLoaderType::Multiboot2;
            }
        }
    }
    (hartid, dtb)
}
