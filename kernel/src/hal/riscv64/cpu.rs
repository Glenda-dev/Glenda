use super::sbi;
use core::arch::asm;

pub const MAX_CPUS: usize = 8;

pub fn cpu_id() -> usize {
    let mut id: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) id);
    }
    id
}
pub fn read_cycle() -> usize {
    let cycle: usize;
    unsafe {
        asm!("rdcycle {}", out(reg) cycle);
    }
    cycle
}
pub fn timer_set_next(next: usize) {
    sbi::set_timer(next as u64).expect("Failed to set timer via SBI");
}
pub fn read_time() -> usize {
    let time: usize;
    unsafe {
        asm!("rdtime {}", out(reg) time);
    }
    time
}
