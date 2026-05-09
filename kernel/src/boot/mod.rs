use crate::mem::{PhysAddr, VirtAddr};
use crate::platform::MemoryType;
use crate::sync::Once;

#[cfg(feature = "bl-limine")]
pub mod limine;

#[cfg(feature = "bl-uefi")]
pub mod uefi;

#[cfg(feature = "bl-uboot")]
pub mod uboot;

#[cfg(feature = "bl-linuxboot")]
pub mod linuxboot;

#[cfg(feature = "bl-multiboot2")]
pub mod multiboot2;

#[cfg(feature = "bl-limine")]
pub use limine::{bootstrap, init};

#[cfg(feature = "bl-uefi")]
pub use uefi::{bootstrap, init};

#[cfg(feature = "bl-uboot")]
pub use uboot::{bootstrap, init};

#[cfg(feature = "bl-linuxboot")]
pub use linuxboot::{bootstrap, init};

#[cfg(feature = "bl-multiboot2")]
pub use multiboot2::{bootstrap, init};

pub mod api;
#[cfg(not(any(
    feature = "bl-limine",
    feature = "bl-uefi",
    feature = "bl-uboot",
    feature = "bl-linuxboot",
    feature = "bl-multiboot2"
)))]
pub use api::*;

#[derive(Debug, Clone, Copy)]
pub struct MemoryMapEntry {
    pub base: PhysAddr,
    pub length: usize,
    pub kind: MemoryType,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameBufferInfo {
    pub address: VirtAddr,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u32,
}

pub struct BootLoaderInfo {
    pub dtb_addr: Option<(VirtAddr, usize)>,
    pub rsdp_addr: Option<VirtAddr>,
    pub hhdm_offset: usize,
    pub memory_map: &'static [MemoryMapEntry],
    pub framebuffer: Option<FrameBufferInfo>,
    pub kernel_address: (PhysAddr, VirtAddr),
    pub kernel_size: usize,
    pub initrd_addr: Option<(VirtAddr, usize)>,
    pub cmdline: Option<&'static str>,
    pub cpu_count: usize,
}

pub static BOOT_LOADER_INFO: Once<BootLoaderInfo> = Once::new();

#[cfg(feature = "embed-initrd")]
#[repr(C, align(4096))]
struct EmbeddedInitrd {
    data: [u8; include_bytes!(env!("INITRD_PATH")).len()],
}

#[cfg(feature = "embed-initrd")]
static EMBEDDED_INITRD: EmbeddedInitrd = EmbeddedInitrd {
    data: *include_bytes!(env!("INITRD_PATH")),
};

pub fn get_dtb() -> Option<(VirtAddr, usize)> {
    BOOT_LOADER_INFO.get().and_then(|info| info.dtb_addr)
}

pub fn get_initrd() -> Option<(VirtAddr, usize)> {
    if let Some(addr) = BOOT_LOADER_INFO.get().and_then(|info| info.initrd_addr) {
        return Some(addr);
    }

    #[cfg(feature = "embed-initrd")]
    {
        return Some((
            VirtAddr::from(EMBEDDED_INITRD.data.as_ptr() as usize),
            EMBEDDED_INITRD.data.len(),
        ));
    }

    #[cfg(not(feature = "embed-initrd"))]
    None
}

pub fn get_rsdp() -> Option<VirtAddr> {
    BOOT_LOADER_INFO.get().and_then(|info| info.rsdp_addr)
}

pub fn get_hhdm() -> usize {
    BOOT_LOADER_INFO.get().map(|info| info.hhdm_offset).unwrap_or(0)
}

pub fn get_mem_map() -> &'static [MemoryMapEntry] {
    BOOT_LOADER_INFO.get().map(|info| info.memory_map).unwrap_or(&[])
}

pub fn get_framebuffer() -> Option<FrameBufferInfo> {
    BOOT_LOADER_INFO.get().and_then(|info| info.framebuffer)
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
