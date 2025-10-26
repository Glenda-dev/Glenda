use super::barrier::MultiCoreTestBarrier;
use crate::dtb;
use crate::printk;
use crate::printk::{ANSI_GREEN, ANSI_RESET, ANSI_YELLOW};
use crate::trap::timer;
use riscv::register::sie;

/// 运行时钟滴答测试和 UART 输出测试
pub fn run(hartid: usize) {
    timer_tick_test(hartid);
    uart_output_test(hartid);
}

fn timer_tick_test(hartid: usize) {
    static TIMER_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();
    TIMER_BARRIER.ensure_inited(dtb::hart_count());
    if hartid == 0 {
        TIMER_BARRIER.init(dtb::hart_count());
        unsafe {
            sie::set_stimer();
        }
        timer::start(hartid);
        printk!(
            "{}[TEST]{} Timer tick test start ({} harts)",
            ANSI_YELLOW,
            ANSI_RESET,
            TIMER_BARRIER.total()
        );
    }
    // 等待所有 hart 初始化完成
    while TIMER_BARRIER.total() == 0 {}
    TIMER_BARRIER.wait_start();

    let base = timer::get_ticks();
    let mut last = base;

    const TICKS_TO_WAIT: usize = 10;
    for _ in 0..TICKS_TO_WAIT {
        loop {
            let cur = timer::get_ticks();
            if cur > last {
                last = cur;
                break;
            }
            core::hint::spin_loop();
        }
        let delta = last.saturating_sub(base);
        printk!("[hart {}] di da, ticks={}", hartid, delta);
    }

    if TIMER_BARRIER.finish_and_last() {
        printk!("{}[PASS]{} Timer tick test", ANSI_GREEN, ANSI_RESET);
        unsafe {
            sie::clear_stimer();
        }
    }
}

fn uart_output_test(hartid: usize) {
    static UART_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();
    UART_BARRIER.ensure_inited(dtb::hart_count());
    if hartid == 0 {
        UART_BARRIER.init(dtb::hart_count());
        printk!(
            "{}[TEST]{} UART output test start ({} harts)",
            ANSI_YELLOW,
            ANSI_RESET,
            UART_BARRIER.total()
        );
    }
    // 等待所有 hart 初始化完成
    while UART_BARRIER.total() == 0 {}
    UART_BARRIER.wait_start();

    printk!("[hart {}] UART test", hartid);

    if UART_BARRIER.finish_and_last() {
        printk!("{}[PASS]{} UART output test", ANSI_GREEN, ANSI_RESET);
    }
}
