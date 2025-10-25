#![allow(dead_code)]

pub const PGSIZE: usize = 4096;
pub const PGNUM: usize = PGSIZE / core::mem::size_of::<usize>(); // 2^9
pub const PGMASK: usize = PGSIZE - 1;
pub const VA_MAX: usize = 1 << 38;
pub const KERN_PAGES: usize = 8192;

pub mod addr;
pub mod pgtbl;
pub mod pmem;
pub mod pte;
pub mod vm;
