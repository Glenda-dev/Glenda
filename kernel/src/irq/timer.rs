use crate::hal;
use crate::printk;
use core::sync::atomic::{AtomicUsize, Ordering};

const INTERVAL: usize = 1000000; // 100ms

static SYS_TICKS: AtomicUsize = AtomicUsize::new(0);

pub fn init(hartid: usize) {
    program_next_tick();
    printk!("timer: init hart {}", hartid);
}

pub fn create() {
    SYS_TICKS.store(0, Ordering::Relaxed);
}

pub fn update() {
    SYS_TICKS.fetch_add(1, Ordering::Relaxed);
    program_next_tick();
}

pub fn get_ticks() -> usize {
    SYS_TICKS.load(Ordering::Relaxed)
}

pub fn start(_hartid: usize) {
    hal::cpu::timer_set_next(hal::cpu::read_time() + INTERVAL);
}

pub fn program_next_tick() {
    let next = hal::cpu::read_time() + INTERVAL;
    hal::cpu::timer_set_next(next);
}
