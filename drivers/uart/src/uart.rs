#![no_std]
#![allow(dead_code)]

use core::fmt::{self, Write};
use core::ptr::{read_volatile, write_volatile};
/*
  TODO: dtb parsing
  `0x10000000' 是 `qemu-system-riscv64' 中 `virt' 机器模型的串口地址

  理论上不可以直接硬编码下面的内容，不然会失去其它硬件的可移植性
  正确方式是解析 `glenda_main(reg a0, reg a1)' 中传进来的设备树，但我还没写
*/
pub struct Uart;

static mut UART_BASE: usize = 0x10000000;
static mut UART_THR: usize = 0x00;
static mut UART_LSR: usize = 0x05;
static mut LSR_THRE: u8 = 0x20;

#[inline(always)]
fn mmio_r8(addr: usize) -> u8 {
    unsafe { read_volatile(addr as *const u8) }
}

#[inline(always)]
fn mmio_w8(addr: usize, value: u8) {
    unsafe { write_volatile(addr as *mut u8, value) }
}

impl Uart {
    #[inline(always)]
    fn putb(b: u8) {
        unsafe {
            while (mmio_r8(UART_BASE + UART_LSR) & LSR_THRE) == 0 {}
            mmio_w8(UART_BASE + UART_THR, b);
        }
    }
    pub fn _print(args: core::fmt::Arguments) {
        let _ = Uart.write_fmt(args);
    }
    pub fn init(base: usize, thr: usize, lsr: usize, thre: u8) {
        unsafe {
            UART_BASE = base;
            UART_THR = thr;
            UART_LSR = lsr;
            LSR_THRE = thre;
        }
    }
}

impl Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for ch in s.bytes() {
            if ch == b'\n' {
                Self::putb(b'\r');
            }
            Self::putb(ch);
        }
        Ok(())
    }
    fn write_char(&mut self, c: char) -> fmt::Result {
        if c == '\n' {
            Self::putb(b'\r');
        }
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
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::_print(core::format_args!($($arg)*));
    }}
}
