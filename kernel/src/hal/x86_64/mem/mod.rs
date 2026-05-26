mod pte;

pub use pte::Pte;

use core::arch::asm;

use crate::error::Error;
use crate::mem::addr::{phys_to_virt, virt_to_phys};
use crate::mem::{PageTable, Perms, PhysAddr, VPN, VirtAddr};

pub const PGSIZE: usize = 4096;
pub const VA_MAX: usize = 1 << 47;
pub const USER_VA: usize = 0x400000;
pub const TRAMPOLINE_VA: usize = VA_MAX - PGSIZE;
pub const STACK_BASE: usize = TRAMPOLINE_VA;
pub const THREAD_AREA_BASE: usize = 0x3F_0000_0000;
pub const UTCB_VA: usize = THREAD_AREA_BASE;
pub const TRAPFRAME_VA: usize = THREAD_AREA_BASE + PGSIZE;
pub const HEAP_VA: usize = 0x2000_0000;
pub const BOOTINFO_VA: usize = 0x4000_0000;
pub const INITRD_VA: usize = 0x5000_0000;
pub const PT_LEVELS: usize = 4;
pub const PGNUM: usize = 512;
pub const PT_INDEX_BITS: usize = 9;
pub const PTEFLAGS_MASK: usize = 0x8000_0000_0000_0fff;
pub const MAX_ASID: usize = 1 << 12;
pub const ASID_MASK: usize = MAX_ASID - 1;
pub const KSTACK_PAGES: usize = 4;

unsafe extern "C" {
    static __trampoline: u8;
    static __alloc_start: u8;
}

#[inline(always)]
fn read_cr3() -> usize {
    let cr3: usize;
    unsafe {
        asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
    }
    cr3
}

#[inline(always)]
unsafe fn write_cr3(cr3: usize) {
    unsafe {
        asm!("mov cr3, {}", in(reg) cr3, options(nostack, preserves_flags));
    }
}

pub unsafe fn activate_vspace(val: usize) {
    unsafe {
        write_cr3(val & !0xfff);
    }
}

pub unsafe fn deactivate_vspace() {
    unsafe {
        write_cr3(0);
    }
}

pub fn get_mmu_register(root_paddr: PhysAddr, _asid: usize) -> usize {
    root_paddr.as_usize() & !0xfff
}

pub fn flush_tlb(vaddr: Option<VirtAddr>, size: usize, _asid: Option<usize>) {
    match vaddr {
        Some(addr) if size <= PGSIZE => unsafe {
            asm!("invlpg [{}]", in(reg) addr.as_usize(), options(nostack, preserves_flags));
        },
        Some(addr) => {
            let mut current = addr.as_usize();
            let end = current.saturating_add(size);
            while current < end {
                unsafe {
                    asm!("invlpg [{}]", in(reg) current, options(nostack, preserves_flags));
                }
                current += PGSIZE;
            }
        }
        None => unsafe {
            write_cr3(read_cr3());
        },
    }
}

pub fn get_vpn_index(va: VirtAddr, level: usize) -> VPN {
    let shift = 12 + level * PT_INDEX_BITS;
    VPN::from((va.as_usize() >> shift) & 0x1ff)
}

pub const fn page_size_for_level(level: usize) -> usize {
    1usize << (12 + level * PT_INDEX_BITS)
}

pub fn kpt_setup(_kpt: &mut PageTable) {}

pub fn pt_setup(pt: &mut PageTable) -> Result<(), Error> {
    let tramp_va = VirtAddr::from(unsafe { &__trampoline as *const u8 as usize });
    let tramp_pa = virt_to_phys(tramp_va);
    pt.map(
        VirtAddr::from(TRAMPOLINE_VA),
        tramp_pa,
        PGSIZE,
        Perms::READ | Perms::EXECUTE | Perms::VALID,
    )
}

pub fn kernel_end_addr() -> PhysAddr {
    virt_to_phys(VirtAddr::from(unsafe { &__alloc_start as *const u8 as usize }))
}

pub fn get_pt() -> &'static mut PageTable {
    let root_paddr = PhysAddr::from(read_cr3() & !0xfff);
    let root_vaddr = phys_to_virt(root_paddr);
    unsafe { root_vaddr.as_mut::<PageTable>() }
}
