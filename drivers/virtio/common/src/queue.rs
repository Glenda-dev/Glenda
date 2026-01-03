use core::mem::size_of;

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct Descriptor {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

pub const DESC_F_NEXT: u16 = 1;
pub const DESC_F_WRITE: u16 = 2;
pub const DESC_F_INDIRECT: u16 = 4;

#[repr(C, align(2))]
pub struct Available {
    pub flags: u16,
    pub idx: u16,
    pub ring: [u16; 0], // Flexible array
}

#[repr(C, align(4))]
pub struct UsedElem {
    pub id: u32,
    pub len: u32,
}

#[repr(C, align(4))]
pub struct Used {
    pub flags: u16,
    pub idx: u16,
    pub ring: [UsedElem; 0], // Flexible array
}

// Helper to calculate queue size
pub fn queue_size_in_bytes(num: usize) -> usize {
    let desc_size = size_of::<Descriptor>() * num;
    let avail_size = size_of::<u16>() * (3 + num);
    let used_size = size_of::<u16>() * 3 + size_of::<UsedElem>() * num;
    // Alignment requirements...
    // For simplicity, just return a safe upper bound or let the driver handle it.
    // VirtIO 1.0:
    // Descriptor Table: 16 * Queue Size
    // Available Ring: 6 + 2 * Queue Size
    // Used Ring: 6 + 8 * Queue Size
    // Padding is needed.
    // Let's assume the driver allocates 3 separate regions or one contiguous region with alignment.
    desc_size + avail_size + used_size + 4096 // Padding
}
