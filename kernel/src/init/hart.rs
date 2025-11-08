use crate::hart;
use crate::printk;
use crate::printk::{ANSI_RED, ANSI_RESET};
use core::sync::atomic::{AtomicBool, Ordering};

static BOOTSTRAP_DONE: AtomicBool = AtomicBool::new(false);
/*
 由主 hart 通过 HSM 启动次级 hart 的入口

 Also see:
 Glenda/kernel/src/boot.S
*/
unsafe extern "C" {
    fn secondary_start(hartid: usize, dtb: *const u8) -> !;
}

unsafe extern "C" {
    fn sbi_hart_start_asm(hartid: usize, start_addr: usize, opaque: usize) -> isize;
}

#[inline(always)]
unsafe fn sbi_hart_start(hartid: usize, start_addr: usize, opaque: usize) -> Result<(), isize> {
    let err = unsafe { sbi_hart_start_asm(hartid, start_addr, opaque) };
    if err == 0 { Ok(()) } else { Err(err) }
}

// 由第一个进来的 hart 调用一次，启动其余参与测试的次级 hart
pub fn bootstrap_secondary_harts(hartid: usize, dtb: *const u8) {
    if BOOTSTRAP_DONE.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return;
    }
    unsafe {
        let start_addr = secondary_start as usize;
        let opaque = dtb as usize;
        let harts = crate::dtb::hart_count();
        for target in 0..harts {
            if target == hartid {
                continue;
            }
            match sbi_hart_start(target, start_addr, opaque) {
                Ok(()) => printk!("HARTS: Started hart {} via SBI", target),
                Err(err) => printk!(
                    "{}HARTS: Failed to start hart {} via SBI: error {}{}",
                    ANSI_RED,
                    target,
                    err,
                    ANSI_RESET
                ),
            }
        }
    }
}

pub fn init(hartid: usize, dtb: *const u8) {
    hart::enable_hart(hartid);
    bootstrap_secondary_harts(hartid, dtb);
}
