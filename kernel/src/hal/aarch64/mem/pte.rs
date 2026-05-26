use super::PTEFLAGS_MASK;
use crate::mem::{PPN, Perms, PhysAddr};

const DESC_VALID: usize = 1 << 0;
const DESC_TABLE: usize = 1 << 1;
const DESC_PAGE: usize = 1 << 1; // At Level 3, bit 1 means page
const ATTRIDX_SHIFT: usize = 2;
const AP_RW_EL1: usize = 0 << 6;
const AP_RO_EL1: usize = 2 << 6;
const AP_USER: usize = 1 << 6;
const SH_INNER: usize = 0b11 << 8;
const AF: usize = 1 << 10;
const NG: usize = 1 << 11;
const UXN: usize = 1 << 54;
const PXN: usize = 1 << 53;
const ADDR_MASK: usize = 0x0000_ffff_ffff_f000;

#[derive(Clone, Copy, Debug)]
pub struct Pte(usize);

impl Pte {
    pub const fn null() -> Self {
        Self(0)
    }

    pub fn from(pa: PhysAddr, flags: Perms) -> Self {
        let mut bits = (pa.as_usize() & ADDR_MASK) | DESC_VALID;
        if flags.intersects(Perms::READ | Perms::WRITE | Perms::EXECUTE) {
            // Assume page descriptor for Level 3 or block for others
            // Actually Glenda walk_with_level assumes bit 1 is set for non-leaf.
            // For AArch64:
            // Levels 0-2: bit 1 = 1 is table, bit 1 = 0 is block.
            // Level 3: bit 1 = 1 is page.

            // We set bit 1 to 1 for now, assuming Page or Table.
            bits |= DESC_TABLE | AF | SH_INNER;
            bits |= if flags.contains(Perms::WRITE) { AP_RW_EL1 } else { AP_RO_EL1 };
            if flags.contains(Perms::USER) {
                bits |= AP_USER | NG;
            }
            if !flags.contains(Perms::EXECUTE) {
                bits |= UXN | PXN;
            }
            if flags.contains(Perms::DEVICE) {
                bits |= 1 << ATTRIDX_SHIFT;
            } else if flags.contains(Perms::FRAMEBUFFER) {
                bits |= 2 << ATTRIDX_SHIFT;
            }
        } else {
            // Table descriptor
            bits |= DESC_TABLE;
        }
        Self(bits & (ADDR_MASK | PTEFLAGS_MASK))
    }

    pub const fn as_usize(&self) -> usize {
        self.0
    }

    pub const fn get_ppn(&self) -> PPN {
        PPN::from((self.0 & ADDR_MASK) >> 12)
    }

    pub fn set_ppn(&mut self, ppn: PPN) {
        self.0 = (self.0 & PTEFLAGS_MASK) | (ppn.as_usize() << 12);
    }

    pub fn get_flags(&self) -> Perms {
        let mut perms = Perms::empty();
        if self.0 & DESC_VALID != 0 {
            perms |= Perms::VALID;
        }
        if self.is_leaf() {
            perms |= Perms::READ;
            if (self.0 & (AP_RO_EL1 | AP_USER)) == AP_RW_EL1 || (self.0 & AP_RO_EL1) == 0 {
                perms |= Perms::WRITE;
            }
            if (self.0 & (UXN | PXN)) != (UXN | PXN) {
                perms |= Perms::EXECUTE;
            }
            if self.0 & NG != 0 {
                perms |= Perms::USER;
            }
        }
        perms
    }

    pub fn set_flags(&mut self, flags: Perms) {
        let pa = self.pa();
        *self = Self::from(pa, flags);
    }

    pub fn is_valid(&self) -> bool {
        (self.0 & DESC_VALID) != 0
    }

    pub fn is_leaf(&self) -> bool {
        // Simple heuristic: if AF is set, it's definitely a leaf (page or block)
        self.is_valid() && (self.0 & AF) != 0
    }

    pub fn is_table(&self) -> bool {
        // Table descriptor must have bit 1 set and NOT be at Level 3?
        // Actually for our walk, we just need to know it's not a leaf.
        self.is_valid() && !self.is_leaf()
    }

    pub fn pa(&self) -> PhysAddr {
        PhysAddr::from(self.0 & ADDR_MASK)
    }
}
