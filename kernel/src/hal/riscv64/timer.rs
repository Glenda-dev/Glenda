use super::asm;
use crate::hal::riscv64::sbi;
use crate::sync::once::Once;

static TIMER_FREQ: Once<usize> = Once::new();
static DEFAULT_FREQ: usize = 10_000_000; // 默认频率为 10MHz

/// 初始化时钟硬件
pub fn init(freq: usize) {
    TIMER_FREQ.call_once(|| freq);
}

/// 获取当前时间 (ticks)
pub fn get_time() -> usize {
    asm::rdtime() as usize
}

/// 获取当前单调时钟的频率
pub fn get_freq() -> usize {
    *TIMER_FREQ.get().unwrap_or(&DEFAULT_FREQ)
}

/// 设置下一次产生中断的绝对时刻 (ticks)
pub fn set_next_event(ticks: usize) {
    sbi::set_timer(ticks as u64).expect("Failed to set timer via SBI");
}
