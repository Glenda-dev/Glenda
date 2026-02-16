use crate::mem::{PhysAddr, VirtAddr};
use crate::mem::Perms;

pub const PGSIZE: usize = 4096;
pub const VA_MAX: usize = 1 << 47;
pub const USER_VA: usize = 0x10000;
pub const KSTACK_PAGES: usize = 4;
pub const PT_LEVELS: usize = 4;
pub const PGNUM: usize = 512;
pub const MAX_ASID: usize = 1024;
pub const ASID_MASK: usize = 0x3FF;

pub const PTE_V: u64 = 1 << 0;
pub const PTE_D: u64 = 1 << 1;
pub const PTE_PLV_U: u64 = 3 << 2;
pub const PTE_MAT_CACHED: u64 = 1 << 4;
pub const PTE_P: u64 = 1 << 7;
pub const PTE_W: u64 = 1 << 8;

#[repr(transparent)]
#[derive(Copy, Clone, Debug, Default)]
pub struct Pte(u64);

impl Pte {
    pub const fn new(val: u64) -> Self { Self(val) }
    pub const fn null() -> Self { Self(0) }
    pub fn bits(&self) -> u64 { self.0 }

    pub fn from(pa: PhysAddr, perms: Perms) -> Self {
        let ppn_bits = (pa.as_usize() as u64) & 0x0000_FFFF_FFFF_F000;
        let mut bits = ppn_bits | PTE_V | PTE_P | PTE_MAT_CACHED;

        if perms.contains(Perms::USER) {
            bits |= PTE_PLV_U;
        }
        if perms.contains(Perms::WRITE) {
            bits |= PTE_W | PTE_D;
        }
        Self(bits)
    }

    pub fn is_valid(&self) -> bool {
        (self.0 & PTE_V) != 0
    }

    pub fn is_leaf(&self) -> bool {
        (self.0 & PTE_P) != 0
    }

    pub fn pa(&self) -> PhysAddr {
        PhysAddr::from((self.0 & 0x0000_FFFF_FFFF_F000) as usize)
    }

    pub fn get_flags(&self) -> Perms {
        let mut perms = Perms::empty();
        if (self.0 & PTE_PLV_U) != 0 { perms |= Perms::USER; }
        if (self.0 & PTE_W) != 0 { perms |= Perms::WRITE; }
        perms | Perms::READ
    }
}

impl From<PhysAddr> for Pte {
    fn from(pa: PhysAddr) -> Self {
        Self::from(pa, Perms::READ)
    }
}

pub const DMW_MASK: usize = 0x0000_FFFF_FFFF_FFFF;
pub const DMW_DIRECT: usize = 0x9000_0000_0000_0000;
pub const DMW_UNCACHED: usize = 0xa000_0000_0000_0000;

pub fn phys_to_virt(pa: PhysAddr) -> VirtAddr {
    VirtAddr::from(pa.as_usize() | DMW_DIRECT)
}

pub fn virt_to_phys(va: VirtAddr) -> PhysAddr {
    PhysAddr::from(va.as_usize() & DMW_MASK)
}

pub fn get_vpn_index(va: VirtAddr, level: usize) -> crate::mem::addr::VPN {
    let va_masked = va.as_usize() & DMW_MASK;
    let shift = 12 + level * 9;
    crate::mem::addr::VPN::from((va_masked >> shift) & 0x1FF)
}

pub fn flush_tlb(_asid: Option<usize>) {
    unsafe {
        core::arch::asm!("invtlb 0x0, $r0, $r0");
    }
}

pub fn pt_setup(_pt: &mut crate::mem::PageTable) -> Result<(), crate::error::Error> {
    Ok(())
}

pub fn kpt_setup(_pt: &mut crate::mem::PageTable) {
}

pub fn get_mmu_register(paddr: PhysAddr, _asid: usize) -> usize {
    paddr.as_usize()
}

pub fn activate_vspace(_reg: usize) {
}

pub fn deactivate_vspace() {
}

pub fn kernel_end_addr() -> PhysAddr {
    unsafe {
        unsafe extern "C" {
            fn __kernel_image_end();
        }
        PhysAddr::from(__kernel_image_end as usize)
    }
}
