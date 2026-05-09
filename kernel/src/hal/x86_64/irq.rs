use core::arch::asm;

pub const MAX_IRQS: usize = 256;

pub unsafe fn enable() {
    unsafe {
        asm!("sti", options(nomem, nostack, preserves_flags));
    }
}

pub unsafe fn disable() {
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
    }
}

pub fn is_enabled() -> bool {
    let rflags: usize;
    unsafe {
        asm!("pushfq", "pop {}", out(reg) rflags, options(nomem, preserves_flags));
    }
    (rflags & (1 << 9)) != 0
}

pub fn wfi() {
    unsafe {
        asm!("hlt", options(nomem, nostack));
    }
}

pub fn init() {}

pub fn init_cpu() {}

pub fn mask(_irq: usize, _cpuid: usize) {}

pub fn unmask(_irq: usize, _cpuid: usize) {}

pub fn claim(_cpuid: usize) -> Option<u32> {
    None
}

pub fn complete(_irq: usize, _cpuid: usize) {}

pub fn set_affinity(_irq: usize, _cpuid: usize) {}

pub fn set_priority(_irq: usize, _priority: usize) {}

pub fn set_threshold(_threshold: usize, _cpuid: usize) {}

pub fn clear_soft() {}

pub fn send_ipi(_mask: usize, _mask_base: usize) {}
