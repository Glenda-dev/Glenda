use crate::mem::{PhysAddr, VirtAddr};

pub const fn phys_to_virt(pa: PhysAddr) -> VirtAddr {
    VirtAddr::from(pa.as_usize())
}

pub const fn virt_to_phys(va: VirtAddr) -> PhysAddr {
    PhysAddr::from(va.as_usize())
}
