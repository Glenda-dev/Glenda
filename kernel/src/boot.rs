/// Magic number to verify BootInfo validity: 'GLENDA_B'
pub const BOOTINFO_MAGIC: u32 = 0x99999999;

/// Maximum number of untyped memory regions we can describe
pub const MAX_UNTYPED_REGIONS: usize = 64;

use crate::cap::CapPtr;
use crate::mem::PhysAddr;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BootInfo {
    /// Magic number for verification
    pub magic: u32,

    /// Physical address of the Device Tree Blob
    pub info_paddr: usize,

    /// Size of the Device Tree Blob
    pub info_size: usize,

    /// Range of empty slots in the Root Task's CSpace
    /// The Root Task can use these slots for minting/copying
    pub empty: SlotRegion,

    /// Range of slots containing Untyped Capabilities
    /// These correspond to the regions in `untyped_list`
    pub untyped: SlotRegion,

    /// Range of MMIO Untypes
    pub mmio: SlotRegion,

    /// Range of slots containing IRQ Handler Capabilities
    pub irq: SlotRegion,

    /// Number of valid entries in `untyped_list`
    pub untyped_count: usize,

    /// List of untyped memory regions available to the system
    /// The i-th entry here corresponds to the capability at `untyped.start + i`
    pub untyped_list: [UntypedDesc; MAX_UNTYPED_REGIONS],

    /// Number of valid entries in `untyped_list`
    pub mmio_count: usize,

    /// List of untyped memory regions available to the system
    /// The i-th entry here corresponds to the capability at `untyped.start + i`
    pub mmio_list: [UntypedDesc; MAX_UNTYPED_REGIONS],
    /// Command line arguments passed to the kernel
    pub cmdline: [u8; 128],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SlotRegion {
    pub start: CapPtr,
    pub end: CapPtr,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UntypedDesc {
    /// Physical address of the memory region
    pub paddr: PhysAddr,

    /// Size of the region in bits (2^size_bits bytes)
    pub size: usize,
}

impl BootInfo {
    pub fn new() -> Self {
        Self {
            magic: BOOTINFO_MAGIC,
            info_paddr: 0,
            info_size: 0,
            irq: SlotRegion { start: 0, end: 0 },
            empty: SlotRegion { start: 0, end: 0 },
            untyped: SlotRegion { start: 0, end: 0 },
            mmio: SlotRegion { start: 0, end: 0 },
            untyped_count: 0,
            untyped_list: [UntypedDesc { paddr: PhysAddr::null(), size: 0 }; MAX_UNTYPED_REGIONS],
            cmdline: [0; 128],
            mmio_count: 0,
            mmio_list: [UntypedDesc { paddr: PhysAddr::null(), size: 0 }; MAX_UNTYPED_REGIONS],
        }
    }
}
