use super::cpu;
use super::dtb;
use super::sbi;
use crate::platform::PlatformInfo;
use crate::printk;
use crate::printk::{ANSI_RED, ANSI_RESET};
use core::sync::atomic::{AtomicBool, Ordering};

/*
 由主 hart 通过 HSM 启动次级 hart 的入口

 Also see:
 Glenda/kernel/src/boot.rs
*/
unsafe extern "C" {
    fn secondary_start(hartid: usize, dtb: *const u8) -> !;
}

static BOOTSTRAP_DONE: AtomicBool = AtomicBool::new(false);

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PlatformHandle(usize);

impl PlatformHandle {
    pub fn from(addr: usize) -> Self {
        PlatformHandle(addr as usize)
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.0 as *const u8
    }
    pub fn bits(&self) -> usize {
        self.0
    }
}

pub fn info() -> PlatformInfo {
    dtb::get_platform_info()
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
/// 发送核间中断 (IPI)
///
/// `mask`: 目标 CPU 的掩码 (通常是 bit mask 或类似于 sbi 的 hart_mask)
pub fn send_ipi(mask: usize, mask_base: usize) {
    sbi::send_ipi(mask, mask_base).expect("Failed to send IPI");
}

pub fn bootstrap_cpus() {
    if BOOTSTRAP_DONE.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return;
    }
    let start_addr = secondary_start as usize;
    let opaque = dtb::dtb_addr();
    let harts = dtb::hart_count();
    let cpuid = cpu::cpu_id();
    for target in 0..harts {
        if target == cpuid {
            continue;
        }
        match sbi::send_hsm(target, 0, start_addr, opaque).map(|_| ()) {
            Ok(()) => printk!("cpus: Started CPU {} via SBI\n", target),
            Err(err) => printk!(
                "{}cpus: Failed to start CPU {} via SBI: error {}{}\n",
                ANSI_RED,
                target,
                err,
                ANSI_RESET
            ),
        }
    }
}
