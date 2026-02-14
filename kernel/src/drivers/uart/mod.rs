use crate::mem::PageTable;

pub trait Uart: Send + Sync {
    fn putc(&self, c: u8);
    fn getc(&self) -> Option<u8>;
    fn map_mmio(&self, kpt: &mut PageTable);
}

pub mod ns16550a;
pub mod pl011;
