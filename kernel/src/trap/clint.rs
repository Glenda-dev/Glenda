use core::ptr::{read_volatile, write_volatile};

const CLINT_BASE: usize = 0x0200_0000;

pub fn set_msip(hartid: usize) {
    unsafe {
        let addr = CLINT_BASE + hartid * 0x4;
        write_volatile(addr as *mut u32, 1);
    }
}
pub fn get_msip(hartid: usize) -> usize {
    unsafe {
        let addr = CLINT_BASE + hartid * 0x4;
        read_volatile(addr as *const u32) as usize
    }
}

pub fn set_mtime() -> usize {
    unsafe {
        let addr = CLINT_BASE + 0xBFF8;
        write_volatile(addr as *mut u64, 0);
    }
    0
}
pub fn get_mtime() -> usize {
    unsafe {
        let addr = CLINT_BASE + 0xBFF8;
        read_volatile(addr as *const u64) as usize
    }
}
pub fn set_mtimecmp(hartid: usize, time: usize) {
    unsafe {
        let addr = CLINT_BASE + 0x4000 + hartid * 0x8;
        write_volatile(addr as *mut u64, time as u64);
    }
}
pub fn get_mtimecmp(hartid: usize) -> usize {
    unsafe {
        let addr = CLINT_BASE + 0x4000 + hartid * 0x8;
        read_volatile(addr as *const u64) as usize
    }
}
