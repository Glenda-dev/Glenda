use super::PTEFLAGS_MASK;
use crate::mem::{PPN, Perms, PhysAddr};

#[cfg(target_pointer_width = "64")]
const PPN_MASK: usize = PPN_MASK;
#[cfg(target_pointer_width = "32")]
const PPN_MASK: usize = 0x3FFFFF;


#[derive(Clone, Copy, Debug)]
pub struct Pte(usize);

impl Pte {
    pub const fn null() -> Self {
        Self(0)
    }
    pub fn from(pa: PhysAddr, flags: Perms) -> Self {
        Self((((pa.as_usize() >> 12) & PPN_MASK) << 10) | convert_flags(flags))
    }
    pub const fn as_usize(&self) -> usize {
        self.0
    }
    pub const fn get_ppn(&self) -> PPN {
        PPN::from((self.0 >> 10) & PPN_MASK)
    }
    pub const fn set_ppn(&mut self, ppn: PPN) {
        self.0 = (self.0 & PTEFLAGS_MASK) | (ppn.as_usize() << 10)
    }
    pub fn get_flags(&self) -> Perms {
        convert_to_perms(self.0 & PTEFLAGS_MASK)
    }
    pub fn set_flags(&mut self, flags: Perms) {
        self.0 = (self.0 & PTEFLAGS_MASK) | convert_flags(flags)
    }
    pub fn is_valid(&self) -> bool {
        let flags = self.get_flags();
        flags.contains(Perms::VALID)
    }
    pub fn is_leaf(&self) -> bool {
        let flags = self.get_flags();
        flags.intersects(Perms::READ | Perms::WRITE | Perms::EXECUTE)
    }
    pub fn is_table(&self) -> bool {
        self.is_valid() && !self.is_leaf()
    }
    pub const fn pa(&self) -> PhysAddr {
        PhysAddr::from(self.get_ppn().as_usize() << 12)
    }
}

fn convert_flags(perms: Perms) -> usize {
    let mut bits = PTE_V;
    if perms.intersects(Perms::READ | Perms::WRITE | Perms::EXECUTE) {
        bits |= PTE_A | PTE_D;
    }
    if perms.contains(Perms::READ) {
        bits |= PTE_R;
    }
    if perms.contains(Perms::WRITE) {
        bits |= PTE_W;
    }
    if perms.contains(Perms::EXECUTE) {
        bits |= PTE_X;
    }
    if perms.contains(Perms::USER) {
        bits |= PTE_U;
    }
    if perms.contains(Perms::GLOBAL) {
        bits |= PTE_G;
    }
    bits & PTEFLAGS_MASK
}

fn convert_to_perms(flags: usize) -> Perms {
    let mut perms = Perms::empty();
    if flags & PTE_R != 0 {
        perms.insert(Perms::READ);
    }
    if flags & PTE_W != 0 {
        perms.insert(Perms::WRITE);
    }
    if flags & PTE_X != 0 {
        perms.insert(Perms::EXECUTE);
    }
    if flags & PTE_U != 0 {
        perms.insert(Perms::USER);
    }
    if flags & PTE_G != 0 {
        perms.insert(Perms::GLOBAL);
    }
    if flags & PTE_V != 0 {
        perms.insert(Perms::VALID);
    }
    perms
}

const PTE_V: usize = 1 << 0;
const PTE_R: usize = 1 << 1;
const PTE_W: usize = 1 << 2;
const PTE_X: usize = 1 << 3;
const PTE_U: usize = 1 << 4;
const PTE_G: usize = 1 << 5;
const PTE_A: usize = 1 << 6;
const PTE_D: usize = 1 << 7;
