/// Maximum number of untyped memory regions we can describe
pub const MAX_UNTYPED_REGIONS: usize = 4;
pub const MAX_MMIO_REGIONS: usize = 64;

use crate::hal::mem::PGSIZE;
use crate::mem::{MemoryRange, UntypedRegion};

pub const BOOTINFO_SIZE: usize = core::mem::size_of::<BootInfo>();
pub const BOOTINFO_PAGES: usize = (BOOTINFO_SIZE + PGSIZE - 1) / PGSIZE;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BootInfo {
    //// Initrd memory region
    pub initrd_start: usize,
    pub initrd_size: usize,

    pub version: u32,
    pub build: [u8; 64],
    pub git_hash: [u8; 8],

    /// Number of valid entries in `untyped_list`
    pub untyped_count: usize,

    /// List of untyped memory regions available to the system
    /// The i-th entry here corresponds to the capability at `untyped.start + i`
    pub untyped_list: [UntypedRegion; MAX_UNTYPED_REGIONS],

    /// Number of valid entries in `untyped_list`
    pub mmio_count: usize,

    /// List of untyped memory regions available to the system
    /// The i-th entry here corresponds to the capability at `untyped.start + i`
    pub mmio_list: [MemoryRange; MAX_MMIO_REGIONS],

    /// Number of IRQs available
    pub irq_count: usize,
}

impl BootInfo {
    pub fn new() -> Self {
        Self {
            untyped_count: 0,
            untyped_list: [UntypedRegion::empty(); MAX_UNTYPED_REGIONS],
            mmio_count: 0,
            mmio_list: [MemoryRange::empty(); MAX_MMIO_REGIONS],
            initrd_start: 0,
            initrd_size: 0,
            irq_count: 0,
            version: 0,
            build: [0; 64],
            git_hash: [0; 8],
        }
    }
}
