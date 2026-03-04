use crate::hal::mem::PGSIZE;
use crate::mem::addr::phys_to_virt;
use crate::mem::{PageTable, Perms, PhysAddr};
use crate::sync::SpinLock;
use core::ptr::{read_volatile, write_volatile};

pub struct Plic {
    pub base: usize,
    pub size: usize,
    lock: SpinLock<()>,
}

impl Plic {
    pub const fn new(base: usize, size: usize) -> Self {
        Self { base, size, lock: SpinLock::new(()) }
    }
}

#[inline(always)]
fn ctx_index(hartid: usize) -> usize {
    // QEMU virt PLIC: per-hart contexts, 2 per hart: M=2*hart, S=2*hart+1
    hartid * 2 + 1
}

impl super::InterruptController for Plic {
    fn map_mmio(&self, kpt: &mut PageTable) {
        let pa = PhysAddr::from(self.base).align_down(PGSIZE);
        let va = phys_to_virt(pa);
        let size = (self.size + PGSIZE - 1) / PGSIZE * PGSIZE;
        let flags = Perms::READ | Perms::WRITE | Perms::DEVICE;
        log!(
            "plic: Map MMIO [{:#x}, {:#x}) -> [{:#x}, {:#x}) {}",
            pa.as_usize(),
            (pa + size).as_usize(),
            va.as_usize(),
            (va + size).as_usize(),
            flags
        );
        kpt.map_with_alloc(va, pa, size, flags);
    }

    fn set_priority(&self, irq: usize, priority: usize) {
        let _lock = self.lock.lock();
        unsafe {
            let addr = phys_to_virt(PhysAddr::from(self.base)).as_usize() + irq * 4;
            write_volatile(addr as *mut u32, priority as u32);
        }
    }

    fn set_enable(&self, hartid: usize, irq: usize, enable: bool) {
        let _lock = self.lock.lock();
        unsafe {
            let context = ctx_index(hartid);
            let word_index = (irq / 32) * 4;
            let addr = phys_to_virt(PhysAddr::from(self.base)).as_usize()
                + 0x2000
                + context * 0x80
                + word_index;
            let bit = 1u32 << (irq % 32);
            let cur = read_volatile(addr as *const u32);
            let new = if enable { cur | bit } else { cur & !bit };
            write_volatile(addr as *mut u32, new);
        }
    }

    fn set_threshold(&self, hartid: usize, threshold: usize) {
        let _lock = self.lock.lock();
        unsafe {
            let context = ctx_index(hartid);
            let addr =
                phys_to_virt(PhysAddr::from(self.base)).as_usize() + 0x200000 + context * 0x1000;
            write_volatile(addr as *mut u32, threshold as u32);
        }
    }

    fn claim(&self, hartid: usize) -> usize {
        let _lock = self.lock.lock();
        unsafe {
            let context = ctx_index(hartid);
            let addr =
                phys_to_virt(PhysAddr::from(self.base)).as_usize() + 0x200004 + context * 0x1000;
            read_volatile(addr as *const u32) as usize
        }
    }

    fn complete(&self, hartid: usize, irq: usize) {
        let _lock = self.lock.lock();
        unsafe {
            let context = ctx_index(hartid);
            let addr =
                phys_to_virt(PhysAddr::from(self.base)).as_usize() + 0x200004 + context * 0x1000;
            write_volatile(addr as *mut u32, irq as u32);
        }
    }
}
