use super::asm;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const MAX_CPUS: usize = 256;

static CPU_ID: AtomicUsize = AtomicUsize::new(0);

pub fn cpu_id() -> usize {
    CPU_ID.load(Ordering::Relaxed)
}

pub fn read_cycle() -> usize {
    asm::read_cntvct()
}

pub fn set_cpuid(cpuid: usize) {
    CPU_ID.store(cpuid, Ordering::Relaxed);
}

pub fn init() {
    let mpidr = asm::read_mpidr();
    // Aff0 is enough for current supported core counts.
    set_cpuid(mpidr & 0xff);
}
