use super::asm;
use core::sync::atomic::{AtomicUsize, Ordering};

static TIMER_FREQ: AtomicUsize = AtomicUsize::new(0);

pub fn init(freq: usize) {
    TIMER_FREQ.store(freq, Ordering::Relaxed);
}

pub fn get_time() -> usize {
    asm::read_cntvct()
}

pub fn get_freq() -> usize {
    TIMER_FREQ.load(Ordering::Relaxed)
}

pub fn set_next_event(ticks: usize) {
    // Program the virtual timer compare value with an absolute tick.
    unsafe {
        asm::write_cntv_cval(ticks as u64);
        asm::write_cntv_ctl(1);
    }
    asm::isb();
}
