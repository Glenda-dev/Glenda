use super::cpu;
use crate::boot;
use crate::hal::riscv64::sbi;
use crate::platform::acpi::GlendaAcpiHandler;
use crate::printk::{ANSI_RED, ANSI_RESET};
use ::acpi::AcpiTables;
use core::str;
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
static VIRT_ENABLED: AtomicBool = AtomicBool::new(false);

fn detect_virt_from_dtb(fdt: &fdt::Fdt) -> bool {
    for node in fdt.all_nodes() {
        let Some(device_type_prop) = node.property("device_type") else {
            continue;
        };
        let Ok(device_type) = str::from_utf8(device_type_prop.value) else {
            continue;
        };
        if device_type.trim_end_matches('\0') != "cpu" {
            continue;
        }

        let Some(isa_prop) = node.property("riscv,isa") else {
            continue;
        };
        let Ok(isa) = str::from_utf8(isa_prop.value) else {
            continue;
        };
        if isa.trim_end_matches('\0').contains('h') {
            return true;
        }
    }

    false
}

fn detect_virt_from_boot_dtb() -> bool {
    if let Some((dtb, _)) = boot::get_dtb() {
        if let Ok(fdt) = unsafe { fdt::Fdt::from_ptr(dtb.as_usize() as *const u8) } {
            return detect_virt_from_dtb(&fdt);
        }
    }

    false
}

pub fn parse_dtb(fdt: &fdt::Fdt) {
    VIRT_ENABLED.store(detect_virt_from_dtb(fdt), Ordering::Release);
    dtb::parse(fdt)
}

pub fn parse_acpi(tables: &AcpiTables<GlendaAcpiHandler>) {
    // 当前实现仅在 DTB 中探测 RISC-V H 扩展。
    VIRT_ENABLED.store(false, Ordering::Release);
    acpi::parse(tables)
}

pub fn is_virtualization_enabled() -> bool {
    if VIRT_ENABLED.load(Ordering::Acquire) {
        return true;
    }

    let enabled = detect_virt_from_boot_dtb();
    VIRT_ENABLED.store(enabled, Ordering::Release);
    enabled
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
    let start_addr = secondary_start as *const () as usize;
    let opaque = crate::boot::get_dtb().map(|(pa, _)| pa.as_usize()).unwrap_or(0);
    let harts = crate::boot::get_cpu_count();
    let cpuid = cpu::cpu_id();
    log!("opensbi: Bootstrapping secondary CPUs from CPU {}...", cpuid);
    for target in 0..harts {
        if target == cpuid {
            continue;
        }
        match sbi::send_hsm(target, 0, start_addr, opaque).map(|_| ()) {
            Ok(()) => log!("opensbi: Started CPU {} via SBI", target),
            Err(err) => log!(
                "{}sbi: Failed to start CPU {} via SBI: error {}{}\n",
                ANSI_RED,
                target,
                err,
                ANSI_RESET
            ),
        }
    }
}
