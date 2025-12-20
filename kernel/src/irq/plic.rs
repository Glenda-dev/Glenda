use crate::dtb;
use core::ptr::{read_volatile, write_volatile};

#[inline(always)]
fn plic_base() -> usize {
    dtb::plic_base().expect("PLIC base not found in DTB")
}

#[inline(always)]
fn ctx_index_s(hartid: usize) -> usize {
    // QEMU virt PLIC: per-hart contexts, 2 per hart: M=2*hart, S=2*hart+1
    hartid * 2 + 1
}

pub fn claim(hartid: usize) -> usize {
    get_claim_s(hartid)
}
pub fn complete(hartid: usize, id: usize) {
    set_claim_s(hartid, id);
}

pub fn set_priority(id: usize, priority: usize) {
    unsafe {
        let addr = plic_base() + id * 4;
        write_volatile(addr as *mut u32, priority as u32);
    }
}

pub fn get_priority(id: usize) -> usize {
    unsafe {
        let addr = plic_base() + id * 4;
        read_volatile(addr as *const u32) as usize
    }
}

pub fn set_enable_s(hartid: usize, id: usize, enable: bool) {
    unsafe {
        let context = ctx_index_s(hartid);
        let word_index = (id / 32) * 4; // only first word used for small IDs
        let addr = plic_base() + 0x2000 + context * 0x80 + word_index;
        let bit = 1u32 << (id % 32);
        let cur = read_volatile(addr as *const u32);
        let new = if enable { cur | bit } else { cur & !bit };
        write_volatile(addr as *mut u32, new);
    }
}
pub fn get_enable_s(hartid: usize, id: usize) -> bool {
    unsafe {
        let context = ctx_index_s(hartid);
        let word_index = (id / 32) * 4;
        let addr = plic_base() + 0x2000 + context * 0x80 + word_index;
        let bit = 1u32 << (id % 32);
        (read_volatile(addr as *const u32) & bit) != 0
    }
}

pub fn set_threshold_s(hartid: usize, threshold: usize) {
    unsafe {
        // S-mode context threshold register for hart
        let context = ctx_index_s(hartid);
        let addr = plic_base() + 0x200000 + context * 0x1000;
        write_volatile(addr as *mut u32, threshold as u32);
    }
}

pub fn get_threshold_s(hartid: usize) -> usize {
    unsafe {
        let context = ctx_index_s(hartid);
        let addr = plic_base() + 0x200000 + context * 0x1000;
        read_volatile(addr as *const u32) as usize
    }
}

pub fn set_claim_s(hartid: usize, id: usize) {
    unsafe {
        // S-mode claim/complete register for hart: write to complete
        let context = ctx_index_s(hartid);
        let addr = plic_base() + 0x200004 + context * 0x1000;
        write_volatile(addr as *mut u32, id as u32);
    }
}

pub fn get_claim_s(hartid: usize) -> usize {
    unsafe {
        // S-mode claim/complete register for hart: read to claim
        let context = ctx_index_s(hartid);
        let addr = plic_base() + 0x200004 + context * 0x1000;
        read_volatile(addr as *const u32) as usize
    }
}
