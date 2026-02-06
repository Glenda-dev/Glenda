use super::asm;
use super::sbi;
use crate::timer::TIME_SLICE_MS;
use crate::timer::msec_to_cycles;

/// 初始化时钟硬件
pub fn init() {
    set_next_event(msec_to_cycles(TIME_SLICE_MS));
}

/// 获取当前单调时钟的计数
pub fn get_time() -> usize {
    unsafe { asm::rdtime() };
}

/// 设置下一次产生中断的绝对时刻 (Cycles)
pub fn set_next_event(cycles: usize) {
    sbi::set_timer(cycles as u64).expect("Failed to set timer via SBI");
}

/// 获取时钟源频率 (Hz)
/// 用于将毫秒/纳秒转换为周期
pub fn get_frequency() -> usize {
    dtb::timebase_frequency()
}
