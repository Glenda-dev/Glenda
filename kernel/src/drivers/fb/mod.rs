pub mod font;

use crate::boot::FrameBufferInfo;
use core::fmt::Write;
use core::sync::atomic::{AtomicU32, Ordering};

pub struct FramebufferWriter {
    info: FrameBufferInfo,
    x: AtomicU32,
    y: AtomicU32,
}

impl FramebufferWriter {
    pub const fn new(info: FrameBufferInfo) -> Self {
        Self { info, x: AtomicU32::new(0), y: AtomicU32::new(0) }
    }

    pub fn get_info(&self) -> FrameBufferInfo {
        self.info
    }

    pub fn draw_pixel(&self, x: u32, y: u32, color: u32) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }
        let offset = (y * self.info.pitch / 4 + x) as usize;
        let ptr = self.info.address.as_mut_ptr::<u32>();
        unsafe {
            *ptr.add(offset) = color;
        }
    }

    pub fn draw_char(&self, char: char, color: u32) {
        let mut x = self.x.load(Ordering::Relaxed);
        let mut y = self.y.load(Ordering::Relaxed);

        match char {
            '\n' => {
                x = 0;
                y += font::FONT_HEIGHT as u32;
            }
            '\r' => {
                x = 0;
            }
            _ => {
                if x + font::FONT_WIDTH as u32 > self.info.width {
                    x = 0;
                    y += font::FONT_HEIGHT as u32;
                }
                if y + font::FONT_HEIGHT as u32 > self.info.height {
                    self.scroll();
                    y -= font::FONT_HEIGHT as u32;
                }

                let glyph = font::get_glyph(char).unwrap_or_else(|| font::get_glyph(' ').unwrap());
                for (row, byte) in glyph.iter().enumerate() {
                    for col in 0..font::FONT_WIDTH {
                        if (byte >> col) & 1 == 1 {
                            self.draw_pixel(x + col as u32, y + row as u32, color);
                        } else {
                            self.draw_pixel(x + col as u32, y + row as u32, 0x000000);
                        }
                    }
                }
                x += font::FONT_WIDTH as u32;
            }
        }
        self.x.store(x, Ordering::Relaxed);
        self.y.store(y, Ordering::Relaxed);
    }

    fn scroll(&self) {
        let pitch_in_u32 = self.info.pitch / 4;
        let height = self.info.height;
        let font_height = font::FONT_HEIGHT as u32;
        let ptr = self.info.address.as_mut_ptr::<u32>();

        unsafe {
            // Move rows up
            core::ptr::copy(
                ptr.add((font_height * pitch_in_u32) as usize),
                ptr,
                ((height - font_height) * pitch_in_u32) as usize,
            );
            // Clear last row
            core::ptr::write_bytes(
                ptr.add(((height - font_height) * pitch_in_u32) as usize),
                0,
                (font_height * pitch_in_u32) as usize,
            );
        }
    }

    pub fn clear(&self) {
        let pitch_in_u32 = self.info.pitch / 4;
        let height = self.info.height;
        let ptr = self.info.address.as_mut_ptr::<u32>();
        unsafe {
            core::ptr::write_bytes(ptr, 0, (height * pitch_in_u32) as usize);
        }
        self.x.store(0, Ordering::Relaxed);
        self.y.store(0, Ordering::Relaxed);
    }
}

pub struct FbWriter<'a>(pub &'a FramebufferWriter);

impl<'a> Write for FbWriter<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.0.draw_char(c, 0xFFFFFF);
        }
        Ok(())
    }
}
