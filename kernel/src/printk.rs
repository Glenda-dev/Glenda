#![allow(dead_code)]
pub mod printk {
    #[macro_export]
    macro_rules! printk {
    () => { driver_uart::print!("\n") };
    ($fmt:expr) => { driver_uart::print!(concat!($fmt, "\n")) };
    ($fmt:expr, $($arg:tt)*) => { driver_uart::print!(concat!($fmt, "\n"), $($arg)*) };
}
    pub const ANSI_RESET: &str = "\x1b[0m";
    pub const ANSI_RED: &str = "\x1b[31m";
    pub const ANSI_GREEN: &str = "\x1b[32m";
    pub const ANSI_YELLOW: &str = "\x1b[33m";
    pub const ANSI_BLUE: &str = "\x1b[34m";
}
