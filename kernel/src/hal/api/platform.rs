use crate::mem::MemoryRange;
use crate::platform::PlatformInfo;
use crate::platform::acpi::GlendaAcpiHandler;
use acpi::AcpiTables;

/// 获取平台信息(DTB)
pub fn parse_dtb(fdt: &fdt::Fdt, info: &mut PlatformInfo) {
    unimplemented!()
}

/// 获取平台信息(ACPI)
pub fn parse_acpi(tables: &AcpiTables<GlendaAcpiHandler>, info: &mut PlatformInfo) {
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

/// 引导其他 CPU
pub fn bootstrap_cpus() {
    unimplemented!()
}
