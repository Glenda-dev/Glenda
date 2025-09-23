use core::arch::asm;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::lock::SpinLock;
use crate::printk;
use crate::printk::{ANSI_BLUE, ANSI_GREEN, ANSI_RED, ANSI_RESET, ANSI_YELLOW};

const HARTS_UNDER_TEST: usize = 4;
const INCREMENTS_PER_HART: usize = 16;

/*
 Hart State Management[1]

 [1]: https://www.scs.stanford.edu/~zyedidia/docs/riscv/riscv-sbi.pdf, Chapter Nine
*/
const SBI_EXT_HSM: usize = 0x48534d;
const SBI_FUNC_HART_START: usize = 0;

// 被测试的自旋锁
static TEST_LOCK: SpinLock = SpinLock::new();
// 计数
static GLOBAL_COUNTER: AtomicUsize = AtomicUsize::new(0);
// 参与 hart 数量，此变量用来确保所有 hart 同步启动
static PARTICIPANTS: AtomicUsize = AtomicUsize::new(0);
static START_TEST: AtomicBool = AtomicBool::new(false);
static HARTS_FINISHED: AtomicUsize = AtomicUsize::new(0);
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
    if err == 0 { Ok(()) } else { Err(err) }
}

// 由第一个进来的 hart 调用一次，启动其余参与测试的次级 hart
fn bootstrap_secondary_harts(hartid: usize, dtb: *const u8) {
    if BOOTSTRAP_DONE.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return;
    }
    unsafe {
        let start_addr = secondary_start as usize;
        let opaque = dtb as usize;
        for target in 0..HARTS_UNDER_TEST {
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

// 自旋锁一致性测试
fn spinlock_test(hartid: usize) {
    if hartid >= HARTS_UNDER_TEST {
        printk!("{}hart {} idle{} (not part of spinlock test)", ANSI_YELLOW, hartid, ANSI_RESET);
        return;
    }

    // 确保所有 hart 统一开始
    let ready = PARTICIPANTS.fetch_add(1, Ordering::SeqCst) + 1;
    if ready == HARTS_UNDER_TEST {
        printk!(
            "{}All {} harts ready. Starting spinlock test{}",
            ANSI_BLUE,
            HARTS_UNDER_TEST,
            ANSI_RESET
        );
        START_TEST.store(true, Ordering::SeqCst);
    } else {
        while !START_TEST.load(Ordering::SeqCst) {
            spin_loop();
        }
    }

    // 拿锁 && 解锁
    for iter in 0..INCREMENTS_PER_HART {
        TEST_LOCK.lock();
        let value_before = GLOBAL_COUNTER.load(Ordering::Relaxed);
        driver_uart::print!("[hart {}] iter {} -> counter {}\n", hartid, iter, value_before + 1);
        GLOBAL_COUNTER.store(value_before + 1, Ordering::Relaxed);
        TEST_LOCK.unlock();
    }

    // 校验 + 打印结果
    let finished = HARTS_FINISHED.fetch_add(1, Ordering::SeqCst) + 1;
    if finished == HARTS_UNDER_TEST {
        let expected = HARTS_UNDER_TEST * INCREMENTS_PER_HART;
        let final_value = GLOBAL_COUNTER.load(Ordering::SeqCst);
        if final_value == expected {
            printk!(
                "{}Spinlock test passed{}: counter reached {}",
                ANSI_GREEN,
                ANSI_RESET,
                final_value
            );
        } else {
            printk!(
                "{}Spinlock test failed{}: counter {} (expected {})",
                ANSI_RED,
                ANSI_RESET,
                final_value,
                expected
            );
        }
    }
}

pub fn run(hartid: usize, dtb: *const u8) {
    printk!("HART ID: {}", hartid);
    bootstrap_secondary_harts(hartid, dtb);
    spinlock_test(hartid);
}
