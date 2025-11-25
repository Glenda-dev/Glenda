use super::barrier::MultiCoreTestBarrier;
use crate::dtb;
use crate::printk;
use crate::printk::{
    ANSI_BLUE, ANSI_CYAN, ANSI_GREEN, ANSI_MAGENTA, ANSI_RED, ANSI_RESET, ANSI_WHITE, ANSI_YELLOW,
};

static PRINTK_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();

pub fn run(hartid: usize) {
    if hartid == 0 {
        PRINTK_BARRIER.init(dtb::hart_count());
        printk!(
            "{}[TEST]{} Printk test start ({} harts)\n",
            ANSI_YELLOW,
            ANSI_RESET,
            PRINTK_BARRIER.total()
        );
    }
    // 其它 hart 等待 init 完成
    while PRINTK_BARRIER.total() == 0 {}
    PRINTK_BARRIER.wait_start();

    // 每个 hart 做一次彩色输出，便于观测并发打印是否串行化良好
    printk_test(hartid);

    // 结束同步：最后一个 hart 输出 PASS
    if PRINTK_BARRIER.finish_and_last() {
        printk!("{}[PASS]{} Printk test\n", ANSI_GREEN, ANSI_RESET);
    }
}

fn printk_test(hart: usize) {
    // 使用各色前缀标识 hart
    let color = match hart % 7 {
        0 => ANSI_RED,
        1 => ANSI_GREEN,
        2 => ANSI_YELLOW,
        3 => ANSI_BLUE,
        4 => ANSI_MAGENTA,
        5 => ANSI_CYAN,
        _ => ANSI_WHITE,
    };
    printk!(
        "{}[hart {}]{} Colors => {}Red{} {}Green{} {}Yellow{} {}Blue{} {}Magenta{} {}Cyan{} {}White{}\n",
        color,
        hart,
        ANSI_RESET,
        ANSI_RED,
        ANSI_RESET,
        ANSI_GREEN,
        ANSI_RESET,
        ANSI_YELLOW,
        ANSI_RESET,
        ANSI_BLUE,
        ANSI_RESET,
        ANSI_MAGENTA,
        ANSI_RESET,
        ANSI_CYAN,
        ANSI_RESET,
        ANSI_WHITE,
        ANSI_RESET
    );
}
