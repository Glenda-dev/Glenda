#![allow(dead_code)]

use crate::dtb;
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

// TODO: Refactor This Out
// 一些测试点会因为 printk! 的竞争导致死锁，这些函数用于暂时规避此类问题
// 日后需要做更 Robust 的 printk! 实现
#[inline(always)]
fn uart_base() -> usize {
    dtb::uart_config().unwrap_or(driver_uart::DEFAULT_QEMU_VIRT).base()
}

#[inline(always)]
pub fn uart_putb(b: u8) {
    const LSR_OFF: usize = 5;
    const THR_OFF: usize = 0;
    const THRE: u8 = 0x20;
    unsafe {
        let lsr = (uart_base() + LSR_OFF) as *const u8;
        let thr = (uart_base() + THR_OFF) as *mut u8;
        while core::ptr::read_volatile(lsr) & THRE == 0 {}
        core::ptr::write_volatile(thr, b);
    }
}

pub fn uart_puts(s: &str) {
    for &b in s.as_bytes() {
        uart_putb(b);
    }
}

pub fn uart_hex(x: usize) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    uart_puts("0x");
    let mut started = false;
    for i in (0..(core::mem::size_of::<usize>() * 2)).rev() {
        let nyb = (x >> (i * 4)) & 0xF;
        if nyb != 0 || started || i == 0 {
            started = true;
            uart_putb(HEX[nyb]);
        }
    }
}
