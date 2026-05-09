use crate::sync::once::Once;

static TIMER_FREQ: Once<usize> = Once::new();
static DEFAULT_FREQ: usize = 1_000_000_000;

pub fn init(freq: usize) {
    TIMER_FREQ.call_once(|| freq);
}

pub fn init_cpu() {}

pub fn get_time() -> usize {
    super::cpu::read_cycle()
}

pub fn get_freq() -> usize {
    *TIMER_FREQ.get().unwrap_or(&DEFAULT_FREQ)
}

pub fn set_next_event(_ticks: usize) {}
