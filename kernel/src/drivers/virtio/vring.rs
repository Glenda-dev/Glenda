#[repr(C)]
#[repr(align(16))]
pub struct VRingDesc {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

#[repr(C)]
#[repr(align(4))]
pub struct VRingUsedElem {
    id: u32,
    len: u32,
}

// Descriptor flags
pub const VRING_DESC_F_NEXT: u16 = 1;
pub const VRING_DESC_F_WRITE: u16 = 2;
