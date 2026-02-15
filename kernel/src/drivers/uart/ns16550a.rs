// A busy-wait 16550A-compatible UART Driver

use crate::hal::mem::PGSIZE;
use crate::mem::addr::phys_to_virt;
use crate::mem::{PageTable, Perms, PhysAddr};
use core::cmp;
use core::fmt::{self, Write};
use core::ptr::{read_volatile, write_volatile};
use fdt::node::FdtNode;

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub base: usize,
    pub thr_offset: usize,
    pub lsr_offset: usize,
    pub lsr_thre_bit: u8,
}

impl Config {
    const THR_REGISTER_INDEX: usize = 0;
    const LSR_REGISTER_INDEX: usize = 5;
    const DEFAULT_LSR_THRE: u8 = 0x20;

    pub const fn new(base: usize, thr_offset: usize, lsr_offset: usize, lsr_thre_bit: u8) -> Self {
        Self { base, thr_offset, lsr_offset, lsr_thre_bit }
    }

    pub fn from_fdt(node: &FdtNode<'_, '_>) -> Option<Self> {
        if !is_ns16550_compatible(node) {
            return None;
        }

        let mut regions = node.reg()?;
        let region = regions.next()?;
        let base = region.starting_address as usize;
        let stride = register_stride(node);

        Some(Self {
            base,
            thr_offset: Self::THR_REGISTER_INDEX * stride,
            lsr_offset: Self::LSR_REGISTER_INDEX * stride,
            lsr_thre_bit: Self::DEFAULT_LSR_THRE,
        })
    }
}

pub struct Uart {
    cfg: Config,
    size: usize,
}

unsafe impl Send for Uart {}
unsafe impl Sync for Uart {}

impl Uart {
    pub const fn from_config(cfg: Config, size: usize) -> Self {
        Self { cfg, size }
    }

    #[inline(always)]
    fn putb(&self, b: u8) {
        unsafe {
            let base_va = phys_to_virt(PhysAddr::from(self.cfg.base)).as_usize();
            let thr = (base_va + self.cfg.thr_offset) as *mut u8;
            let lsr = (base_va + self.cfg.lsr_offset) as *const u8;

            while (read_volatile(lsr) & self.cfg.lsr_thre_bit) == 0 {}
            write_volatile(thr, b);
        }
    }

    fn get(&self) -> Option<u8> {
        unsafe {
            let base_va = phys_to_virt(PhysAddr::from(self.cfg.base)).as_usize();
            let thr = (base_va + self.cfg.thr_offset) as *const u8;
            let lsr = (base_va + self.cfg.lsr_offset) as *const u8;

            if (read_volatile(lsr) & 1) == 0 { None } else { Some(read_volatile(thr)) }
        }
    }
}

impl super::Uart for Uart {
    fn putc(&self, b: u8) {
        self.putb(b);
    }

    fn getc(&self) -> Option<u8> {
        self.get()
    }

    fn map_mmio(&self, kpt: &mut PageTable) {
        let pa = PhysAddr::from(self.cfg.base).align_down(PGSIZE);
        let va = phys_to_virt(pa);
        let size = (self.size + PGSIZE - 1) / PGSIZE * PGSIZE;
        let flags = Perms::READ | Perms::WRITE | Perms::ACCESSED | Perms::DIRTY | Perms::GLOBAL;
        log!(
            "ns16550a: Map MMIO [{:#x}, {:#x}) -> [{:#x}, {:#x}) {}",
            pa.as_usize(),
            (pa + size).as_usize(),
            va.as_usize(),
            (va + size).as_usize(),
            flags
        );
        kpt.map_with_alloc(va, pa, size, flags);
    }
}

pub struct UartWriter<'a>(pub &'a Uart);

impl<'a> Write for UartWriter<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for ch in s.bytes() {
            if ch == b'\n' {
                self.0.putb(b'\r');
            }
            self.0.putb(ch);
        }
        Ok(())
    }
    fn write_char(&mut self, c: char) -> fmt::Result {
        if c == '\n' {
            self.0.putb(b'\r');
        }
        let mut buf = [0u8; 4];
        for &b in c.encode_utf8(&mut buf).as_bytes() {
            self.0.putb(b);
        }
        Ok(())
    }
}

/*
 Fallback: QEMU Virt

 当设备树解析失败时，回退到 QEMU Virt
 Also see: kernel/src/main.rs
*/
pub const DEFAULT_QEMU_VIRT: Config = Config::new(
    0x1000_0000, // base
    0x00,        // THR offset
    0x05,        // LSR offset
    0x20,        // LSR.THRE
);

/*
 See SPEC: https://devicetree-specification.readthedocs.io/en/stable/device-bindings.html
*/
fn register_stride(node: &FdtNode<'_, '_>) -> usize {
    let reg_shift = node.property("reg-shift").and_then(|prop| prop.as_usize()).unwrap_or(0);
    let reg_io_width = node.property("reg-io-width").and_then(|prop| prop.as_usize()).unwrap_or(1);

    let shift_multiplier = if reg_shift < usize::BITS as usize { 1usize << reg_shift } else { 0 };

    let stride =
        reg_io_width.saturating_mul(if shift_multiplier == 0 { 1 } else { shift_multiplier });
    cmp::max(stride, 1)
}

fn is_ns16550_compatible(node: &FdtNode<'_, '_>) -> bool {
    node.compatible()
        .map(|compat| compat.all().any(|name| name.contains("ns16550")))
        .unwrap_or(false)
}
