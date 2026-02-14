pub mod sbi;
use super::drivers as hal_drivers;
use super::sbi as sbi_driver;
use crate::drivers;
use core::fmt::Write;

pub fn init() {
    // 逻辑已移动至 hal::init()，通过驱动探测完成
}

pub fn print(args: core::fmt::Arguments) {
    if let Some(driver) = hal_drivers::UART.get() {
        match driver {
            hal_drivers::UartDriver::Ns16550a(uart) => {
                let _ = drivers::uart::ns16550a::UartWriter(uart).write_fmt(args);
            }
            hal_drivers::UartDriver::Pl011(uart) => {
                let _ = drivers::uart::pl011::UartWriter(uart).write_fmt(args);
            }
        }
    } else {
        let _ = sbi::SBIWriter.write_fmt(args);
    }
}

pub fn read() -> u8 {
    loop {
        if let Some(driver) = hal_drivers::UART.get() {
            if let Some(c) = driver.getc() {
                return c;
            }
        } else {
            if let Some(c) = sbi_driver::get_char() {
                sbi_driver::put_char(c); // 回显
                return c;
            }
        }
    }
}
