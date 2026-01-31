use crate::cap::CapPtr;
use crate::hal::mem::PGSIZE;
use crate::mem::UTCB_VA;

pub const CSPACE_CAP: CapPtr = CapPtr::from(1);
pub const VSPACE_CAP: CapPtr = CapPtr::from(2);
pub const TCB_CAP: CapPtr = CapPtr::from(3);
pub const CONSOLE_CAP: CapPtr = CapPtr::from(5);
pub const PLATFORM_CAP: CapPtr = CapPtr::from(6);
pub const UNTYPED_CAP: CapPtr = CapPtr::from(7);
pub const MMIO_CAP: CapPtr = CapPtr::from(8);
pub const IRQ_CAP: CapPtr = CapPtr::from(9);

pub const STACK_VA: usize = UTCB_VA - PGSIZE; // 用户栈映射地址
pub const STACK_PAGES: usize = 16; // 用户栈页面数 16 * 4KB = 64KB
pub const STACK_SIZE: usize = STACK_PAGES * PGSIZE; // 64KB
pub const HEAP_PAGES: usize = 64; // 用户堆页面数 64 * 4KB = 256KB
pub const HEAP_SIZE: usize = HEAP_PAGES * PGSIZE; // 256KB
pub const HEAP_VA: usize = 0x2000_0000; // 用户堆地址
pub const BOOTINFO_VA: usize = 0x4000_0000; // Bootinfo映射地址
pub const INITRD_VA: usize = 0x5000_0000; // Initrd 映射地址 (Root Task)
pub const ROOT_TASK_PRIORITY: u8 = 253; // Root Task 优先级
