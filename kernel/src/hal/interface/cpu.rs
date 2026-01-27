pub const MAX_CPUS: usize = 0;

/// 获取当前 CPU 核心 ID
pub fn cpu_id() -> usize {
    unimplemented!()
}

/// 读取周期计数器 (Time/Cycle)
pub fn read_cycle() -> usize {
    unimplemented!()
}

/// 读取时间计数器 (Time/TimeStamp)
pub fn read_time() -> usize {
    unimplemented!()
}
/// 设置下一个定时器中断时间点
pub fn timer_set_next(next: usize) {
    unimplemented!()
}
