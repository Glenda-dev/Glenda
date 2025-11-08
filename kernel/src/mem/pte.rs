#![allow(dead_code)]

use super::PhysAddr;

pub const PTE_V: usize = 1 << 0; // Valid
pub const PTE_R: usize = 1 << 1; // Read
pub const PTE_W: usize = 1 << 2; // Write
pub const PTE_X: usize = 1 << 3; // Execute
pub const PTE_U: usize = 1 << 4; // User
pub const PTE_G: usize = 1 << 5; // Global
pub const PTE_A: usize = 1 << 6; // Accessed
pub const PTE_D: usize = 1 << 7; // Dirty

// TODO: change to struct
pub type Pte = usize;
pub type PteFlags = usize;

#[inline(always)]
pub const fn set_ppn(pte: Pte, ppn: usize) -> Pte {
    (pte & 0x3FF) | (ppn << 10)
}

#[inline(always)]
pub const fn get_ppn(pte: Pte) -> usize {
    (pte >> 10) & 0xFFFFFFFFFFF
}

#[inline(always)]
pub const fn set_flags(pte: Pte, flags: PteFlags) -> Pte {
    (pte & !0x3FF) | (flags & 0x3FF)
}

#[inline(always)]
pub const fn get_flags(pte: Pte) -> PteFlags {
    pte & 0x3FF
}

#[inline(always)]
pub const fn is_valid(pte: Pte) -> bool {
    (pte & PTE_V) != 0
}

#[inline(always)]
pub const fn is_leaf(pte: Pte) -> bool {
    (pte & (PTE_R | PTE_W | PTE_X)) != 0
}

// 中间页表条目：有效但不是 leaf
#[inline(always)]
pub const fn is_table(pte: Pte) -> bool {
    is_valid(pte) && !is_leaf(pte)
}

#[inline(always)]
pub const fn pte_to_pa(pte: Pte) -> PhysAddr {
    get_ppn(pte) << 12
}

#[inline(always)]
pub const fn pa_to_pte(pa: PhysAddr, flags: PteFlags) -> Pte {
    (((pa >> 12) & 0xFFFFFFFFFFF) << 10) | (flags & 0x3FF)
}
