use crate::mem::PageTable;

pub trait InterruptController: Send + Sync {
    fn map_mmio(&self, kpt: &mut PageTable);
    fn set_priority(&self, irq: usize, priority: usize);
    fn set_enable(&self, hartid: usize, irq: usize, enable: bool);
    fn set_threshold(&self, hartid: usize, threshold: usize);
    fn claim(&self, hartid: usize) -> usize;
    fn complete(&self, hartid: usize, irq: usize);
}

pub mod aplic;
pub mod gicv2;
pub mod plic;
