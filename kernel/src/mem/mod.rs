use crate::hal::mem::{PGSIZE, VA_MAX};
use bitflags::bitflags;
use core::fmt::Display;

pub const TRAMPOLINE_VA: usize = VA_MAX - PGSIZE; // Trampoline 映射地址
pub const TRAPFRAME_VA: usize = TRAMPOLINE_VA - PGSIZE; // Trapframe 映射地址
pub const UTCB_VA: usize = TRAPFRAME_VA - PGSIZE; // UTCB 映射地址 0x3FFFFFD000

pub use addr::{PPN, PhysAddr, VPN, VirtAddr};
pub use pagetable::PageTable;
pub use pmem::{MemoryRange, PhysFrame};
pub use untyped::UntypedRegion;

pub mod addr;
pub mod pagetable;
pub mod pmem;
pub mod untyped;
pub mod vm;

bitflags! {
    #[derive(Clone, Copy)]
    pub struct Perms: usize {
        const VALID = 1 << 0;
        const READ = 1 << 1;
        const WRITE = 1 << 2;
        const EXECUTE = 1 << 3;
        const USER = 1 << 4;
        const GLOBAL = 1 << 5;
        const ACCESSED = 1 << 6;
        const DIRTY= 1 << 7;
    }
}

impl Display for Perms {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut first = true;
        let perms = [
            (Perms::VALID, "V"),
            (Perms::READ, "R"),
            (Perms::WRITE, "W"),
            (Perms::EXECUTE, "X"),
            (Perms::USER, "U"),
            (Perms::GLOBAL, "G"),
        ];
        for (bit, name) in perms.iter() {
            if self.contains(*bit) {
                if !first {
                    write!(f, "|")?;
                }
                write!(f, "{}", name)?;
                first = false;
            }
        }
        if first {
            write!(f, "NONE")?;
        }
        Ok(())
    }
}
