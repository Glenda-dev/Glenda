use crate::hal;

pub const TIME_SLICE_MS: usize = 100;

/// 将毫秒转换为时钟周期
pub fn msec_to_cycles(msec: usize) -> usize {
    msec * (hal::timer::get_frequency() / 1000)
}

/// 将微秒转换为时钟周期
pub fn usec_to_cycles(usec: usize) -> usize {
    usec * (hal::timer::get_frequency() / 1_000_000)
}
