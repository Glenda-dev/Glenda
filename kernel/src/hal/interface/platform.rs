use crate::platform::MemoryRange;

/// 获取引导参数
pub fn bootargs() -> Option<&'static str> {
    unimplemented!()
}

/// 关闭系统
pub fn shutdown() -> ! {
    unimplemented!()
}

/// 重启系统
pub fn reboot() -> ! {
    unimplemented!()
}
/// 发送核间中断 (IPI)
///
/// `mask`: 目标 CPU 的掩码 (通常是 bit mask 或类似于 sbi 的 hart_mask)
pub fn send_ipi(mask: usize) {
    unimplemented!()
}

/// 获取内存范围
pub fn memory_range() -> Option<MemoryRange> {
    unimplemented!()
}
