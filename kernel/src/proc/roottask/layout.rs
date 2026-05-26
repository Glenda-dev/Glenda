use crate::cap::CapPtr;
pub use crate::hal::mem::{
    BOOTINFO_VA, HEAP_VA, INITRD_VA, PGSIZE, STACK_BASE, TRAPFRAME_VA, UTCB_VA,
};

pub const CSPACE_CAP: CapPtr = CapPtr::from(1);
pub const VSPACE_CAP: CapPtr = CapPtr::from(2);
pub const TCB_CAP: CapPtr = CapPtr::from(3);
pub const CONSOLE_CAP: CapPtr = CapPtr::from(5);
pub const BOOTINFO_CAP: CapPtr = CapPtr::from(9);
pub const UNTYPED_CAP: CapPtr = CapPtr::from(10);
pub const KERNEL_CAP: CapPtr = CapPtr::from(11);
pub const IRQ_CAP: CapPtr = CapPtr::from(12);

pub const STACK_LIMIT: usize = STACK_BASE - STACK_SIZE; // 用户栈最低地址
pub const STACK_PAGES: usize = 32; // 增加栈大小到 128KB
pub const STACK_SIZE: usize = STACK_PAGES * PGSIZE; // 128KB
pub const HEAP_PAGES: usize = 256; // 用户堆页面数 256 * 4KB = 1MB
pub const HEAP_SIZE: usize = HEAP_PAGES * PGSIZE; // 1MB
pub const ROOT_TASK_PRIORITY: u8 = 253; // Root Task 优先级
