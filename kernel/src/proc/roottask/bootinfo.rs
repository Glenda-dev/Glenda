/// Magic number to verify BootInfo validity: 'GLENDA_B'
pub const BOOTINFO_MAGIC: u32 = 0x99999999;

/// Maximum number of untyped memory regions we can describe
pub const MAX_UNTYPED_REGIONS: usize = 8;
pub const MAX_MMIO_REGIONS: usize = 64;

use crate::hal::mem::PGSIZE;
use crate::mem::{MemoryRange, UntypedRegion};

pub const BOOTINFO_SIZE: usize = core::mem::size_of::<BootInfo>();
pub const BOOTINFO_PAGES: usize = (BOOTINFO_SIZE + PGSIZE - 1) / PGSIZE;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BootInfo {
    /// Magic number for verification
    pub magic: u32,

    //// Initrd memory region
    pub initrd_start: usize,
    pub initrd_size: usize,

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
            magic: BOOTINFO_MAGIC,
            untyped_count: 0,
            untyped_list: [UntypedRegion::empty(); MAX_UNTYPED_REGIONS],
            mmio_count: 0,
            mmio_list: [MemoryRange::empty(); MAX_MMIO_REGIONS],
            initrd_start: 0,
            initrd_size: 0,
            irq_count: 0,
        }
    }
}
