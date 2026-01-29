mod pagetable;
mod pte;
mod vm;

pub use pagetable::PageTable;
pub use pte::perms as PtePerms;
pub use pte::{Pte, PteFlags};

pub const PGSIZE: usize = 4096;
pub const VA_MAX: usize = 1 << 38;
pub const PHYS_MAP_BASE: usize = 0;
pub const KERNEL_BASE: usize = 0;
pub const PT_LEVELS: usize = 3;
pub const PGNUM: usize = 512;
pub const PTEFLAGS_MASK: usize = 0x3FF;
pub const MAX_ASID: usize = 1 << 16;
pub const ASID_MASK: usize = 0xFFFF;

use super::asm;
use crate::mem::{PhysAddr, VPN, VirtAddr};

const SATP_MODE: usize = 8;

pub unsafe fn activate_vspace(val: usize) {
    unsafe {
        asm::write_satp(val);
        asm::sfence_vma_all();
    }
}
pub unsafe fn deactivate_vspace() {
    unsafe {
        asm::write_satp(0);
        asm::sfence_vma_all();
    }
}

pub fn get_mmu_register(root_paddr: PhysAddr, asid: usize) -> usize {
    (SATP_MODE << 60) | (root_paddr.as_usize() >> 12) | (asid & ASID_MASK) << 44
}

pub fn flush_tlb(vaddr: Option<VirtAddr>) {
    unsafe {
        match vaddr {
            Some(vaddr) => {
                asm::sfence_vma(vaddr.as_usize());
            }
            None => {
                asm::sfence_vma_all();
            }
        }
    }
}
pub fn get_vpn_index(va: VirtAddr, level: usize) -> VPN {
    let shift = 12 + level * 9;
    VPN::from((va.as_usize() >> shift) & 0x1FF)
}

pub fn kpt_setup(kpt: &mut PageTable) {
    vm::setup_trampoline(kpt);
    vm::setup_mmio(kpt);
}
