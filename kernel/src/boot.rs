use crate::cap::CapPtr;
use crate::mem::PhysAddr;

/// Magic number to verify BootInfo validity: 'GLENDA_B'
pub const BOOTINFO_MAGIC: u32 = 0x99999999;

/// Fixed size of the BootInfo page (usually 4KB)
pub const BOOTINFO_SIZE: usize = 4096;

/// Maximum number of untyped memory regions we can describe
pub const MAX_UNTYPED_REGIONS: usize = 128;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BootInfo {
    /// Magic number for verification
    pub magic: u32,

    /// Physical address of the Device Tree Blob
    pub dtb_paddr: usize,

    /// Size of the Device Tree Blob
    pub dtb_size: usize,

    /// Range of empty slots in the Root Task's CSpace
    /// The Root Task can use these slots for minting/copying
    pub empty: SlotRegion,

    /// Range of slots containing Untyped Capabilities
    /// These correspond to the regions in `untyped_list`
    pub untyped: SlotRegion,

    /// Range of slots containing IRQ Handler Capabilities
    pub irq: SlotRegion,

    /// Number of valid entries in `untyped_list`
    pub untyped_count: usize,

    /// List of untyped memory regions available to the system
    /// The i-th entry here corresponds to the capability at `untyped.start + i`
    pub untyped_list: [UntypedDesc; MAX_UNTYPED_REGIONS],
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
    pub size_bits: u8,

    /// Whether this is device memory (MMIO) or RAM
    pub is_device: bool,

    pub padding: [u8; 6],
}

impl BootInfo {
    pub fn new() -> Self {
        Self {
            magic: BOOTINFO_MAGIC,
            dtb_paddr: 0,
            dtb_size: 0,
            irq: SlotRegion { start: 0, end: 0 },
            empty: SlotRegion { start: 0, end: 0 },
            untyped: SlotRegion { start: 0, end: 0 },
            untyped_count: 0,
            untyped_list: [UntypedDesc {
                paddr: PhysAddr::null(),
                size_bits: 0,
                is_device: false,
                padding: [0; 6],
            }; MAX_UNTYPED_REGIONS],
        }
    }
}
