pub const PGSIZE: usize = 4096;
pub const PGNUM: usize = PGSIZE / core::mem::size_of::<usize>(); // 2^9
pub const PGMASK: usize = PGSIZE - 1;
pub const VA_MAX: usize = 1 << 38;
pub const KERN_PAGES: usize = 8192;

pub use addr::{PhysAddr, VirtAddr};
pub use frame::PhysFrame;
pub use pagetable::PageTable;
pub use vm::KernelStack;
pub use vspace::VSpace;

pub mod addr;
pub mod alloc;
pub mod frame;
pub mod pagetable;
pub mod pmem;
pub mod pte;
pub mod uvm;
pub mod vm;
pub mod vspace;
