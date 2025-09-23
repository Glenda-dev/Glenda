#![allow(dead_code)]

use crate::lock::SpinLock;
use driver_uart;

static PRINTK_LOCK: SpinLock = SpinLock::new();
pub fn _printk(args: core::fmt::Arguments) {
    PRINTK_LOCK.lock();
    let _ = driver_uart::_print(args);
    PRINTK_LOCK.unlock();
}
#[macro_export]
macro_rules! printk {
    () => { printk::_printk(format_args!("\n")) };
    ($fmt:expr) => { printk::_printk(format_args!(concat!($fmt, "\n"))) };
    ($fmt:expr, $($arg:tt)*) => { printk::_printk(format_args!(concat!($fmt, "\n"), $($arg)*)) };
}
pub const ANSI_RESET: &str = "\x1b[0m";
pub const ANSI_RED: &str = "\x1b[31m";
pub const ANSI_GREEN: &str = "\x1b[32m";
pub const ANSI_YELLOW: &str = "\x1b[33m";
pub const ANSI_BLUE: &str = "\x1b[34m";
