pub mod sbi;
pub mod uart;

use super::dtb;
use core::fmt::Write;

pub fn init() {
    uart::init(dtb::uart_config().expect("Failed to get UART config from DTB"));
}

pub fn print(args: core::fmt::Arguments) {
    if let Some(uart) = uart::UART.get() {
        let _ = uart::UartWriter(uart).write_fmt(args);
    } else {
        let _ = sbi::SBIWriter.write_fmt(args);
    }
}

pub fn read() -> u8 {
    loop {
        if let Some(uart) = uart::UART.get() {
            if let Some(c) = uart.get() {
                return c;
            }
        } else {
            if let Some(c) = super::sbi::get_char() {
                return c;
            }
        }
    }
}
