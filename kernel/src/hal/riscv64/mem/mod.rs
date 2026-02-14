mod pte;
mod vm;

pub use pte::Pte;

pub const PGSIZE: usize = 4096;
pub const VA_MAX: usize = 1 << 38;
pub const USER_VA: usize = 0x10000;
pub const PT_LEVELS: usize = 3;
pub const PGNUM: usize = 512;
pub const PTEFLAGS_MASK: usize = 0x3FF;
pub const MAX_ASID: usize = 1 << 16;
pub const ASID_MASK: usize = 0xFFFF;
pub const KSTACK_PAGES: usize = 4; // 16KB

use super::asm;
use crate::mem::TRAMPOLINE_VA;
pub use crate::mem::addr::phys_to_virt;
pub use crate::mem::addr::virt_to_phys;
use crate::mem::{PageTable, Perms, PhysAddr, VPN, VirtAddr};

const SATP_MODE: usize = 8;
unsafe extern "C" {
    static __trampoline: u8;
}

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

pub fn pt_setup(pt: &mut PageTable) -> Result<(), crate::error::Error> {
    let tramp_pa = PhysAddr::from(unsafe { &__trampoline as *const u8 as usize });
    pt.map(VirtAddr::from(TRAMPOLINE_VA), tramp_pa, PGSIZE, Perms::READ | Perms::EXECUTE)
}

pub fn get_pt() -> &'static mut PageTable {
    let satp = asm::read_satp();
    let root_ppn = satp & ((1 << 44) - 1);
    let root_paddr = PhysAddr::from(root_ppn << 12);
    let root_vaddr = phys_to_virt(root_paddr);
    unsafe { &mut *root_vaddr.as_mut_ptr::<PageTable>() }
}
