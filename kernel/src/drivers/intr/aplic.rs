#[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
use crate::hal::cpu::cpu_id;
use crate::hal::mem::PGSIZE;
use crate::mem::addr::phys_to_virt;
use crate::mem::{PageTable, Perms, PhysAddr};
use crate::sync::SpinLock;
#[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

pub struct Aplic {
    pub base: usize,
    pub size: usize,
    lock: SpinLock<()>,
}

impl Aplic {
    pub const fn new(base: usize, size: usize) -> Self {
        Self { base, size, lock: SpinLock::new(()) }
    }
}

impl super::InterruptController for Aplic {
    fn map_mmio(&self, kpt: &mut PageTable) {
        let pa = PhysAddr::from(self.base).align_down(PGSIZE);
        let va = phys_to_virt(pa);
        let size = (self.size + PGSIZE - 1) / PGSIZE * PGSIZE;
        let flags = Perms::READ | Perms::WRITE;
        log!(
            "aplic: Map MMIO [{:#x}, {:#x}) -> [{:#x}, {:#x}) {}",
            pa.as_usize(),
            (pa + size).as_usize(),
            va.as_usize(),
            (va + size).as_usize(),
            flags
        );
        kpt.map_with_alloc(va, pa, size, flags);
    }

    fn set_priority(&self, irq: usize, priority: usize) {
        if irq == 0 {
            return;
        }
        let _lock = self.lock.lock();
        unsafe {
            let base_va = phys_to_virt(PhysAddr::from(self.base)).as_usize();
            let target_addr = (base_va + 0x3000 + irq * 4) as *mut u32;
            let mut val = read_volatile(target_addr);
            // Clear priority (bits 31:24)
            val &= 0x00FF_FFFF;
            // Set priority
            val |= ((priority as u32) & 0xFF) << 24;
            write_volatile(target_addr, val);
        }
    }

    fn set_enable(&self, hartid: usize, irq: usize, enable: bool) {
        if irq == 0 {
            return;
        }
        let _lock = self.lock.lock();
        unsafe {
            let base_va = phys_to_virt(PhysAddr::from(self.base)).as_usize();
            if enable {
                // Set target hart for the source
                let target_addr = (base_va + 0x3000 + irq * 4) as *mut u32;
                let mut val = read_volatile(target_addr);
                // Preserve priority, update hart index (bits 23:0, reserved are 0)
                val &= 0xFF00_0000;
                val |= (hartid as u32) & 0xFFFFFF;
                write_volatile(target_addr, val);

                // Enable source
                let setie_addr = (base_va + 0x1E00 + (irq / 32) * 4) as *mut u32;
                write_volatile(setie_addr, 1u32 << (irq % 32));
            } else {
                // Disable source
                let clrie_addr = (base_va + 0x1F00 + (irq / 32) * 4) as *mut u32;
                write_volatile(clrie_addr, 1u32 << (irq % 32));
            }
        }
    }

    fn set_threshold(&self, hartid: usize, threshold: usize) {
        // AIA specific: writes to IMSIC/CPU interface CSRs
        #[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
        if hartid == cpu_id() {
            unsafe {
                // sethreshold (0x15e in S-mode)
                asm!("csrw 0x15e, {}", in(reg) threshold);
            }
        }
        #[cfg(not(any(target_arch = "riscv64", target_arch = "riscv32")))]
        let _ = (hartid, threshold);
    }

    fn claim(&self, _hartid: usize) -> usize {
        #[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
        {
            let irq: usize;
            unsafe {
                // stopei (S-mode Top External Interrupt, 0x15c)
                // Reads highest priority interrupt and acknowledges it.
                asm!("csrr {}, 0x15c", out(reg) irq);
            }
            // IID is in bits 26:16
            return (irq >> 16) & 0x7FF;
        }
        #[cfg(not(any(target_arch = "riscv64", target_arch = "riscv32")))]
        {
            0
        }
    }

    fn complete(&self, _hartid: usize, _irq: usize) {
        // AIA stopei read automatically claims/completes level sensitive handled logic?
        // Actually, for Edge-triggered (MSI), read removes from pending.
        // For Level-triggered (APLIC Direct), read claims.
        // No EOI write is required for AIA.
    }
}

// AIA implementation goes here (CMSI, Direct Mode, etc.)
