use super::asm;
use crate::hal::riscv64::sbi;

static mut TIMER_FREQ: usize = 10_000_000;

/// 初始化时钟硬件
pub fn init() {}

/// 设置频率
pub fn set_frequency(freq: usize) {
    unsafe {
        TIMER_FREQ = freq;
    }
}

/// 获取时钟频率 (Hz)
pub fn get_frequency() -> usize {
    unsafe { TIMER_FREQ }
}

/// 获取当前时间 (ms)
pub fn get_time() -> usize {
    unsafe { asm::rdtime() / (TIMER_FREQ / 1000) }
}

/// 设置下一次产生中断的绝对时刻 (ms)
pub fn set_next_event(ms: usize) {
    let ticks = unsafe { (ms as u64) * (TIMER_FREQ as u64) / 1000 };
    sbi::set_timer(ticks).expect("Failed to set timer via SBI");
}
