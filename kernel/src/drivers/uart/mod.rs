// A busy-wait 16550A-compatible UART Driver

pub mod irq;
#[cfg(feature = "uart-unicode")]
pub mod utf8;

use crate::dtb;
use core::cmp;
use core::fmt::{self, Write};
use core::ptr::{read_volatile, write_volatile};
use fdt::node::FdtNode;
use spin::Once;

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
    thr: *mut u8,
    lsr: *const u8,
    lsr_thre: u8,
    cfg: Config,
}

unsafe impl Send for Uart {}
unsafe impl Sync for Uart {}

impl Uart {
    pub const fn from_config(cfg: Config) -> Self {
        Self {
            cfg,
            thr: (cfg.base + cfg.thr_offset) as *mut u8,
            lsr: (cfg.base + cfg.lsr_offset) as *const u8,
            lsr_thre: cfg.lsr_thre_bit,
        }
    }

    #[inline(always)]
    pub fn putb(&self, b: u8) {
        unsafe {
            while (read_volatile(self.lsr) & self.lsr_thre) == 0 {}
            write_volatile(self.thr, b);
        }
    }

    pub fn puts(&self, s: &str) {
        for &b in s.as_bytes() {
            self.putb(b);
        }
    }
}

struct UartWriter<'a>(&'a Uart);

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

static UART: Once<Uart> = Once::new();

pub fn init(cfg: Config) {
    UART.call_once(|| Uart::from_config(cfg));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    if let Some(uart) = UART.get() {
        let _ = UartWriter(uart).write_fmt(args);
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::_print(core::format_args!($($arg)*));
    }}
}

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

pub fn initialize_from_dtb(dtb: *const u8) {
    if let Some(cfg) = dtb::uart_config() {
        init(cfg);
    } else {
        init(DEFAULT_QEMU_VIRT);
    }
}
