// ==========================================
// 1. CPU 本地状态管理 (Local State)
// ==========================================

/// 全局开启中断
pub unsafe fn enable() {
    unimplemented!()
}

/// 全局关闭中断
pub unsafe fn disable() {
    unimplemented!()
}

/// 获取当前中断状态
pub fn is_enabled() -> bool {
    unimplemented!()
}

/// 等待中断 (WFI/HLT)
pub fn wfi() {
    unimplemented!()
}

// ==========================================
// 2. 平台级中断控制器 (Platform Controller: PLIC/GIC/IOAPIC)
// ==========================================

pub const MAX_IRQS: usize = 0;

/// 初始化中断控制器
pub fn init() {
    unimplemented!()
}

/// 初始化当前 CPU 的中断控制器相关设置
pub fn init_cpu() {
    unimplemented!()
}

/// 屏蔽（禁用）指定硬件中断号
///
/// 在微内核中，通常在接收到中断后内核会先 Mask，
/// 等待用户态驱动处理完毕通过 Syscall 告知内核后再 Unmask。
pub fn mask(irq: u32, cpuid: usize) {
    unimplemented!()
}

/// 解除屏蔽（启用）指定硬件中断号
pub fn unmask(irq: u32, cpuid: usize) {
    unimplemented!()
}

/// 获取并确认当前挂起的外部中断号
///
/// 对应 RISC-V PLIC 的 Claim，或 GIC 的 IAR。
/// 返回 `None` 表示它是伪中断或非外部来源。
pub fn claim(cpuid: usize) -> Option<u32> {
    unimplemented!()
}

/// 发送中断完成信号 (EOI)
///
/// 告知控制器该中断已处理（注意：这不同于 Unmask）。
/// 对应 RISC-V PLIC 的 Complete，或 GIC 的 EOI。
pub fn complete(irq: u32, cpuid: usize) {
    unimplemented!()
}

/// 设置中断亲和性 (可选)
/// 将中断路由到指定 CPU
pub fn set_affinity(irq: u32, cpuid: usize) {
    unimplemented!()
}

/// 设置中断优先级 (可选)
pub fn set_priority(irq: u32, priority: u8) {
    unimplemented!()
}

/// 清除软件中断
pub fn clear_soft() {
    unimplemented!()
}
