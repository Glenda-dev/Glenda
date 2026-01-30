/// Magic number to verify BootInfo validity: 'GLENDA_B'
pub const BOOTINFO_MAGIC: u32 = 0x99999999;

/// Maximum number of untyped memory regions we can describe
pub const MAX_UNTYPED_REGIONS: usize = 64;

use crate::mem::{MemoryRange, UntypedRegion};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BootInfo {
    /// Magic number for verification
    pub magic: u32,

    /// Platform Info Desc
    pub info_desc: MemoryRange,

    /// Number of valid entries in `untyped_list`
    pub untyped_count: usize,

    /// List of untyped memory regions available to the system
    /// The i-th entry here corresponds to the capability at `untyped.start + i`
    pub untyped_list: [UntypedRegion; MAX_UNTYPED_REGIONS],

    /// Number of valid entries in `untyped_list`
    pub mmio_count: usize,

    /// List of untyped memory regions available to the system
    /// The i-th entry here corresponds to the capability at `untyped.start + i`
    pub mmio_list: [MemoryRange; MAX_UNTYPED_REGIONS],

    /// Command line arguments passed to the kernel
    pub cmdline: [u8; 128],

    /// IRQ Handler count
    pub irq_count: usize,
}

impl BootInfo {
    pub fn new() -> Self {
        Self {
            magic: BOOTINFO_MAGIC,
            info_desc: MemoryRange::empty(),
            untyped_count: 0,
            untyped_list: [UntypedRegion::empty(); MAX_UNTYPED_REGIONS],
            cmdline: [0; 128],
            mmio_count: 0,
            mmio_list: [MemoryRange::empty(); MAX_UNTYPED_REGIONS],
            irq_count: 0,
        }
    }
}
