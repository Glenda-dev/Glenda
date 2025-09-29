#![allow(dead_code)]

use driver_uart;
use spin::Mutex;

static PRINTK_LOCK: Mutex<()> = Mutex::new(());
pub fn _printk(args: core::fmt::Arguments) {
    let _guard = PRINTK_LOCK.lock();
    driver_uart::_print(args);
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
pub const ANSI_MAGENTA: &str = "\x1b[35m";
pub const ANSI_CYAN: &str = "\x1b[36m";
pub const ANSI_WHITE: &str = "\x1b[37m";
