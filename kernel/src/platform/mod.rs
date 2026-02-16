use crate::boot;
use crate::mem::{PhysAddr, VirtAddr};

pub mod acpi;
pub mod dtb;

pub const MAX_MEMORY_REGIONS: usize = 128;

/// 描述一块物理内存区域
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub start: PhysAddr,
    pub size: usize,
    pub region_type: MemoryType,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    Ram = 1,
    Mmio = 2,
    Reserved = 3,
    Reclaimable = 4,
}

pub fn init() {
    if let Some(rsdp_pa) = boot::get_rsdp() {
        self::acpi::parse(rsdp_pa.as_usize());
    } else if let Some((dtb_pa, _)) = crate::boot::get_dtb() {
        self::dtb::parse(dtb_pa.as_usize());
    } else {
        log!("platform: No DTB or ACPI found. Continuing anyway...");
    }
}

pub fn get_dtb() -> Option<VirtAddr> {
    boot::get_dtb().map(|(pa, _)| pa)
}

pub fn get_rsdp() -> Option<VirtAddr> {
    boot::get_rsdp()
}
