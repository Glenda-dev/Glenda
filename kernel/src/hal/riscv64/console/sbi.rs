use crate::hal::riscv64::sbi;
use core::fmt::{self, Write};

pub struct SBIWriter;

impl Write for SBIWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            if c == b'\n' {
                sbi::put_char(b'\r');
            }
            sbi::put_char(c);
        }
        Ok(())
    }
    fn write_char(&mut self, c: char) -> fmt::Result {
        if c == '\n' {
            sbi::put_char(b'\r');
        }
        let mut buf = [0u8; 4];
        for &b in c.encode_utf8(&mut buf).as_bytes() {
            sbi::put_char(b);
        }
        Ok(())
    }
}
