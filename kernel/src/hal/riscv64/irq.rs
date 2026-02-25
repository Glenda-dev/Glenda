use super::asm;
use super::drivers::INTC;
use crate::hal::riscv64::sbi;

pub const MAX_IRQS: usize = 128;
pub unsafe fn enable() {
    unsafe {
        asm::sstatus_set(1);
    }
}
pub unsafe fn disable() {
    unsafe {
        asm::sstatus_clear(1);
    }
}
pub fn is_enabled() -> bool {
    asm::sstatus_query(1)
}
pub fn wfi() {
    unsafe {
        asm::wfi();
    }
}
pub fn init() {}
pub fn init_cpu() {
    unsafe {
        asm::sstatus_set(1); // SIE
        asm::sie_set(9); // sext
        asm::sie_set(1); // ssoft
        asm::sie_set(5); // stimer
    };
}
pub fn claim(cpuid: usize) -> Option<u32> {
    if let Some(intc) = INTC.get() {
        let id = intc.claim(cpuid);
        if id == 0 { None } else { Some(id as u32) }
    } else {
        None
    }
}
pub fn complete(irq: usize, cpuid: usize) {
    if let Some(intc) = INTC.get() {
        intc.complete(cpuid, irq);
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
pub fn set_affinity(_irq: usize, _cpuid: usize) {
    // RISC-V PLIC does not support per-CPU IRQ routing
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
    unsafe {
        asm::sip_clear(1);
    }
}

pub fn send_ipi(mask: usize, mask_base: usize) {
    sbi::send_ipi(mask, mask_base).expect("Failed to send IPI");
}
