use crate::cap::CapPtr;
use crate::hal::mem::PGSIZE;
use crate::mem::TRAMPOLINE_VA;

pub const CSPACE_CAP: CapPtr = CapPtr::from(1);
pub const VSPACE_CAP: CapPtr = CapPtr::from(2);
pub const TCB_CAP: CapPtr = CapPtr::from(3);
pub const CONSOLE_CAP: CapPtr = CapPtr::from(5);
pub const BOOTINFO_CAP: CapPtr = CapPtr::from(9);
pub const UNTYPED_CAP: CapPtr = CapPtr::from(10);
pub const KERNEL_CAP: CapPtr = CapPtr::from(11);
pub const IRQ_CAP: CapPtr = CapPtr::from(12);

pub const STACK_BASE: usize = TRAMPOLINE_VA; // 用户栈最高地址（起始地址，向低地址生长）
pub const STACK_LIMIT: usize = STACK_BASE - STACK_SIZE; // 用户栈最低地址
pub const STACK_PAGES: usize = 32; // 增加栈大小到 128KB
pub const STACK_SIZE: usize = STACK_PAGES * PGSIZE; // 128KB
pub const HEAP_PAGES: usize = 256; // 用户堆页面数 256 * 4KB = 1MB
pub const HEAP_SIZE: usize = HEAP_PAGES * PGSIZE; // 1MB
pub const HEAP_VA: usize = 0x2000_0000; // 用户堆地址
pub const BOOTINFO_VA: usize = 0x4000_0000; // Bootinfo映射地址
pub const INITRD_VA: usize = 0x5000_0000; // Initrd 映射地址 (Root Task)
pub const ROOT_TASK_PRIORITY: u8 = 253; // Root Task 优先级
#[cfg(target_pointer_width = "64")]
pub const THREAD_AREA_BASE: usize = 0x3F_0000_0000;
#[cfg(target_pointer_width = "32")]
pub const THREAD_AREA_BASE: usize = 0x3000_0000;
pub const UTCB_VA: usize = THREAD_AREA_BASE; // UTCB 映射地址
pub const TRAPFRAME_VA: usize = THREAD_AREA_BASE + PGSIZE; // Trapframe 映射地址
