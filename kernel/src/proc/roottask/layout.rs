use crate::hal::mem::PGSIZE;
use crate::mem::UTCB_VA;

// Common CSpace Slot Definitions
pub const CSPACE_SLOT: usize = 1;
pub const VSPACE_SLOT: usize = 2;
pub const TCB_SLOT: usize = 3;
// Root Task Specific Slots
pub const CONSOLE_SLOT: usize = 5;
pub const PLATFORM_SLOT: usize = 6;
pub const UNTYPED_SLOT: usize = 7;
pub const MMIO_SLOT: usize = 8;
pub const IRQ_SLOT: usize = 9;

pub const STACK_VA: usize = UTCB_VA - PGSIZE; // 用户栈映射地址
pub const STACK_PAGES: usize = 16; // 用户栈页面数 16 * 4KB = 64KB
pub const STACK_SIZE: usize = STACK_PAGES * PGSIZE; // 64KB
pub const HEAP_PAGES: usize = 64; // 用户堆页面数 64 * 4KB = 256KB
pub const HEAP_SIZE: usize = HEAP_PAGES * PGSIZE; // 256KB
pub const HEAP_VA: usize = 0x2000_0000; // 用户堆地址
pub const RES_VA_BASE: usize = 0x4000_0000; // 启动时提供的资源
pub const BOOTINFO_VA: usize = RES_VA_BASE; // Bootinfo映射地址
pub const INITRD_VA: usize = BOOTINFO_VA + PGSIZE; // Initrd 映射地址 (Root Task)
pub const ROOT_TASK_PRIORITY: u8 = 253; // Root Task 优先级
