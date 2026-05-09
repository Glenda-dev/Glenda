use core::arch::asm;

use crate::platform::acpi::GlendaAcpiHandler;
use acpi::AcpiTables;

pub fn parse_dtb(_fdt: &fdt::Fdt) {}

pub fn parse_acpi(_tables: &AcpiTables<GlendaAcpiHandler>) {}

pub fn shutdown() -> ! {
    loop {
        unsafe {
            asm!("cli; hlt", options(nomem, nostack));
        }
    }
}

pub fn reboot() -> ! {
    shutdown()
}

pub fn bootstrap_cpus() {}

pub fn is_virtualization_enabled() -> bool {
    false
}
