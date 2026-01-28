use crate::hal::mem::{PGSIZE, VA_MAX};

pub const TRAMPOLINE_VA: usize = VA_MAX - PGSIZE; // Trampoline 映射地址
pub const TRAPFRAME_VA: usize = TRAMPOLINE_VA - PGSIZE; // Trapframe 映射地址
pub const UTCB_VA: usize = TRAPFRAME_VA - PGSIZE; // UTCB 映射地址 0x3FFFFFD000
pub const STACK_VA: usize = UTCB_VA - PGSIZE; // 用户栈映射地址
pub const STACK_PAGES: usize = 16; // 用户栈页面数 16 * 4KB = 64KB
pub const STACK_SIZE: usize = STACK_PAGES * PGSIZE; // 64KB
pub const HEAP_PAGES: usize = 64; // 用户堆页面数 64 * 4KB = 256KB
pub const HEAP_SIZE: usize = HEAP_PAGES * PGSIZE; // 256KB
pub const HEAP_VA: usize = 0x2000_0000; // 用户堆地址
pub const RES_VA_BASE: usize = 0x4000_0000; // 启动时提供的资源

pub use addr::{PPN, PhysAddr, VPN, VirtAddr};

pub mod addr;
pub mod pmem;
pub mod vm;
pub mod vspace;
