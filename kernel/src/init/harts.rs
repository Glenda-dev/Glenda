use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::printk;
use crate::printk::{ANSI_BLUE, ANSI_RED, ANSI_RESET};

/*
 Hart State Management[1]

 [1]: https://www.scs.stanford.edu/~zyedidia/docs/riscv/riscv-sbi.pdf, Chapter Nine
*/
const SBI_EXT_HSM: usize = 0x48534d;
const SBI_FUNC_HART_START: usize = 0;

static BOOTSTRAP_DONE: AtomicBool = AtomicBool::new(false);
/*
 由主 hart 通过 HSM 启动次级 hart 的入口

 Also see:
 Glenda/kernel/src/boot.S
*/
unsafe extern "C" {
    fn secondary_start(hartid: usize, dtb: *const u8) -> !;
}

#[inline(always)]
unsafe fn sbi_hart_start(hartid: usize, start_addr: usize, opaque: usize) -> Result<(), isize> {
    let mut err: isize;
    unsafe {
        asm!(
            "ecall",
            in("a0") hartid,
            in("a1") start_addr,
            in("a2") opaque,
            in("a6") SBI_FUNC_HART_START,
            in("a7") SBI_EXT_HSM,
            lateout("a0") err,
            lateout("a1") _,
        options(nostack)
        );
    }
    if err == 0 {
        Ok(())
    } else {
        Err(err)
    }
}

// 由第一个进来的 hart 调用一次，启动其余参与测试的次级 hart
pub fn bootstrap_secondary_harts(hartid: usize, dtb: *const u8) {
    if BOOTSTRAP_DONE.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return;
    }
    unsafe {
        let start_addr = secondary_start as usize;
        let opaque = dtb as usize;
        let harts = 4; // TODO: 通过设备树获取系统中 hart 数量
        for target in 0..harts {
            if target == hartid {
                continue;
            }
            match sbi_hart_start(target, start_addr, opaque) {
                Ok(()) => printk!("{}Started hart {} via SBI{}", ANSI_BLUE, target, ANSI_RESET),
                Err(err) => printk!(
                    "{}Failed to start hart {} via SBI: error {}{}",
                    ANSI_RED,
                    target,
                    err,
                    ANSI_RESET
                ),
            }
        }
    }
}
