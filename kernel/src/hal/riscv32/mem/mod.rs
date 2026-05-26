mod pte;
mod vm;

pub use pte::Pte;

pub const PGSIZE: usize = 4096;
#[cfg(target_pointer_width = "64")]
pub const VA_MAX: usize = 1 << 38;
#[cfg(target_pointer_width = "32")]
pub const VA_MAX: usize = usize::MAX;
pub const USER_VA: usize = 0x10000;
pub const TRAMPOLINE_VA: usize = VA_MAX - PGSIZE;
pub const STACK_BASE: usize = TRAMPOLINE_VA;
#[cfg(target_pointer_width = "64")]
pub const THREAD_AREA_BASE: usize = 0x3F_0000_0000;
#[cfg(target_pointer_width = "32")]
pub const THREAD_AREA_BASE: usize = 0x3000_0000;
pub const UTCB_VA: usize = THREAD_AREA_BASE;
pub const TRAPFRAME_VA: usize = THREAD_AREA_BASE + PGSIZE;
pub const HEAP_VA: usize = 0x2000_0000;
pub const BOOTINFO_VA: usize = 0x4000_0000;
pub const INITRD_VA: usize = 0x5000_0000;
#[cfg(target_pointer_width = "64")]
pub const PT_LEVELS: usize = 3;
#[cfg(target_pointer_width = "32")]
pub const PT_LEVELS: usize = 2;
#[cfg(target_pointer_width = "64")]
pub const PT_INDEX_BITS: usize = 9;
#[cfg(target_pointer_width = "32")]
pub const PT_INDEX_BITS: usize = 10;
#[cfg(target_pointer_width = "64")]
pub const PGNUM: usize = 512;
#[cfg(target_pointer_width = "32")]
pub const PGNUM: usize = 1024;
pub const PTEFLAGS_MASK: usize = 0x3FF;
#[cfg(target_pointer_width = "64")]
pub const MAX_ASID: usize = 1 << 16;
#[cfg(target_pointer_width = "32")]
pub const MAX_ASID: usize = 1 << 9;
#[cfg(target_pointer_width = "64")]
pub const ASID_MASK: usize = 0xFFFF;
#[cfg(target_pointer_width = "32")]
pub const ASID_MASK: usize = 0x1FF;
pub const KSTACK_PAGES: usize = 4; // 16KB

use super::asm;
use crate::mem::addr::{phys_to_virt, virt_to_phys};
use crate::mem::{PageTable, Perms, PhysAddr, VPN, VirtAddr};

#[cfg(target_pointer_width = "64")]
const SATP_MODE: usize = 8;
#[cfg(target_pointer_width = "32")]
const SATP_MODE: usize = 1;
unsafe extern "C" {
    static __trampoline: u8;
    static __kernel_pbase: u8;
}

pub unsafe fn activate_vspace(val: usize) {
    unsafe {
        asm::write_satp(val);
        asm::sfence_vma_global();
    }
}
pub unsafe fn deactivate_vspace() {
    unsafe {
        asm::write_satp(0);
        asm::sfence_vma_global();
    }
}

pub fn get_mmu_register(root_paddr: PhysAddr, asid: usize) -> usize {
    #[cfg(target_pointer_width = "64")]
    return (SATP_MODE << 60) | (root_paddr.as_usize() >> 12) | (asid & ASID_MASK) << 44;
    #[cfg(target_pointer_width = "32")]
    return (SATP_MODE << 31) | (root_paddr.as_usize() >> 12) | (asid & ASID_MASK) << 22;
}

pub fn flush_tlb(vaddr: Option<VirtAddr>, size: usize, asid: Option<usize>) {
    unsafe {
        match (vaddr, asid) {
            (Some(vaddr), Some(asid)) => {
                if size <= PGSIZE {
                    asm::sfence_vma(vaddr.as_usize(), asid);
                } else {
                    let mut current = vaddr.as_usize();
                    let end = current + size;
                    while current < end {
                        asm::sfence_vma(current, asid);
                        current += PGSIZE;
                    }
                }
                // 远程刷新
                super::sbi::remote_sfence_vma_asid(
                    usize::MAX, // All harts in mask base
                    0,          // Mask base 0
                    vaddr.as_usize(),
                    size,
                    asid,
                );
            }
            (None, Some(asid)) => {
                asm::sfence_vma_all(asid);
                // 远程全刷该 ASID
                super::sbi::remote_sfence_vma_asid(usize::MAX, 0, 0, usize::MAX, asid);
            }
            (Some(vaddr), None) => {
                if size <= PGSIZE {
                    asm::sfence_vma(vaddr.as_usize(), 0);
                } else {
                    let mut current = vaddr.as_usize();
                    let end = current + size;
                    while current < end {
                        asm::sfence_vma(current, 0);
                        current += PGSIZE;
                    }
                }
                // 远程刷新 (无 ASID)
                super::sbi::remote_sfence_vma(usize::MAX, 0, vaddr.as_usize(), size);
            }
            (None, None) => {
                asm::sfence_vma_global();
                // 远程全局全刷
                super::sbi::remote_sfence_vma(usize::MAX, 0, 0, usize::MAX);
            }
        }
    }
}
pub fn get_vpn_index(va: VirtAddr, level: usize) -> VPN {
    let shift = 12 + level * PT_INDEX_BITS;
    #[cfg(target_pointer_width = "64")]
    let mask = 0x1FF;
    #[cfg(target_pointer_width = "32")]
    let mask = 0x3FF;
    VPN::from((va.as_usize() >> shift) & mask)
}

pub const fn page_size_for_level(level: usize) -> usize {
    1usize << (12 + level * PT_INDEX_BITS)
}

pub fn kpt_setup(kpt: &mut PageTable) {
    vm::setup_trampoline(kpt);
    vm::setup_mmio(kpt);
}

pub fn pt_setup(pt: &mut PageTable) -> Result<(), crate::error::Error> {
    let tramp_va_sym = VirtAddr::from(unsafe { &__trampoline as *const u8 as usize });
    let tramp_pa = virt_to_phys(tramp_va_sym);
    pt.map(VirtAddr::from(TRAMPOLINE_VA), tramp_pa, PGSIZE, Perms::READ | Perms::EXECUTE)
}

pub fn get_pt() -> &'static mut PageTable {
    let satp = asm::read_satp();
    #[cfg(target_pointer_width = "64")]
    let root_ppn = satp & ((1 << 44) - 1);
    #[cfg(target_pointer_width = "32")]
    let root_ppn = satp & ((1 << 22) - 1);
    let root_paddr = PhysAddr::from(root_ppn << 12);
    let root_vaddr = phys_to_virt(root_paddr);
    unsafe { &mut *root_vaddr.as_mut_ptr::<PageTable>() }
}
