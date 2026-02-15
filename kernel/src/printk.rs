use crate::hal::console;
use crate::sync::SpinLock;
use core::fmt::Arguments;
use core::sync::atomic::{AtomicBool, Ordering};

static PRINTK_LOCK: SpinLock<()> = SpinLock::new(());
pub static VERBOSE: AtomicBool = AtomicBool::new(true);

pub fn set_verbose(enable: bool) {
    VERBOSE.store(enable, Ordering::Relaxed);
}

pub fn is_verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

pub fn _printk(args: Arguments) {
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

#[cfg(feature = "logging")]
#[macro_export]
macro_rules! log {
    ($fmt:expr) => {
        if crate::printk::is_verbose() {
            crate::printk!("{}\n", format_args!($fmt));
        }
    };
    ($fmt:expr, $($arg:tt)*) => {
        if crate::printk::is_verbose() {
            crate::printk!("{}\n", format_args!($fmt, $($arg)*));
        }
    };
}

#[cfg(feature = "logging")]
#[macro_export]
macro_rules! error {
    ($fmt:expr) => {
        crate::printk!("{}{}{}\n", crate::printk::ANSI_RED, format_args!($fmt), crate::printk::ANSI_RESET);
    };
    ($fmt:expr, $($arg:tt)*) => {
        crate::printk!("{}{}{}\n", crate::printk::ANSI_RED, format_args!($fmt, $($arg)*), crate::printk::ANSI_RESET);
    };
}

#[cfg(feature = "logging")]
#[macro_export]
macro_rules! warn {
    ($fmt:expr) => {
        crate::printk!("{}{}{}\n", crate::printk::ANSI_YELLOW, format_args!($fmt), crate::printk::ANSI_RESET);
    };
    ($fmt:expr, $($arg:tt)*) => {
        crate::printk!("{}{}{}\n", crate::printk::ANSI_YELLOW, format_args!($fmt, $($arg)*), crate::printk::ANSI_RESET);
    };
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! log {
    ($fmt:expr) => {};
    ($fmt:expr, $($arg:tt)*) => {};
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! error {
    ($fmt:expr) => {};
    ($fmt:expr, $($arg:tt)*) => {};
}

#[cfg(not(feature = "logging"))]
#[macro_export]
macro_rules! warn {
    ($fmt:expr) => {};
    ($fmt:expr, $($arg:tt)*) => {};
}

pub const ANSI_RESET: &str = "\x1b[0m";
pub const ANSI_RED: &str = "\x1b[31m";
pub const ANSI_GREEN: &str = "\x1b[32m";
pub const ANSI_YELLOW: &str = "\x1b[33m";
pub const ANSI_BLUE: &str = "\x1b[34m";
pub const ANSI_MAGENTA: &str = "\x1b[35m";
pub const ANSI_CYAN: &str = "\x1b[36m";
pub const ANSI_WHITE: &str = "\x1b[37m";
pub const ANSI_CLEAR: &str = "\x1b[2J\x1b[H";
