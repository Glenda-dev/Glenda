use core::arch::asm;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, Ordering};

const COM1: u16 = 0x3f8;
const REG_DATA: u16 = 0;
const REG_INT_ENABLE: u16 = 1;
const REG_FIFO_CTRL: u16 = 2;
const REG_LINE_CTRL: u16 = 3;
const REG_MODEM_CTRL: u16 = 4;
const REG_LINE_STATUS: u16 = 5;

static INITIALIZED: AtomicBool = AtomicBool::new(false);

#[inline(always)]
unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn ensure_init() {
    if INITIALIZED.load(Ordering::Relaxed) {
        return;
    }

    unsafe {
        outb(COM1 + REG_INT_ENABLE, 0x00);
        outb(COM1 + REG_LINE_CTRL, 0x80);
        outb(COM1 + REG_DATA, 0x01);
        outb(COM1 + REG_INT_ENABLE, 0x00);
        outb(COM1 + REG_LINE_CTRL, 0x03);
        outb(COM1 + REG_FIFO_CTRL, 0xc7);
        outb(COM1 + REG_MODEM_CTRL, 0x0b);
    }

    INITIALIZED.store(true, Ordering::Relaxed);
}

fn putchar(byte: u8) {
    ensure_init();
    unsafe {
        while (inb(COM1 + REG_LINE_STATUS) & 0x20) == 0 {}
        outb(COM1 + REG_DATA, byte);
    }
}

pub fn init() {
    ensure_init();
}

struct SerialWriter;

impl Write for SerialWriter {
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

pub fn print(arg: core::fmt::Arguments) {
    let _ = SerialWriter.write_fmt(arg);
}

pub fn read() -> u8 {
    ensure_init();
    unsafe {
        while (inb(COM1 + REG_LINE_STATUS) & 0x01) == 0 {}
        inb(COM1 + REG_DATA)
    }
}
