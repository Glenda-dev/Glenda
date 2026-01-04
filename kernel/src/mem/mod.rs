pub const PGSIZE: usize = 4096;
pub const PGNUM: usize = PGSIZE / core::mem::size_of::<usize>(); // 2^9
pub const KERN_PAGES: usize = 8192;
pub const VA_MAX: usize = 1 << 38; // 256 GiB 虚拟地址空间上限
pub const EMPTY_VA: usize = 0x0; // 空虚拟地址
pub const TRAMPOLINE_VA: usize = VA_MAX - PGSIZE; // Trampoline 映射地址
pub const TRAPFRAME_VA: usize = TRAMPOLINE_VA - PGSIZE; // Trapframe 映射地址
pub const UTCB_VA: usize = TRAPFRAME_VA - PGSIZE; // UTCB 映射地址 0x3FFFFFD000
pub const STACK_VA: usize = UTCB_VA - PGSIZE; // 用户栈映射地址
pub const STACK_SIZE: usize = 16 * PGSIZE; // 16KB
pub const HEAP_SIZE: usize = 1024 * PGSIZE; // 1MB
pub const HEAP_VA: usize = 0x2000_0000; // 用户堆地址
pub const RES_VA_BASE: usize = 0x4000_0000; // 启动时提供的资源

pub use addr::{PPN, PhysAddr, VirtAddr};
pub use pagetable::PageTable;
pub use pte::{Pte, PteFlags};

pub mod addr;
pub mod pagetable;
pub mod pmem;
pub mod pte;
pub mod vm;
pub mod vspace;
