use core::fmt::{self, Write};
use core::ptr::{read_volatile, write_volatile};
use crate::boot;
use crate::mem::addr::HHDM_OFFSET;

const PL011_BASE: usize = 0x0900_0000;
const UARTDR: usize = 0x00;
const UARTFR: usize = 0x18;
const FR_TXFF: u32 = 1 << 5;
const FR_RXFE: u32 = 1 << 4;

#[inline(always)]
fn base() -> usize {
    if let Some(offset) = HHDM_OFFSET.get() {
        return PL011_BASE + *offset;
    }
    let hhdm = boot::get_hhdm();
    if hhdm != 0 {
        return PL011_BASE + hhdm;
    }
    PL011_BASE
}

pub fn putchar(byte: u8) {
    unsafe {
        let base = base();
        let dr = (base + UARTDR) as *mut u32;
        let fr = (base + UARTFR) as *const u32;
        while read_volatile(fr) & FR_TXFF != 0 {}
        write_volatile(dr, byte as u32);
    }
}

pub struct PL011Writer;

impl Write for PL011Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte == b'\n' {
                putchar(b'\r');
            }
            putchar(byte);
        }
        Ok(())
    }
}
