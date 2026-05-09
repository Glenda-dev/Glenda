use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const MAX_CPUS: usize = 256;
const CR4_FSGSBASE: usize = 1 << 16;

// Early x86_64 port keeps CPU identity in a software slot until GS-based
// per-cpu storage lands. This is enough for BSP bring-up and single-core tests.
static CPU_ID: AtomicUsize = AtomicUsize::new(0);

pub fn cpu_id() -> usize {
    CPU_ID.load(Ordering::Relaxed)
}

pub fn read_cycle() -> usize {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags));
    }
    (((hi as u64) << 32) | lo as u64) as usize
}

pub fn set_cpuid(cpuid: usize) {
    CPU_ID.store(cpuid, Ordering::Relaxed);
}

pub fn init() {
    unsafe {
        let mut cr4: usize;
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        cr4 |= CR4_FSGSBASE;
        asm!("mov cr4, {}", in(reg) cr4, options(nostack, preserves_flags));
    }
}
