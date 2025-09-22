/*
 UART Driver for Glenda
 */

#![allow(dead_code)]

use core::fmt::{self, Write};

// TODO: dtb parsing
// `0x10000000' 是 `qemu-system-riscv64' 中 `virt' 机器模型的串口地址
// 理论上不可以直接硬编码下面的内容，不然会失去其它硬件的可移植性
// 正确方式是解析 `glenda_main(reg a0, reg a1)' 中传进来的文件树，但我还没写
const UART0_BASE: usize = 0x10000000;
const UART_THR:   usize = 0x00; // TX Holding Register
const UART_LSR:   usize = 0x05; // Line Status Register
const LSR_THRE:   u8    = 0x20; // THR Empty

#[inline(always)]
fn mmio_r8(addr: usize) -> u8 {
    unsafe { core::ptr::read_volatile(addr as *const u8) }
}
#[inline(always)]
fn mmio_w8(addr: usize, v: u8) {
    unsafe { core::ptr::write_volatile(addr as *mut u8, v) }
}

/// INFO: 16550 是全局唯一寄存器映射，所以不用持有任何状态
pub struct Uart;

impl Uart {
    #[inline(always)]
    fn putb(b: u8) {
        while (mmio_r8(UART0_BASE + UART_LSR) & LSR_THRE) == 0 {}
        mmio_w8(UART0_BASE + UART_THR, b);
    }
}

impl Write for Uart {
    #[inline(always)]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for ch in s.bytes() {
            if ch == b'\n' { Self::putb(b'\r'); }
            Self::putb(ch);
        }
        Ok(())
    }

    #[inline(always)]
    fn write_char(&mut self, c: char) -> fmt::Result {
        if c == '\n' { Self::putb(b'\r'); }
        let mut buf = [0u8; 4];
        for &b in c.encode_utf8(&mut buf).as_bytes() {
            Self::putb(b);
        }
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    let _ = Uart.write_fmt(args);
}

#[macro_export]
// 不自动加换行符
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::driver::uart::_print(core::format_args!($($arg)*));
    }}
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n") };
    ($fmt:expr) => { $crate::print!(concat!($fmt, "\n")) };
    ($fmt:expr, $($arg:tt)*) => { $crate::print!(concat!($fmt, "\n"), $($arg)*) };
}

// ANSI escape sequences
// 以下变量是为了颜色显示[1]
// [1]: https://en.wikipedia.org/wiki/ANSI_escape_code
pub const ANSI_RESET:  &str = "\x1b[0m";
pub const ANSI_RED:    &str = "\x1b[31m";
pub const ANSI_GREEN:  &str = "\x1b[32m";
pub const ANSI_YELLOW: &str = "\x1b[33m";
pub const ANSI_BLUE:   &str = "\x1b[34m";
