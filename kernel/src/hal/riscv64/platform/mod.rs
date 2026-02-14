use super::cpu;
use crate::hal::riscv64::sbi;
use crate::platform::acpi::GlendaAcpiHandler;
use crate::printk::{ANSI_RED, ANSI_RESET};
use ::acpi::AcpiTables;
use core::sync::atomic::{AtomicBool, Ordering};

mod acpi;
mod dtb;

/*
 由主 hart 通过 HSM 启动次级 hart 的入口

 Also see:
 Glenda/kernel/src/boot.rs
*/
unsafe extern "C" {
    fn secondary_start(hartid: usize, dtb: *const u8) -> !;
}

static BOOTSTRAP_DONE: AtomicBool = AtomicBool::new(false);

pub fn parse_dtb(fdt: &fdt::Fdt) {
    dtb::parse(fdt)
}

pub fn parse_acpi(tables: &AcpiTables<GlendaAcpiHandler>) {
    acpi::parse(tables)
}

/// 关闭系统
pub fn shutdown() -> ! {
    let err = sbi::system_reset(0, 0);
    panic!("Failed to shutdown system via SBI: {:?}", err);
}

/// 重启系统
pub fn reboot() -> ! {
    let err = sbi::system_reset(1, 0);
    panic!("Failed to reboot system via SBI: {:?}", err);
}

pub fn bootstrap_cpus() {
    if BOOTSTRAP_DONE.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return;
    }

    let start_addr = secondary_start as usize;
    let opaque = crate::boot::get_dtb().map(|pa| pa.as_usize()).unwrap_or(0);
    let harts = crate::boot::get_cpu_count();
    let cpuid = cpu::cpu_id();
    for target in 0..harts {
        if target == cpuid {
            continue;
        }
        match sbi::send_hsm(target, 0, start_addr, opaque).map(|_| ()) {
            Ok(()) => log!("cpus: Started CPU {} via SBI", target),
            Err(err) => log!(
                "{}cpus: Failed to start CPU {} via SBI: error {}{}\n",
                ANSI_RED,
                target,
                err,
                ANSI_RESET
            ),
        }
    }
}
