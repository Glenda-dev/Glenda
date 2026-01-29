use crate::cpu;
use crate::hal::console;
use core::fmt::Arguments;
use spin::Mutex;

static PRINTK_LOCK: Mutex<()> = Mutex::new(());
pub fn _printk(args: Arguments) {
    if cpu::get().nest_count > 0 {
        console::print(args);
        return;
    }
    let _guard = PRINTK_LOCK.lock();
    console::print(args);
}
pub fn _printk_unsynced(args: Arguments) {
    console::print(args);
}
#[macro_export]
macro_rules! printk {
    ($fmt:expr) => { crate::printk::_printk(format_args!($fmt)) };
    ($fmt:expr, $($arg:tt)*) => { crate::printk::_printk(format_args!($fmt, $($arg)*)) };
}
#[macro_export]
macro_rules! printk_unsynced {
    ($fmt:expr) => { crate::printk::_printk_unsynced(format_args!($fmt)) };
    ($fmt:expr, $($arg:tt)*) => { crate::printk::_printk_unsynced(format_args!($fmt, $($arg)*)) };
}

pub const ANSI_RESET: &str = "\x1b[0m";
pub const ANSI_RED: &str = "\x1b[31m";
pub const ANSI_GREEN: &str = "\x1b[32m";
pub const ANSI_YELLOW: &str = "\x1b[33m";
pub const ANSI_BLUE: &str = "\x1b[34m";
pub const ANSI_MAGENTA: &str = "\x1b[35m";
pub const ANSI_CYAN: &str = "\x1b[36m";
pub const ANSI_WHITE: &str = "\x1b[37m";
