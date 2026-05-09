use crate::mem::{PPN, Perms, PhysAddr};

use super::PTEFLAGS_MASK;

const PTE_PRESENT: usize = 1 << 0;
const PTE_WRITABLE: usize = 1 << 1;
const PTE_USER: usize = 1 << 2;
const PTE_PWT: usize = 1 << 3;
const PTE_PCD: usize = 1 << 4;
const PTE_GLOBAL: usize = 1 << 8;
const PTE_SOFT_LEAF: usize = 1 << 9;
const PTE_NX: usize = 1usize << 63;
const PTE_ADDR_MASK: usize = 0x000f_ffff_ffff_f000;

#[derive(Clone, Copy, Debug)]
pub struct Pte(usize);

impl Pte {
    pub const fn null() -> Self {
        Self(0)
    }

    pub fn from(pa: PhysAddr, flags: Perms) -> Self {
        let mut bits = pa.as_usize() & PTE_ADDR_MASK;
        if flags.contains(Perms::VALID) {
            bits |= PTE_PRESENT;
        }
        if flags.contains(Perms::WRITE) {
            bits |= PTE_WRITABLE;
        }
        if flags.contains(Perms::USER) {
            bits |= PTE_USER;
        }
        if flags.contains(Perms::GLOBAL) {
            bits |= PTE_GLOBAL;
        }
        if flags.contains(Perms::DEVICE) {
            bits |= PTE_PCD | PTE_PWT;
        }
        if flags.contains(Perms::FRAMEBUFFER) {
            bits |= PTE_PCD;
        }
        if flags.intersects(Perms::READ | Perms::WRITE | Perms::EXECUTE) {
            bits |= PTE_SOFT_LEAF;
        }
        if !flags.contains(Perms::EXECUTE) {
            bits |= PTE_NX;
        }
        Self(bits)
    }

    pub const fn as_usize(&self) -> usize {
        self.0
    }

    pub const fn get_ppn(&self) -> PPN {
        PPN::from((self.0 & PTE_ADDR_MASK) >> 12)
    }

    pub fn set_ppn(&mut self, ppn: PPN) {
        self.0 = (self.0 & PTEFLAGS_MASK) | (ppn.as_usize() << 12);
    }

    pub fn get_flags(&self) -> Perms {
        let mut perms = Perms::empty();
        if (self.0 & PTE_PRESENT) != 0 {
            perms |= Perms::VALID;
        }
        if (self.0 & PTE_SOFT_LEAF) != 0 {
            perms |= Perms::READ;
        }
        if (self.0 & PTE_WRITABLE) != 0 {
            perms |= Perms::WRITE;
        }
        if (self.0 & PTE_USER) != 0 {
            perms |= Perms::USER;
        }
        if (self.0 & PTE_GLOBAL) != 0 {
            perms |= Perms::GLOBAL;
        }
        if (self.0 & PTE_PCD) != 0 {
            perms |= Perms::DEVICE;
        }
        if (self.0 & PTE_NX) == 0 && (self.0 & PTE_SOFT_LEAF) != 0 {
            perms |= Perms::EXECUTE;
        }
        perms
    }

    pub fn set_flags(&mut self, flags: Perms) {
        let pa = self.pa();
        *self = Self::from(pa, flags);
    }

    pub fn is_valid(&self) -> bool {
        (self.0 & PTE_PRESENT) != 0
    }

    pub fn is_leaf(&self) -> bool {
        (self.0 & PTE_SOFT_LEAF) != 0
    }

    pub fn is_table(&self) -> bool {
        self.is_valid() && !self.is_leaf()
    }

    pub fn pa(&self) -> PhysAddr {
        PhysAddr::from(self.0 & PTE_ADDR_MASK)
    }
}
