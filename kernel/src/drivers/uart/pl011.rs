use super::Uart;
use crate::hal::mem::PGSIZE;
use crate::mem::addr::phys_to_virt;
use crate::mem::{PageTable, Perms, PhysAddr};
use core::fmt::{self, Write};
use core::ptr::{read_volatile, write_volatile};

pub struct Pl011 {
    base: usize,
    size: usize,
}

impl Pl011 {
    pub const fn new(base: usize, size: usize) -> Self {
        Self { base, size }
    }
}

impl super::Uart for Pl011 {
    fn putc(&self, c: u8) {
        unsafe {
            let base_va = phys_to_virt(PhysAddr::from(self.base)).as_usize();
            let dr = base_va as *mut u32;
            let fr = (base_va + 0x18) as *const u32;
            while read_volatile(fr) & (1 << 5) != 0 {} // Wait until TX FIFO is not full
            write_volatile(dr, c as u32);
        }
    }

    fn getc(&self) -> Option<u8> {
        unsafe {
            let base_va = phys_to_virt(PhysAddr::from(self.base)).as_usize();
            let dr = base_va as *const u32;
            let fr = (base_va + 0x18) as *const u32;
            if read_volatile(fr) & (1 << 4) != 0 {
                // RX FIFO empty
                None
            } else {
                Some((read_volatile(dr) & 0xFF) as u8)
            }
        }
    }

    fn map_mmio(&self, kpt: &mut PageTable) {
        let pa = PhysAddr::from(self.base).align_down(PGSIZE);
        let va = phys_to_virt(pa);
        let size = (self.size + PGSIZE - 1) / PGSIZE * PGSIZE;
        let flags = Perms::READ | Perms::WRITE | Perms::DEVICE;
        log!(
            "pl011: Map MMIO [{:#x}, {:#x}) -> [{:#x}, {:#x}) {}",
            pa.as_usize(),
            (pa + size).as_usize(),
            va.as_usize(),
            (va + size).as_usize(),
            flags
        );
        kpt.map_with_alloc(va, pa, size, flags);
    }
}

pub struct UartWriter<'a>(pub &'a Pl011);

impl<'a> Write for UartWriter<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                self.0.putc(b'\r');
            }
            self.0.putc(b);
        }
        Ok(())
    }
}
