use crate::platform::MemoryRange;

/// 平台信息结构体
#[derive(Clone, Copy, Debug)]
pub struct PlatformInfo;

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
pub fn send_ipi(mask: usize, mask_base: usize) {
    unimplemented!()
}

/// 获取内存范围
pub fn memory_range() -> Option<MemoryRange> {
    unimplemented!()
}

/// 平台初始化
pub fn init(dtb: PlatformInfo) {
    unimplemented!()
}

/// 获取 initrd 内存范围
pub fn initrd() -> Option<MemoryRange> {
    unimplemented!()
}

/// 获取内存范围
pub fn range() -> Option<MemoryRange> {
    unimplemented!()
}

/// 获取设备内存
pub fn mmio_ranges() -> &'static [MemoryRange] {
    unimplemented!()
}

/// 引导其他 CPU
pub fn bootstrap_cpus(cpuid: usize, info: PlatformInfo) -> ! {
    unimplemented!()
}
