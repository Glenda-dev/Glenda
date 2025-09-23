use crate::printk;
use crate::printk::{
    ANSI_BLUE, ANSI_CYAN, ANSI_GREEN, ANSI_MAGENTA, ANSI_RED, ANSI_RESET, ANSI_WHITE, ANSI_YELLOW,
};
pub fn run() {
    printk_test();
    printk!("{}[PASS]{} Printk test", ANSI_GREEN, ANSI_RESET);
}
fn printk_test() {
    printk!("{}printk test start{}", ANSI_BLUE, ANSI_RESET);
    printk!(
        "{}Red{} {}Green{} {}Yellow{} {}Blue{} {}Magenta{} {}Cyan{} {}White{}",
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
