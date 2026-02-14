pub const MAX_CPUS: usize = 0;

/// 获取当前 CPU 核心 ID
pub fn cpu_id() -> usize {
    unimplemented!()
}
/// 读取Cycle
pub fn read_cycle() -> usize {
    unimplemented!()
}

/// 设置当前 CPU 核心 ID（仅在启动其他核心时使用）
pub fn set_cpuid(cpuid: usize) {
    unimplemented!()
}
