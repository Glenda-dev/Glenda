#![allow(dead_code)]

// TODO: change to struct
pub type PhysAddr = usize;
pub type VirtAddr = usize;
pub type PPN = usize;
pub type VPN = usize;

use super::{PGMASK, PGSIZE};

#[inline(always)]
pub const fn align_up(value: usize) -> usize {
    assert!(PGSIZE.is_power_of_two());
    (value + PGMASK) & !PGMASK
}

#[inline(always)]
pub const fn align_down(value: usize) -> usize {
    assert!(PGSIZE.is_power_of_two());
    value & !PGMASK
}

#[inline(always)]
pub const fn ppn(addr: PhysAddr) -> [PPN; 3] {
    [(addr >> 12) & 0x1FF, (addr >> 21) & 0x1FF, (addr >> 30) & 0x1FF]
}

#[inline(always)]
pub const fn page_offset(addr: VirtAddr) -> usize {
    addr & PGMASK
}

#[inline(always)]
pub const fn vpn(addr: VirtAddr) -> [VPN; 3] {
    [(addr >> 12) & 0x1FF, (addr >> 21) & 0x1FF, (addr >> 30) & 0x1FF]
}

/// 物理地址转虚拟地址
/// 不使用 HHDM 时，内核采用恒等映射，因此 PA == VA
pub fn phys_to_virt(pa: PhysAddr) -> VirtAddr {
    VirtAddr::from(pa)
}

/// 虚拟地址转物理地址
pub fn virt_to_phys(va: VirtAddr) -> PhysAddr {
    PhysAddr::from(va)
}
