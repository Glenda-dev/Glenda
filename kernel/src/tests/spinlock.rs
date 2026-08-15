#![allow(dead_code)]

use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::barrier::MultiCoreTestBarrier;
use crate::dtb;
use crate::printk;
use crate::printk::{ANSI_BLUE, ANSI_GREEN, ANSI_RESET, ANSI_YELLOW};
use spin::Mutex;

const INCREMENTS_PER_HART: usize = 16;
static TEST_LOCK: Mutex<()> = Mutex::new(());
static GLOBAL_COUNTER: AtomicUsize = AtomicUsize::new(0);
static TEST_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();
static TEST_DONE: AtomicBool = AtomicBool::new(false);
static START_PRINTED: AtomicBool = AtomicBool::new(false);

pub fn run(hartid: usize) {
    // 任意先到达的 hart 负责初始化（无顺序假设）
    let total = dtb::hart_count();
    TEST_BARRIER.ensure_inited(total);
    if START_PRINTED.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        printk!("{}[TEST]{} Spinlock test start ({} harts)\n", ANSI_YELLOW, ANSI_RESET, total);
    }

    // 启动栅栏（所有核一致开始）
    TEST_BARRIER.wait_start();
    if START_PRINTED.load(Ordering::Acquire) && !TEST_DONE.load(Ordering::Acquire) {
        // 只有第一个到达 wait_start 阶段且尚未打印过 ready 的情况下打印（允许多核初次阶段交错）
        if hartid == 0 {
            printk!(
                "{}All {} harts ready. Starting spinlock test{}\n",
                ANSI_BLUE,
                TEST_BARRIER.total(),
                ANSI_RESET
            );
        }
    }

    // 临界区递增
    for iter in 0..INCREMENTS_PER_HART {
        let _guard = TEST_LOCK.lock();
        let value_before = GLOBAL_COUNTER.load(Ordering::Relaxed);
        printk!("[hart {}] iter {} -> counter {}\n", hartid, iter, value_before + 1);
        GLOBAL_COUNTER.store(value_before + 1, Ordering::Relaxed);
    }

    if TEST_BARRIER.finish_and_last() {
        let expected = TEST_BARRIER.total() * INCREMENTS_PER_HART;
        let final_value = GLOBAL_COUNTER.load(Ordering::SeqCst);
        assert_eq!(
            final_value, expected,
            "Spinlock test: counter mismatch (expected {}, got {})",
            expected, final_value
        );
        printk!(
            "{}[PASS]{} Spinlock test: counter reached {}\n",
            ANSI_GREEN,
            ANSI_RESET,
            final_value
        );
        TEST_DONE.store(true, Ordering::Release);
    } else {
        while !TEST_DONE.load(Ordering::Acquire) {
            spin_loop();
        }
    }
}
