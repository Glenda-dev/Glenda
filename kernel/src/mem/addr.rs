#![allow(dead_code)]

// TODO: change to struct
pub type PhysAddr = usize;
pub type VirtAddr = usize;
pub type PPN = usize;
pub type VPN = usize;

use super::{PGMASK, PGSIZE};

#[inline(always)]
pub const fn align_up(value: usize) -> usize {
    debug_assert!(PGSIZE.is_power_of_two());
    (value + PGMASK) & !PGMASK
}

#[inline(always)]
pub const fn align_down(value: usize) -> usize {
    debug_assert!(PGSIZE.is_power_of_two());
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
