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
use crate::boot::MemoryMapEntry;
use crate::mem::TRAMPOLINE_VA;
use crate::mem::addr::{phys_to_virt, virt_to_phys};
use crate::mem::{PageTable, Perms, PhysAddr, VPN, VirtAddr};
use crate::platform::MemoryType;

const SATP_MODE: usize = 8;
unsafe extern "C" {
    static __trampoline: u8;
    static __kernel_pbase: u8;
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

#[unsafe(no_mangle)]
#[unsafe(link_section = ".data.boot_pt")]
pub static mut BOOT_PAGE_TABLE: PageTable = PageTable::new();

/// 重写后的初始页表建立：使用提供的内存区域列表建立恒等映射
pub unsafe fn setup_boot_pagetable(regions: &[MemoryMapEntry]) -> usize {
    let pbase = &raw const __kernel_pbase as usize;
    log!("hal: Setup boot pagetable: pbase={:#x}, regions={}", pbase, regions.len());

    let flags = Perms::VALID
        | Perms::READ
        | Perms::WRITE
        | Perms::EXECUTE
        | Perms::GLOBAL
        | Perms::ACCESSED
        | Perms::DIRTY;

    // 清零页表（针对多核启动时可能的并发或重复调用）
    unsafe {
        for i in 0..512 {
            BOOT_PAGE_TABLE.entries[i] = Pte::null();
        }
    }

    // 1. 根据提供的 RAM 范围建立恒等映射 (使用 1GB 大页)
    for entry in regions {
        if entry.kind == MemoryType::Ram {
            let start = entry.base.as_usize();
            let size = entry.length;
            let start_gb = start >> 30;
            let end_gb = (start + size + (1 << 30) - 1) >> 30;
            for gb in start_gb..end_gb {
                if gb < 512 {
                    unsafe {
                        BOOT_PAGE_TABLE.entries[gb] = Pte::from(PhysAddr::from(gb << 30), flags);
                    }
                }
            }
        }
    }

    // 2. 映射关键 MMIO 区域（前 1GB 通常包含 UART、PLIC 等外设）
    unsafe {
        if !BOOT_PAGE_TABLE.entries[0].is_valid() {
            BOOT_PAGE_TABLE.entries[0] = Pte::from(PhysAddr::from(0), flags);
        }
    }

    // 3. 映射内核镜像所在区域
    let kernel_gb = pbase >> 30;
    unsafe {
        if !BOOT_PAGE_TABLE.entries[kernel_gb].is_valid() {
            BOOT_PAGE_TABLE.entries[kernel_gb] = Pte::from(PhysAddr::from(kernel_gb << 30), flags);
        }
    }

    let root_pa = PhysAddr::from(&raw const BOOT_PAGE_TABLE as usize);
    get_mmu_register(root_pa, 0)
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
    let tramp_va_sym = VirtAddr::from(unsafe { &__trampoline as *const u8 as usize });
    let tramp_pa = virt_to_phys(tramp_va_sym);
    pt.map(VirtAddr::from(TRAMPOLINE_VA), tramp_pa, PGSIZE, Perms::READ | Perms::EXECUTE)
}

pub fn get_pt() -> &'static mut PageTable {
    let satp = asm::read_satp();
    let root_ppn = satp & ((1 << 44) - 1);
    let root_paddr = PhysAddr::from(root_ppn << 12);
    let root_vaddr = phys_to_virt(root_paddr);
    unsafe { &mut *root_vaddr.as_mut_ptr::<PageTable>() }
}
