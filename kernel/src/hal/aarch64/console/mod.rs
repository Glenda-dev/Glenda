pub mod pl011;
use super::drivers as hal_drivers;
use crate::drivers;
use core::fmt::Write;

pub fn print(args: core::fmt::Arguments) {
    if let Some(fb) = hal_drivers::FB.get() {
        let _ = fb.write_fmt(args);
    }
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
        let _ = pl011::PL011Writer.write_fmt(args);
    }
}

pub fn read() -> u8 {
    loop {
        if let Some(driver) = hal_drivers::UART.get() {
            if let Some(c) = driver.getc() {
                return c;
            }
        } else {
            // Fallback to direct read if needed, but for now just loop
        }
    }
}
