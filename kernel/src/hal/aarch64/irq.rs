use super::drivers::INTC;
use super::drivers::IntcDriver;
use super::asm;

pub const MAX_IRQS: usize = 1024;

pub unsafe fn enable() {
    let daif = asm::read_daif() & !0x3c0usize;
    unsafe { asm::write_daif(daif) }
}

pub unsafe fn disable() {
    let daif = asm::read_daif() | 0x3c0usize;
    unsafe { asm::write_daif(daif) }
}

pub fn is_enabled() -> bool {
    let daif = asm::read_daif();
    // DAIF bits are [9:6], where bit 7 is I (IRQ mask)
    (daif & (1 << 7)) == 0
}

pub fn wfi() {
    asm::wfi();
}

pub fn init() {
    // Configure default CPU interface state for boot CPU.
    let cpuid = super::cpu::cpu_id();
    set_threshold(0xff, cpuid);
    // Enable common local interrupts:
    // SGI1 for IPI, PPI27 for virtual timer.
    unmask(1, cpuid);
    unmask(27, cpuid);
}

pub fn init_cpu() {
    if let Some(intc) = INTC.get()
        && let IntcDriver::GicV2(gic) = intc
    {
        gic.init_cpu_interface();
    }
}

pub fn mask(irq: usize, cpuid: usize) {
    if let Some(intc) = INTC.get() {
        intc.set_enable(cpuid, irq, false);
    }
}

pub fn unmask(irq: usize, cpuid: usize) {
    if let Some(intc) = INTC.get() {
        intc.set_enable(cpuid, irq, true);
    }
}

pub fn claim(cpuid: usize) -> Option<u32> {
    if let Some(intc) = INTC.get() {
        let id = intc.claim(cpuid);
        if id >= 1020 { None } else { Some(id as u32) }
    } else {
        None
    }
}

pub fn complete(irq: usize, cpuid: usize) {
    if let Some(intc) = INTC.get() {
        intc.complete(cpuid, irq);
    }
}

pub fn set_affinity(irq: usize, cpuid: usize) {
    if let Some(intc) = INTC.get() {
        intc.set_enable(cpuid, irq, true);
    }
}

pub fn set_priority(irq: usize, priority: usize) {
    if let Some(intc) = INTC.get() {
        intc.set_priority(irq, priority);
    }
}

pub fn set_threshold(threshold: usize, cpuid: usize) {
    if let Some(intc) = INTC.get() {
        intc.set_threshold(cpuid, threshold);
    }
}

pub fn clear_soft() {
    // Drain a pending local SGI/software interrupt if present.
    let cpuid = super::cpu::cpu_id();
    if let Some(irq) = claim(cpuid)
        && irq < 16
    {
        complete(irq as usize, cpuid);
    }
}

pub fn send_ipi(mask: usize, mask_base: usize) {
    let mut target_list: u16 = 0;
    for bit in 0..16 {
        if (mask & (1usize << bit)) != 0 {
            let target = mask_base + bit;
            if target < 16 {
                target_list |= 1u16 << target;
            }
        }
    }
    let sgi_id: u64 = 1; // Use SGI 1 for scheduler IPI.
    let value = ((sgi_id & 0xf) << 24) | target_list as u64;
    unsafe {
        asm::write_icc_sgi1r(value);
    }
    asm::isb();
}
