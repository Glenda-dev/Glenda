use crate::hal::mem::PGSIZE;
use crate::mem::addr::phys_to_virt;
use crate::mem::{PageTable, Perms, PhysAddr};
use crate::sync::SpinLock;
use core::ptr::{read_volatile, write_volatile};

const GICC_CTLR: usize = 0x0000;
const GICC_PMR: usize = 0x0004;
const GICC_IAR: usize = 0x000C;
const GICC_EOIR: usize = 0x0010;

const GICD_CTLR: usize = 0x0000;
const GICD_ISENABLER: usize = 0x0100;
const GICD_ICENABLER: usize = 0x0180;
const GICD_IPRIORITYR: usize = 0x0400;
const GICD_ITARGETSR: usize = 0x0800;

pub struct GicV2 {
    dist_base: usize,
    cpu_base: usize,
    dist_size: usize,
    cpu_size: usize,
    lock: SpinLock<()>,
}

impl GicV2 {
    pub const fn new(dist_base: usize, cpu_base: usize, dist_size: usize, cpu_size: usize) -> Self {
        Self { dist_base, cpu_base, dist_size, cpu_size, lock: SpinLock::new(()) }
    }

    #[inline(always)]
    fn dist_va(&self) -> usize {
        phys_to_virt(PhysAddr::from(self.dist_base)).as_usize()
    }

    #[inline(always)]
    fn cpu_va(&self) -> usize {
        phys_to_virt(PhysAddr::from(self.cpu_base)).as_usize()
    }
}

impl super::InterruptController for GicV2 {
    fn map_mmio(&self, kpt: &mut PageTable) {
        let flags = Perms::READ | Perms::WRITE | Perms::DEVICE;

        let dist_pa = PhysAddr::from(self.dist_base).align_down(PGSIZE);
        let dist_size = self.dist_size.div_ceil(PGSIZE) * PGSIZE;
        kpt.map_with_alloc(phys_to_virt(dist_pa), dist_pa, dist_size, flags);

        let cpu_pa = PhysAddr::from(self.cpu_base).align_down(PGSIZE);
        let cpu_size = self.cpu_size.div_ceil(PGSIZE) * PGSIZE;
        kpt.map_with_alloc(phys_to_virt(cpu_pa), cpu_pa, cpu_size, flags);
    }

    fn set_priority(&self, irq: usize, priority: usize) {
        let _lock = self.lock.lock();
        unsafe {
            let addr = (self.dist_va() + GICD_IPRIORITYR + irq) as *mut u8;
            write_volatile(addr, priority as u8);
        }
    }

    fn set_enable(&self, hartid: usize, irq: usize, enable: bool) {
        let _lock = self.lock.lock();
        unsafe {
            let reg_off = if enable { GICD_ISENABLER } else { GICD_ICENABLER };
            let word = irq / 32;
            let bit = irq % 32;
            let addr = (self.dist_va() + reg_off + word * 4) as *mut u32;
            write_volatile(addr, 1u32 << bit);

            // SPI targets are in ITARGETSR and 4 IRQs share one 32-bit register.
            if irq >= 32 {
                let target_addr = (self.dist_va() + GICD_ITARGETSR + irq) as *mut u8;
                let cpu_mask = 1u8 << (hartid & 0x7);
                write_volatile(target_addr, cpu_mask);
            }
        }
    }

    fn set_threshold(&self, _hartid: usize, threshold: usize) {
        unsafe {
            let addr = (self.cpu_va() + GICC_PMR) as *mut u32;
            write_volatile(addr, threshold as u32);
        }
    }

    fn claim(&self, _hartid: usize) -> usize {
        unsafe {
            let addr = (self.cpu_va() + GICC_IAR) as *const u32;
            let iar = read_volatile(addr);
            (iar & 0x3ff) as usize
        }
    }

    fn complete(&self, _hartid: usize, irq: usize) {
        unsafe {
            let addr = (self.cpu_va() + GICC_EOIR) as *mut u32;
            write_volatile(addr, irq as u32);
        }
    }
}

impl GicV2 {
    pub fn init_cpu_interface(&self) {
        unsafe {
            let cpu_va = self.cpu_va();
            write_volatile((cpu_va + GICC_PMR) as *mut u32, 0xff);
            write_volatile((cpu_va + GICC_CTLR) as *mut u32, 0x1);
            write_volatile((self.dist_va() + GICD_CTLR) as *mut u32, 0x1);
        }
    }
}
