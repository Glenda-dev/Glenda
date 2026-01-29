use crate::mem::{PPN, Perms, PhysAddr};

const PTEFLAGS_MASK: usize = 0x3FF;

#[derive(Clone, Copy, Debug)]
pub struct Pte(usize);

impl Pte {
    pub const fn null() -> Self {
        Self(0)
    }
    pub const fn from(pa: PhysAddr, flags: Perms) -> Self {
        Self((((pa.as_usize() >> 12) & 0xFFFFFFFFFFF) << 10) | convert_flags(flags))
    }
    pub const fn as_usize(&self) -> usize {
        self.0
    }
    pub const fn get_ppn(&self) -> PPN {
        PPN::from((self.0 >> 10) & 0xFFFFFFFFFFF)
    }
    pub const fn set_ppn(&mut self, ppn: PPN) {
        self.0 = (self.0 & PTEFLAGS_MASK) | (ppn.as_usize() << 10)
    }
    pub const fn get_flags(&self) -> Perms {
        convert_to_perms(self.0 & PTEFLAGS_MASK)
    }
    pub const fn set_flags(&mut self, flags: Perms) {
        self.0 = (self.0 & PTEFLAGS_MASK) | convert_flags(flags)
    }
    pub const fn is_valid(&self) -> bool {
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

const fn convert_flags(perms: Perms) -> usize {
    perms.bits() & PTEFLAGS_MASK
}

const fn convert_to_perms(flags: usize) -> Perms {
    Perms::from_bits_truncate(flags & PTEFLAGS_MASK)
}
