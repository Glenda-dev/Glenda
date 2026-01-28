use super::dtb;
use super::sbi;
use crate::platform::MemoryRange;
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
pub struct PlatformInfo(usize);

impl PlatformInfo {
    pub fn from(raw: usize) -> Self {
        PlatformInfo(raw)
    }
}

/// 获取引导参数
pub fn bootargs() -> Option<&'static str> {
    dtb::bootargs()
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

/// 获取内存范围
pub fn memory_range() -> Option<MemoryRange> {
    dtb::memory_range()
}

/// 平台初始化
pub fn init(info: PlatformInfo) {
    let dtb = info.0 as *const u8;
    dtb::init(dtb);
}

/// 获取initrd范围
pub fn initrd() -> Option<MemoryRange> {
    dtb::initrd_range()
}

/// 获取DTB范围
pub fn range() -> Option<MemoryRange> {
    Some(dtb::dtb_range())
}

/// 获取设备内存
pub fn mmio_ranges() -> &'static [MemoryRange] {
    dtb::mmio_ranges()
}

pub fn bootstrap_cpus(cpuid: usize, info: PlatformInfo) {
    if BOOTSTRAP_DONE.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return;
    }
    let start_addr = secondary_start as usize;
    let opaque = info.0;
    let harts = dtb::hart_count();
    for target in 0..harts {
        if target == cpuid {
            continue;
        }
        match sbi::send_hsm(target, 0, start_addr, opaque).map(|_| ()) {
            Ok(()) => printk!("cpus: Started cpu {} via SBI\n", target),
            Err(err) => printk!(
                "{}cpus: Failed to start cpu {} via SBI: error {}{}\n",
                ANSI_RED,
                target,
                err,
                ANSI_RESET
            ),
        }
    }
}
