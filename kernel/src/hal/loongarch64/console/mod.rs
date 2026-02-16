/// FIXME: should be a service
use core::fmt::Write;

pub const UART_BASE: usize = 0xa00000001fe001e0;

struct Uart;

impl Write for Uart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            self.putc(b);
        }
        Ok(())
    }
}

impl Uart {
    fn putc(&mut self, c: u8) {
        let ptr = UART_BASE as *mut u8;
        unsafe {
            ptr.write_volatile(c);
        }
    }
}

pub fn init() {
    // Basic UART initialization if needed
}

pub fn print(args: core::fmt::Arguments) {
    let mut uart = Uart;
    let _ = uart.write_fmt(args);
}

pub fn read() -> u8 {
    0 // TODO: Implement read
}
