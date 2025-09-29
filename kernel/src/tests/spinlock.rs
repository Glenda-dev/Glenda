use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::dtb;
use crate::lock::SpinLock;
use crate::printk;
use crate::printk::{ANSI_BLUE, ANSI_GREEN, ANSI_RED, ANSI_RESET, ANSI_YELLOW};

const INCREMENTS_PER_HART: usize = 16;

// 被测试的自旋锁
static TEST_LOCK: SpinLock = SpinLock::new();
// 计数
static GLOBAL_COUNTER: AtomicUsize = AtomicUsize::new(0);
// 参与 hart 数量，此变量用来确保所有 hart 同步启动
static PARTICIPANTS: AtomicUsize = AtomicUsize::new(0);
static START_TEST: AtomicBool = AtomicBool::new(false);
static HARTS_FINISHED: AtomicUsize = AtomicUsize::new(0);

// 自旋锁一致性测试
fn spinlock_test(hartid: usize, harts_under_test: usize) -> usize {
    if hartid >= harts_under_test {
        return 0;
    }

    // 确保所有 hart 统一开始
    let ready = PARTICIPANTS.fetch_add(1, Ordering::SeqCst) + 1;
    if ready == harts_under_test {
        printk!(
            "{}All {} harts ready. Starting spinlock test{}",
            ANSI_BLUE,
            harts_under_test,
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

    return HARTS_FINISHED.fetch_add(1, Ordering::SeqCst) + 1;
}

pub fn run(hartid: usize) {
    let harts_under_test = dtb::hart_count();
    // 运行测试
    let result = spinlock_test(hartid, harts_under_test);

    // 校验 + 打印结果
    if result == 0 {
        printk!("{}hart {} idle{} (not part of spinlock test)", ANSI_YELLOW, hartid, ANSI_RESET);
        return;
    }
    if result == harts_under_test {
        let expected = harts_under_test * INCREMENTS_PER_HART;
        let final_value = GLOBAL_COUNTER.load(Ordering::SeqCst);
        if final_value == expected {
            printk!(
                "{}[PASS]{} Spinlock test: counter reached {}",
                ANSI_GREEN,
                ANSI_RESET,
                final_value
            );
        } else {
            printk!(
                "{}[FAIL]{} Spinlock test: counter {} (expected {})",
                ANSI_RED,
                ANSI_RESET,
                final_value,
                expected
            );
        }
    }
}
