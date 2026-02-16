use super::csr;

pub const MAX_IRQS: usize = 256;

pub fn init() {}
pub fn init_cpu() {}

pub fn enable() {
    unsafe {
        csr::xchg_csr(csr::CSR_CRMD, csr::CRMD_IE, csr::CRMD_IE);
    }
}

pub fn disable() {
    unsafe {
        csr::xchg_csr(csr::CSR_CRMD, 0, csr::CRMD_IE);
    }
}

pub fn is_enabled() -> bool {
    (csr::read_csr(csr::CSR_CRMD) & csr::CRMD_IE) != 0
}

pub fn mask(_irq: usize, _cpuid: usize) {}
pub fn unmask(_irq: usize, _cpuid: usize) {}
pub fn complete(_irq: usize, _cpuid: usize) {}
pub fn claim(_cpuid: usize) -> Option<usize> { None }
pub fn set_priority(_irq: usize, _priority: usize) {}
pub fn clear_soft() {}
pub fn wfi() {
    unsafe { core::arch::asm!("idle 0"); }
}
