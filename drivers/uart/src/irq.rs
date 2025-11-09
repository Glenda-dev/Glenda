use super::UART;

#[cfg(feature = "unicode")]
use crate::utf8::{CONSOLE_ECHO, Utf8PushResult, char_display_width};

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

        #[cfg(feature = "unicode")]
        {
            match b {
                b'\r' | b'\n' => {
                    let mut con = CONSOLE_ECHO.lock();
                    con.decoder.clear();
                    con.clear_line();
                    super::print!("\n");
                }
                0x08 | 0x7f => {
                    let mut con = CONSOLE_ECHO.lock();
                    if con.decoder.has_pending() {
                        con.decoder.clear();
                    } else if let Some(w) = con.pop_width() {
                        for _ in 0..w {
                            super::print!("\x08 \x08");
                        }
                    } else {
                        // Nothing
                    }
                }
                b if b < 0x80 => {
                    let mut con = CONSOLE_ECHO.lock();
                    if con.decoder.has_pending() {
                        super::print!("\u{FFFD}");
                        con.push_width(1);
                        con.decoder.clear();
                    }
                    let ch = b as char;
                    super::print!("{}", ch);
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
                            super::print!("{}", c);
                            con.push_width(w);
                        }
                        Utf8PushResult::Invalid => {
                            super::print!("\u{FFFD}");
                            con.push_width(1);
                        }
                    }
                }
            }
        }

        #[cfg(not(feature = "unicode"))]
        {
            match b {
                b'\r' | b'\n' => {
                    super::print!("\n");
                }
                0x08 | 0x7f => {
                    super::print!("\x08 \x08");
                }
                _ => {
                    let ch = b as char;
                    super::print!("{}", ch);
                }
            }
        }
    }
}
