use core::sync::atomic::{AtomicUsize, Ordering};
use riscv::register::time;

const INTERVAL: usize = 1000000; // 100ms

static SYS_TICKS: AtomicUsize = AtomicUsize::new(0);

pub fn init(hartid: usize) {
    // SBI is nice
    program_next_tick();
    crate::printk!("timer: init hart {}", hartid);
}

pub fn create() {
    SYS_TICKS.store(0, Ordering::Relaxed);
}
pub fn update() {
    SYS_TICKS.fetch_add(1, Ordering::Relaxed);
}

pub fn get_ticks() -> usize {
    SYS_TICKS.load(Ordering::Relaxed)
}

#[inline(always)]
fn time_now() -> u64 {
    time::read() as u64
}

pub fn program_next_tick() {
    let next = time_now().wrapping_add(INTERVAL as u64);
    // FIXME: 错误处理
    let _ = crate::sbi::set_timer(next);
}

pub fn start(hartid: usize) {
    if hartid == 0 {
        program_next_tick();
    }
}
