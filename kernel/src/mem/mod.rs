pub const PGSIZE: usize = 4096;
pub const PGNUM: usize = PGSIZE / core::mem::size_of::<usize>(); // 2^9
pub const KERN_PAGES: usize = 8192;

pub use addr::{PPN, PhysAddr, VA_MAX, VirtAddr};
pub use kstack::KernelStack;
pub use pagetable::PageTable;
pub use pte::{Pte, PteFlags};
pub use vspace::VSpace;

pub mod addr;
pub mod kstack;
pub mod pagetable;
pub mod pmem;
pub mod pte;
pub mod vm;
pub mod vspace;
