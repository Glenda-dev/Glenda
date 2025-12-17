pub const PGSIZE: usize = 4096;
pub const PGNUM: usize = PGSIZE / core::mem::size_of::<usize>(); // 2^9
pub const PGMASK: usize = PGSIZE - 1;
pub const VA_MAX: usize = 1 << 38;
pub const KERN_PAGES: usize = 8192;
pub const MMAP_END: usize = VA_MAX - (16 * 256 + 2) * PGSIZE;
pub const MMAP_BEGIN: usize = MMAP_END - 64 * 256 * PGSIZE;

pub use addr::{PhysAddr, VirtAddr};
pub use frame::PhysFrame;
pub use pagetable::PageTable;

pub mod addr;
pub mod alloc;
pub mod frame;
pub mod mmap;
pub mod pagetable;
pub mod pmem;
pub mod pte;
pub mod uvm;
pub mod vm;
