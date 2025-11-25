use crate::hart;
use drivers::uart::_print;
use spin::Mutex;

static PRINTK_LOCK: Mutex<()> = Mutex::new(());
pub fn _printk(args: core::fmt::Arguments) {
    if hart::get().nest_count > 0 {
        _print(args);
        return;
    }
    let _guard = PRINTK_LOCK.lock();
    _print(args);
}
#[macro_export]
macro_rules! printk {
    ($fmt:expr) => { crate::printk::_printk(format_args!($fmt)) };
    ($fmt:expr, $($arg:tt)*) => { crate::printk::_printk(format_args!($fmt, $($arg)*)) };
}

pub const ANSI_RESET: &str = "\x1b[0m";
pub const ANSI_RED: &str = "\x1b[31m";
pub const ANSI_GREEN: &str = "\x1b[32m";
pub const ANSI_YELLOW: &str = "\x1b[33m";
pub const ANSI_BLUE: &str = "\x1b[34m";
pub const ANSI_MAGENTA: &str = "\x1b[35m";
pub const ANSI_CYAN: &str = "\x1b[36m";
pub const ANSI_WHITE: &str = "\x1b[37m";
