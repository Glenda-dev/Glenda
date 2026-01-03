use core::{
    fmt::Debug,
    ops::{Add, BitAnd, BitAndAssign, BitOr, BitOrAssign, Sub},
};

use super::{PPN, PhysAddr};
#[derive(Clone, Copy, Debug)]
pub struct Pte(usize);
#[derive(Clone, Copy)]
pub struct PteFlags(usize);

pub const PTEFLAGS_MASK: usize = 0x3FF;

pub mod perms {
    pub const VALID: usize = 1 << 0;
    pub const READ: usize = 1 << 1;
    pub const WRITE: usize = 1 << 2;
    pub const EXECUTE: usize = 1 << 3;
    pub const USER: usize = 1 << 4;
    pub const GLOBAL: usize = 1 << 5;
    pub const ACCESSED: usize = 1 << 6;
    pub const DIRTY: usize = 1 << 7;
}

impl Pte {
    pub const fn null() -> Self {
        Self(0)
    }
    pub const fn from(pa: PhysAddr, flags: PteFlags) -> Self {
        Self((((pa.as_usize() >> 12) & 0xFFFFFFFFFFF) << 10) | (flags.as_usize() & PTEFLAGS_MASK))
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
    pub const fn get_flags(&self) -> PteFlags {
        PteFlags::from((self.0 & PTEFLAGS_MASK) as usize)
    }
    pub const fn set_flags(&mut self, flags: PteFlags) {
        self.0 = (self.0 & PTEFLAGS_MASK) | flags.as_usize()
    }
    pub const fn is_valid(&self) -> bool {
        (self.0 & perms::VALID as usize) != 0
    }
    pub const fn is_leaf(&self) -> bool {
        (self.0 & (perms::READ | perms::WRITE | perms::EXECUTE) as usize) != 0
    }
    pub const fn is_table(&self) -> bool {
        self.is_valid() && !self.is_leaf()
    }
    pub const fn pa(&self) -> PhysAddr {
        PhysAddr::from(self.get_ppn().as_usize() << 12)
    }
}

impl PteFlags {
    pub const fn null() -> Self {
        Self(0)
    }
    pub const fn from(value: usize) -> Self {
        Self(value & PTEFLAGS_MASK)
    }
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

impl BitOr<usize> for PteFlags {
    type Output = PteFlags;
    fn bitor(self, flags: usize) -> PteFlags {
        PteFlags(self.0 | flags)
    }
}
impl BitOrAssign<usize> for PteFlags {
    fn bitor_assign(&mut self, rhs: usize) {
        self.0 = self.0 | rhs;
    }
}

impl BitAnd<usize> for PteFlags {
    type Output = PteFlags;
    fn bitand(self, flags: usize) -> PteFlags {
        PteFlags(self.0 & flags)
    }
}
impl BitAndAssign<usize> for PteFlags {
    fn bitand_assign(&mut self, rhs: usize) {
        self.0 = self.0 & rhs;
    }
}

impl Add for PteFlags {
    type Output = PteFlags;
    fn add(self, rhs: PteFlags) -> PteFlags {
        PteFlags(self.0 | rhs.0)
    }
}

impl Sub for PteFlags {
    type Output = PteFlags;
    fn sub(self, rhs: PteFlags) -> PteFlags {
        PteFlags(self.0 & !rhs.0)
    }
}

impl Debug for PteFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut first = true;
        let perms = [
            (perms::VALID, "V"),
            (perms::READ, "R"),
            (perms::WRITE, "W"),
            (perms::EXECUTE, "X"),
            (perms::USER, "U"),
            (perms::GLOBAL, "G"),
            (perms::ACCESSED, "A"),
            (perms::DIRTY, "D"),
        ];
        for (bit, name) in perms.iter() {
            if (self.0 & *bit) != 0 {
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
