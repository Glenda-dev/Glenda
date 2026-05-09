use super::psci;
use crate::hal::aarch64::asm;
use crate::hal::cpu;
use crate::printk::{ANSI_RED, ANSI_RESET};
use core::sync::atomic::{AtomicBool, Ordering};

mod acpi;
mod dtb;

pub use acpi::parse_acpi;
pub use dtb::parse_dtb;

pub fn init() {
    // Prime virtualization capability cache early.
    let enabled = is_virtualization_enabled();
    set_virtualization_enabled(enabled);
}
static VIRT_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn shutdown() -> ! {
    psci::system_off()
}

pub fn reboot() -> ! {
    psci::system_reset()
}

static BOOTSTRAP_DONE: AtomicBool = AtomicBool::new(false);
unsafe extern "C" {
    fn secondary_start(cpuid: usize) -> !;
}

pub fn bootstrap_cpus() {
    if BOOTSTRAP_DONE.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return;
    }
    let start_addr = secondary_start as *const () as usize;
    let cpus = crate::boot::get_cpu_count();
    let cpuid = cpu::cpu_id();
    log!("linuxboot: Bootstrapping secondary CPUs from CPU {}...", cpuid);
    for target in 0..cpus {
        if target == cpuid {
            continue;
        }
        // context_id is passed to x0, which we use as cpuid
        let res = psci::cpu_on(target, start_addr, target);
        if res == 0 {
            log!("linuxboot: Started CPU {} via PSCI", target);
        } else {
            log!(
                "{}linuxboot: Failed to start CPU {} via PSCI: error {}{}\n",
                ANSI_RED,
                target,
                res,
                ANSI_RESET
            );
        }
    }
}

pub fn is_virtualization_enabled() -> bool {
    if VIRT_ENABLED.load(Ordering::Acquire) {
        return true;
    }

    // ID_AA64PFR0_EL1[11:8]: EL2 support.
    let pfr0 = asm::read_id_aa64pfr0();
    let el2 = (pfr0 >> 8) & 0xF;
    let enabled = el2 != 0xF;
    VIRT_ENABLED.store(enabled, Ordering::Release);
    enabled
}

pub(super) fn set_virtualization_enabled(enabled: bool) {
    VIRT_ENABLED.store(enabled, Ordering::Release);
}
