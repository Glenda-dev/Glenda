pub const PGSIZE: usize = 4096;
pub const PGNUM: usize = PGSIZE / core::mem::size_of::<usize>(); // 2^9
pub const KERN_PAGES: usize = 8192;

pub use addr::{BOOTINFO_VA, TRAMPOLINE_VA, TRAPFRAME_VA, UTCB_VA, VA_MAX};
pub use addr::{PPN, PhysAddr, VirtAddr};
pub use pagetable::PageTable;
pub use pte::{Pte, PteFlags};
pub use vspace::VSpace;

pub mod addr;
pub mod pagetable;
pub mod pmem;
pub mod pte;
pub mod vm;
pub mod vspace;
