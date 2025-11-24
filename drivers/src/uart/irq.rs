use super::UART;

#[cfg(feature = "uart-unicode")]
use crate::uart::utf8::{CONSOLE_ECHO, Utf8PushResult, char_display_width};

pub fn handler() {
    let uart = UART.get().expect("UART not initialized in interrupt handler");
    let lsr = uart.lsr;
    let rbr = uart.thr;
    const LSR_DR: u8 = 0x01;

    loop {
        let status = unsafe { core::ptr::read_volatile(lsr) };
        if (status & LSR_DR) == 0 {
            break;
        }
        let b = unsafe { core::ptr::read_volatile(rbr) };

        #[cfg(feature = "uart-unicode")]
        {
            match b {
                b'\r' | b'\n' => {
                    let mut con = CONSOLE_ECHO.lock();
                    con.decoder.clear();
                    con.clear_line();
                    uart.puts("\n");
                }
                0x08 | 0x7f => {
                    let mut con = CONSOLE_ECHO.lock();
                    if con.decoder.has_pending() {
                        con.decoder.clear();
                    } else if let Some(w) = con.pop_width() {
                        for _ in 0..w {
                            uart.puts("\x08 \x08");
                        }
                    } else {
                        // Nothing
                    }
                }
                b if b < 0x80 => {
                    let mut con = CONSOLE_ECHO.lock();
                    if con.decoder.has_pending() {
                        uart.puts("\u{FFFD}");
                        con.push_width(1);
                        con.decoder.clear();
                    }
                    let ch = b as char;
                    let mut buf = [0u8; 4];
                    uart.puts(ch.encode_utf8(&mut buf));
                    con.push_width(1);
                }
                _ => {
                    let mut con = CONSOLE_ECHO.lock();
                    match con.decoder.push(b) {
                        Utf8PushResult::Pending => {
                            // wait more
                        }
                        Utf8PushResult::Completed(c) => {
                            let w = char_display_width(c);
                            let mut buf = [0u8; 4];
                            uart.puts(c.encode_utf8(&mut buf));
                            con.push_width(w);
                        }
                        Utf8PushResult::Invalid => {
                            uart.puts("\u{FFFD}");
                            con.push_width(1);
                        }
                    }
                }
            }
        }

        #[cfg(not(feature = "uart-unicode"))]
        {
            match b {
                b'\r' | b'\n' => {
                    uart.puts("\n");
                }
                0x08 | 0x7f => {
                    uart.puts("\x08 \x08");
                }
                _ => {
                    let ch = b as char;
                    uart.putb(ch as u8);
                }
            }
        }
    }
}

pub fn enable() {
    let cfg = UART.get().expect("UART not initialized in interrupt enable").cfg;
    let base = cfg.base;
    let lsr_off = cfg.lsr_offset;
    let stride = if lsr_off >= 5 { lsr_off / 5 } else { 1 };
    let ier = (base + stride * 1) as *mut u8;
    unsafe { core::ptr::write_volatile(ier, 0x01) };
}
