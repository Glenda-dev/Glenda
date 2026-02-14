use crate::mem::{PhysAddr, VirtAddr};
use crate::platform::MemoryType;
use crate::sync::Once;

#[cfg(feature = "bl-limine")]
pub mod limine;

#[cfg(feature = "bl-limine")]
pub use limine::{bootstrap, init};

#[derive(Debug, Clone, Copy)]
pub struct MemoryMapEntry {
    pub base: PhysAddr,
    pub length: usize,
    pub kind: MemoryType,
}

pub struct BootLoaderInfo {
    pub dtb_addr: Option<PhysAddr>,
    pub rsdp_addr: Option<PhysAddr>,
    pub hhdm_offset: usize,
    pub memory_map: &'static [MemoryMapEntry],
    pub kernel_address: (PhysAddr, VirtAddr),
    pub kernel_size: usize,
    pub initrd_addr: Option<(PhysAddr, usize)>,
    pub cmdline: Option<&'static str>,
    pub cpu_count: usize,
}

pub static BOOT_LOADER_INFO: Once<BootLoaderInfo> = Once::new();

pub fn get_dtb() -> Option<PhysAddr> {
    BOOT_LOADER_INFO.get().and_then(|info| info.dtb_addr)
}

pub fn get_initrd() -> Option<(PhysAddr, usize)> {
    BOOT_LOADER_INFO.get().and_then(|info| info.initrd_addr)
}

pub fn get_rsdp() -> Option<PhysAddr> {
    BOOT_LOADER_INFO.get().and_then(|info| info.rsdp_addr)
}

pub fn get_hhdm() -> usize {
    BOOT_LOADER_INFO.get().map(|info| info.hhdm_offset).unwrap_or(0)
}

pub fn get_mem_map() -> &'static [MemoryMapEntry] {
    BOOT_LOADER_INFO.get().map(|info| info.memory_map).unwrap_or(&[])
}

pub fn get_kernel_address() -> (PhysAddr, VirtAddr) {
    BOOT_LOADER_INFO
        .get()
        .map(|info| info.kernel_address)
        .unwrap_or((PhysAddr::null(), VirtAddr::null()))
}

pub fn get_kernel_size() -> usize {
    BOOT_LOADER_INFO.get().map(|info| info.kernel_size).unwrap_or(0)
}

pub fn get_cmdline() -> Option<&'static str> {
    BOOT_LOADER_INFO.get().and_then(|info| info.cmdline)
}

pub fn get_cpu_count() -> usize {
    BOOT_LOADER_INFO.get().map(|info| info.cpu_count).unwrap_or(1)
}
