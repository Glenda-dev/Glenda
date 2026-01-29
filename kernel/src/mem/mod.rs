use crate::hal::mem::{PGSIZE, VA_MAX};

pub const TRAMPOLINE_VA: usize = VA_MAX - PGSIZE; // Trampoline 映射地址
pub const TRAPFRAME_VA: usize = TRAMPOLINE_VA - PGSIZE; // Trapframe 映射地址
pub const UTCB_VA: usize = TRAPFRAME_VA - PGSIZE; // UTCB 映射地址 0x3FFFFFD000

pub use addr::{PPN, PhysAddr, VPN, VirtAddr};

pub mod addr;
pub mod pmem;
pub mod vm;
