use crate::platform::acpi::GlendaAcpiHandler;
use acpi::AcpiTables;

/// 获取平台信息(DTB)
pub fn parse_dtb(fdt: &fdt::Fdt) {
    unimplemented!()
}

/// 获取平台信息(ACPI)
pub fn parse_acpi(tables: &AcpiTables<GlendaAcpiHandler>) {
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

/// 引导其他 CPU
pub fn bootstrap_cpus() {
    unimplemented!()
}
